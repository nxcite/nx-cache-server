import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../../support/artifacts.ts";
import { expectHit } from "../../support/assertions.ts";
import { cachePath } from "../../support/client.ts";
import {
    IN_FLIGHT_UPLOAD_LENGTH,
    KeepAliveConnection,
} from "../../support/keep-alive-connection.ts";
import { startServer } from "../../support/nx-cache-aws.ts";
import { e2eServerEnv } from "../storage/shared-storage.ts";

const BODY_LENGTH = 64 * 1024;

test("read-only and read-write clients on one server see each other's artifacts and permissions", async () => {
    const server = await startServer(e2eServerEnv());
    const readWrite = server.client("read-write");
    const readOnly = server.client("read-only");
    const shared = uniqueHash("mixed-tokens-shared");
    const refused = uniqueHash("mixed-tokens-refused");
    const body = artifact(BODY_LENGTH);

    const missBeforePut = await readOnly.get(shared);
    const put = await readWrite.put(shared, body);
    expect(missBeforePut.status, "step 1: read-only GET before the PUT").toBe(404);
    expect(put.status, "step 1: read-write PUT").toBe(200);

    const hit = await readOnly.get(shared);
    expectHit(hit, body, "step 2: read-only GET");

    const refusedPut = await readOnly.put(refused, body);
    const afterRefusal = await readWrite.get(refused);
    expect(refusedPut.status, "step 3: read-only PUT").toBe(403);
    expect(afterRefusal.status, "step 3: read-write GET of the refused hash").toBe(404);

    const connection = new KeepAliveConnection(server.port);
    const streamed = await connection.putStreaming(
        uniqueHash("mixed-tokens-streamed"),
        "read-only",
        IN_FLIGHT_UPLOAD_LENGTH,
    );
    const reused = await connection.get(cachePath(shared), "read-write");
    expect(streamed.status, "step 4: read-only streamed PUT").toBe(403);
    expect(reused, "step 4: read-write GET on the same connection").toEqual({
        status: 200,
        reusedConnection: true,
    });
});
