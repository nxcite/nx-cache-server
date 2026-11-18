import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import { jsonLines, requestLines } from "../support/logs.ts";
import { startServer } from "../support/nx-cache-aws.ts";
import { withVariables } from "../support/server-env.ts";
import { e2eServerEnv } from "./storage/shared-storage.ts";

test("each cache outcome is logged with its level and byte count", async () => {
    const server = await startServer(
        withVariables(e2eServerEnv(), { LOG_FORMAT: "json", DEBUG: "true" }),
    );
    const client = server.client("read-write");
    const hash = uniqueHash("outcomes");
    const body = artifact(1024);

    await client.put(hash, body);
    await client.put(hash, body);
    await client.get(hash);
    await client.get(uniqueHash("outcomes-missing"));
    const lines = requestLines((await server.stop()).stdout);

    expect(lines).toMatchObject([
        { outcome: "stored", status: 200, bytes: 1024, access: "read_write", level: "INFO" },
        { outcome: "exists", status: 409, bytes: 0, level: "DEBUG" },
        { outcome: "hit", status: 200, bytes: 1024, level: "INFO" },
        { outcome: "miss", status: 404, bytes: 0, level: "INFO" },
    ]);
});

test("the default level logs hits, misses and stores but not exists", async () => {
    const server = await startServer(withVariables(e2eServerEnv(), { LOG_FORMAT: "json" }));
    const client = server.client("read-write");
    const hash = uniqueHash("quiet");

    await client.put(hash, artifact(16));
    await client.put(hash, artifact(16));
    await client.get(hash);
    await client.get(uniqueHash("quiet-missing"));

    expect(requestLines((await server.stop()).stdout).map((line) => line.outcome)).toEqual([
        "stored",
        "hit",
        "miss",
    ]);
});

test("an S3 error is logged as readable text", async () => {
    const server = await startServer(
        withVariables(e2eServerEnv(), {
            LOG_FORMAT: "json",
            S3_BUCKET_NAME: uniqueHash("missing-bucket"),
        }),
    );
    const client = server.client("read-write");
    const hash = uniqueHash("s3-error-text");

    await client.put(hash, artifact(16));
    await client.get(hash);
    const warnings = jsonLines((await server.stop()).stdout).filter(
        (line) => line.message === "S3 request failed",
    );

    expect(warnings).toContainEqual(expect.objectContaining({ code: "NoSuchBucket" }));
    for (const warning of warnings) {
        // Debug struct syntax, e.g. `Error { code: … }`.
        expect(warning.error).not.toContain(" { ");
        expect(warning.error).not.toMatch(/^service error/);
    }
});
