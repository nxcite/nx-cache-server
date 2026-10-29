import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import { KeepAliveConnection } from "../support/keep-alive-connection.ts";
import { jsonLines, requestLines } from "../support/logs.ts";
import { startServer } from "../support/nx-cache-aws.ts";
import { withVariables } from "../support/server-env.ts";
import { e2eServerEnv } from "./storage/shared-storage.ts";

const DECLARED_LENGTH = 64 * 1024;
const SENT_LENGTH = 16 * 1024;

test("an upload cut short stores nothing and can be retried", async () => {
    const server = await startServer(withVariables(e2eServerEnv(), { LOG_FORMAT: "json" }));
    const client = server.client("read-write");
    const hash = uniqueHash("aborted");
    const connection = new KeepAliveConnection(server.port);

    const truncated = await connection.putTruncated(
        hash,
        "read-write",
        DECLARED_LENGTH,
        SENT_LENGTH,
    );
    const afterTruncation = await client.get(hash);
    const retry = await client.put(hash, artifact(DECLARED_LENGTH));

    expect(truncated.status, "truncated PUT").toBe(400);
    expect(afterTruncation.status, "GET after the truncated PUT").toBe(404);
    expect(retry.status, "full PUT").toBe(200);

    const { stdout } = await server.stop();
    expect(jsonLines(stdout).filter((line) => line.level === "ERROR")).toEqual([]);
    expect(requestLines(stdout)[0]).toMatchObject({
        method: "PUT",
        status: 400,
        outcome: "aborted",
        level: "INFO",
    });
});
