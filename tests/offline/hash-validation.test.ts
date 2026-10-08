import { expect, test } from "vitest";
import { artifact } from "../support/artifacts.ts";
import { MAX_HASH_LENGTH } from "../support/client.ts";
import { startServer } from "../support/nx-cache-aws.ts";
import { VALID_ENV } from "../support/server-env.ts";

const INVALID_HASHES = {
    "encoded slash": "a%2Fb",
    dot: "a.b",
    "one character too long": "a".repeat(MAX_HASH_LENGTH + 1),
    "invalid UTF-8": "%FF",
};

test("invalid hashes are rejected with 400", async () => {
    const server = await startServer(VALID_ENV);
    const client = server.client("read-write");

    for (const [label, hash] of Object.entries(INVALID_HASHES)) {
        const read = await client.get(hash);
        const write = await client.put(hash, artifact(16));

        expect(read.status, `GET with ${label}`).toBe(400);
        expect(write.status, `PUT with ${label}`).toBe(400);
    }
});
