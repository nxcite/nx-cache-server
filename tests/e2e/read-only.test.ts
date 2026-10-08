import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import { expectHit } from "../support/assertions.ts";
import { startServer } from "../support/nx-cache-aws.ts";
import { e2eServerEnv } from "./storage/shared-storage.ts";

test("the read-only token reads a stored artifact", async () => {
    const server = await startServer(e2eServerEnv());
    const hash = uniqueHash("read-only-hit");
    const body = artifact(1024);

    const write = await server.client("read-write").put(hash, body);
    const read = await server.client("read-only").get(hash);

    expect(write.status, "read-write PUT").toBe(200);
    expectHit(read, body, "read-only GET");
});

test("the read-only token cannot store an artifact", async () => {
    const server = await startServer(e2eServerEnv());
    const hash = uniqueHash("read-only-write");

    const write = await server.client("read-only").put(hash, artifact(1024));
    const read = await server.client("read-write").get(hash);

    expect(write.status, "read-only PUT").toBe(403);
    expect(read.status, "read-write GET after the refused PUT").toBe(404);
});
