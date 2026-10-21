import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import { MAX_HASH_LENGTH, cachePath } from "../support/client.ts";
import { type LogLine, jsonLines, requestLines } from "../support/logs.ts";
import { ERROR_LOG_LEVEL, type Server, startServer } from "../support/nx-cache-aws.ts";
import { LOOPBACK } from "../support/ports.ts";
import { pollUntil } from "../support/process.ts";
import {
    READ_ONLY_TOKEN,
    READ_WRITE_TOKEN,
    type ServerEnv,
    VALID_ENV,
    withVariables,
} from "../support/server-env.ts";

const JSON_ENV = withVariables(VALID_ENV, { LOG_FORMAT: "json" });

async function onlyRequestLine(
    env: ServerEnv,
    send: (server: Server) => Promise<unknown>,
): Promise<LogLine> {
    const server = await startServer(env);

    await send(server);
    const lines = requestLines((await server.stop()).stdout);

    const [line, ...others] = lines;
    if (line === undefined || others.length > 0) {
        throw new Error(`expected exactly one request line, got ${lines.length}`);
    }
    return line;
}

test("a request without a token logs unauthorized", async () => {
    const line = await onlyRequestLine(JSON_ENV, (server) =>
        server.client("none").get("some-hash"),
    );

    expect(line).toMatchObject({
        method: "GET",
        hash: "some-hash",
        status: 401,
        outcome: "unauthorized",
        access: "none",
        level: "WARN",
    });
});

test("a read-only upload logs forbidden only at debug", async () => {
    const atInfo = await startServer(JSON_ENV);
    await atInfo.client("read-only").put(uniqueHash("forbidden"), artifact(1024));
    const infoLines = requestLines((await atInfo.stop()).stdout);

    const line = await onlyRequestLine(withVariables(JSON_ENV, { DEBUG: "true" }), (server) =>
        server.client("read-only").put(uniqueHash("forbidden"), artifact(1024)),
    );

    expect(infoLines).toEqual([]);
    expect(line).toMatchObject({
        method: "PUT",
        status: 403,
        outcome: "forbidden",
        access: "read_only",
        level: "DEBUG",
    });
});

test("an invalid hash is logged without the hash", async () => {
    const line = await onlyRequestLine(JSON_ENV, (server) =>
        server.client("read-write").get("a.b"),
    );

    expect(line).toMatchObject({
        status: 400,
        outcome: "invalid",
        access: "read_write",
        level: "WARN",
    });
    expect(line).not.toHaveProperty("hash");
});

test("a too-long hash is not logged", async () => {
    const line = await onlyRequestLine(JSON_ENV, (server) =>
        server.client("read-write").get("a".repeat(MAX_HASH_LENGTH + 1)),
    );

    expect(line.status).toBe(400);
    expect(line).not.toHaveProperty("hash");
});

test("a percent-encoded hash is logged decoded", async () => {
    const line = await onlyRequestLine(JSON_ENV, (server) =>
        server.client("none").request("GET", "/v1/cache/abc-%C3%A9t%C3%A9"),
    );

    expect(line.hash).toBe("abc-été");
});

test("an unknown route logs invalid", async () => {
    const line = await onlyRequestLine(JSON_ENV, (server) =>
        server.client("none").request("GET", "/nope"),
    );

    expect(line).toMatchObject({ status: 404, outcome: "invalid" });
});

test("health checks are logged only at debug", async () => {
    const atInfo = await startServer(JSON_ENV);
    await atInfo.client("none").request("GET", "/health");
    const infoLines = requestLines((await atInfo.stop()).stdout);

    const atDebug = await startServer(withVariables(JSON_ENV, { DEBUG: "true" }));
    await atDebug.client("none").request("GET", "/health");
    const debugLines = requestLines((await atDebug.stop()).stdout);

    expect(infoLines.filter((line) => line.outcome === "health")).toEqual([]);
    expect(debugLines.filter((line) => line.outcome === "health")).toEqual([
        expect.objectContaining({ level: "DEBUG", status: 200 }),
    ]);
});

test("a storage failure logs an ERROR request line", async () => {
    const line = await onlyRequestLine(JSON_ENV, (server) =>
        server.client("read-write").get(uniqueHash("storage-failure")),
    );

    expect(line).toMatchObject({ status: 500, outcome: "error", level: "ERROR" });
});

test("a storage failure logs exactly one ERROR line", async () => {
    const server = await startServer(VALID_ENV);

    await server.client("read-write").get(uniqueHash("one-error"));
    const logs = await server.stop();

    const errorLines = logs.stdout.split("\n").filter((line) => line.includes(ERROR_LOG_LEVEL));
    expect(errorLines).toHaveLength(1);
});

test("a storage failure carries the hash on the S3 warning", async () => {
    const server = await startServer(JSON_ENV);
    const hash = uniqueHash("s3-warn");

    await server.client("read-write").get(hash);
    const lines = jsonLines((await server.stop()).stdout);

    expect(lines).toContainEqual(
        expect.objectContaining({
            level: "WARN",
            message: "S3 request failed",
            operation: "get_object",
            span: expect.objectContaining({ hash }),
        }),
    );
});

test("the S3 warning gives the error without a debug dump", async () => {
    const server = await startServer(JSON_ENV);

    await server.client("read-write").get(uniqueHash("s3-error-text"));
    const warning = jsonLines((await server.stop()).stdout).find(
        (line) => line.message === "S3 request failed",
    );

    expect(warning?.error).toEqual(expect.any(String));
    // Debug struct syntax, e.g. `Unhandled { source: … }`.
    expect(warning?.error).not.toContain(" { ");
});

test("a request the client abandons is still logged", async () => {
    const server = await startServer(JSON_ENV);
    const hash = uniqueHash("abandoned");

    // Gives up long before the unreachable storage times out.
    await fetch(`http://${LOOPBACK}:${server.port}${cachePath(hash)}`, {
        headers: { authorization: `Bearer ${READ_WRITE_TOKEN}` },
        signal: AbortSignal.timeout(300),
    }).catch(() => undefined);
    await pollUntil(
        5_000,
        "no request line",
        () => requestLines(server.output().stdout).length > 0,
    );
    const lines = requestLines((await server.stop()).stdout);

    expect(lines).toEqual([
        expect.objectContaining({
            method: "GET",
            hash,
            outcome: "aborted",
            access: "read_write",
            level: "INFO",
        }),
    ]);
    expect(lines[0]).not.toHaveProperty("status");
});

test("tokens never reach the logs", async () => {
    const server = await startServer(JSON_ENV);
    const hash = uniqueHash("tokens");

    await server.client("read-write").get(hash);
    await server.client("read-only").put(hash, artifact(16));
    await server.client({ authorization: "Bearer some-guess-123" }).get(hash);
    await server.client({ authorization: "Basic c2VjcmV0" }).get(hash);
    const logs = await server.stop();

    const output = logs.stdout + logs.stderr;
    for (const secret of [READ_WRITE_TOKEN, READ_ONLY_TOKEN, "some-guess-123", "c2VjcmV0"]) {
        expect(output).not.toContain(secret);
    }
});
