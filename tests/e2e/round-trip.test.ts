import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import { expectHit } from "../support/assertions.ts";
import { OCTET_STREAM } from "../support/client.ts";
import { startServer } from "../support/nx-cache-aws.ts";
import { e2eServerEnv } from "./storage/shared-storage.ts";

const ABOVE_AXUM_DEFAULT_BODY_LIMIT = 6 * 1024 * 1024;

test("a new artifact is stored with exactly 200, the only status Nx accepts", async () => {
    const server = await startServer(e2eServerEnv());

    const reply = await server.client("read-write").put(uniqueHash("new"), artifact(1024));

    expect(reply.status).toBe(200);
});

test("a stored 1 KiB artifact is returned byte for byte", async () => {
    await expectStoredBytesReturned(1024);
});

test("a stored 6 MiB artifact is returned byte for byte", async () => {
    await expectStoredBytesReturned(ABOVE_AXUM_DEFAULT_BODY_LIMIT);
});

async function expectStoredBytesReturned(length: number): Promise<void> {
    const server = await startServer(e2eServerEnv());
    const client = server.client("read-write");
    const hash = uniqueHash("round-trip");
    const body = artifact(length);

    const write = await client.put(hash, body);
    const reply = await client.get(hash);

    expect(write.status, "PUT").toBe(200);
    expectHit(reply, body, "GET");
    expect(reply.headers.get("content-type"), "GET").toBe(OCTET_STREAM);
}
