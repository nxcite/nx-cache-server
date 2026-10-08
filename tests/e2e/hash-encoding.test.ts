import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import { expectHit } from "../support/assertions.ts";
import { MAX_HASH_LENGTH } from "../support/client.ts";
import { startServer } from "../support/nx-cache-aws.ts";
import { e2eServerEnv } from "./storage/shared-storage.ts";

test("a hash of the maximum 128 characters round-trips", async () => {
    await expectHashRoundTrips(uniqueHash("max-length").padEnd(MAX_HASH_LENGTH, "x"));
});

test("a percent-encoded non-ASCII hash round-trips", async () => {
    await expectHashRoundTrips(`${uniqueHash("non-ascii")}-%C3%A9t%C3%A9`);
});

async function expectHashRoundTrips(hash: string): Promise<void> {
    const server = await startServer(e2eServerEnv());
    const client = server.client("read-write");
    const body = artifact(1024);

    const write = await client.put(hash, body);
    const read = await client.get(hash);

    expect(write.status, `PUT ${hash}`).toBe(200);
    expectHit(read, body, `GET ${hash}`);
}
