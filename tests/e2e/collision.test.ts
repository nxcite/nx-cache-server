import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import { expectHit } from "../support/assertions.ts";
import { IN_FLIGHT_UPLOAD_LENGTH, KeepAliveConnection } from "../support/keep-alive-connection.ts";
import { startServer } from "../support/nx-cache-aws.ts";
import { e2eServerEnv } from "./storage/shared-storage.ts";

test("a duplicate upload is received in full, then refused with 409", async () => {
    const server = await startServer(e2eServerEnv());
    const hash = uniqueHash("collision-in-flight");
    const first = await server.client("read-write").put(hash, artifact(1024));
    const connection = new KeepAliveConnection(server.port);

    const duplicate = await connection.putStreaming(hash, "read-write", IN_FLIGHT_UPLOAD_LENGTH);

    expect(first.status, "first PUT").toBe(200);
    expect(duplicate.status, "duplicate PUT").toBe(409);
});

test("a duplicate upload does not replace the stored artifact", async () => {
    const server = await startServer(e2eServerEnv());
    const client = server.client("read-write");
    const hash = uniqueHash("collision-poisoning");
    const original = artifact(1024);
    const first = await client.put(hash, original);

    const duplicate = await client.put(hash, artifact(1024));
    const stored = await client.get(hash);

    expect(first.status, "first PUT").toBe(200);
    expect(duplicate.status, "PUT with different bytes").toBe(409);
    expectHit(stored, original, "GET after the refused PUT");
});
