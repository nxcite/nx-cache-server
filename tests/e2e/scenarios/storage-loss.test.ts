import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../../support/artifacts.ts";
import { expectHit } from "../../support/assertions.ts";
import { ERROR_LOG_LEVEL, startServer } from "../../support/nx-cache-aws.ts";
import { withVariables } from "../../support/server-env.ts";
import { e2eServerEnv, storageAdmin } from "../storage/shared-storage.ts";

const BODY_LENGTH = 64 * 1024;

test("the server survives losing its bucket and recovers when it returns", async () => {
    const admin = storageAdmin();
    const ownBucket = uniqueHash("storage-loss-bucket");
    await admin.createBucket(ownBucket);
    const server = await startServer(withVariables(e2eServerEnv(), { S3_BUCKET_NAME: ownBucket }));
    const client = server.client("read-write");
    const stored = uniqueHash("storage-loss-stored");
    const storedBody = artifact(BODY_LENGTH);
    const during = uniqueHash("storage-loss-during");
    const duringBody = artifact(BODY_LENGTH);

    const putStored = await client.put(stored, storedBody);
    const getStored = await client.get(stored);
    expect(putStored.status, "before the loss: PUT stored").toBe(200);
    expectHit(getStored, storedBody, "before the loss: GET stored");

    await admin.deleteBucket(ownBucket);

    const getWithoutBucket = await client.get(stored);
    const putWithoutBucket = await client.put(during, duringBody);
    const health = await client.request("GET", "/health");
    expect(getWithoutBucket.status, "bucket lost: GET stored").not.toBe(200);
    expect(putWithoutBucket.status, "bucket lost: PUT during").not.toBe(200);
    expect(health.status, "bucket lost: GET /health").toBe(200);

    await admin.createBucket(ownBucket);
    const putRecovered = await client.put(during, duringBody);
    const getRecovered = await client.get(during);
    expect(putRecovered.status, "bucket back: PUT during").toBe(200);
    expectHit(getRecovered, duringBody, "bucket back: GET during");

    const logs = await server.stop();
    expect(logs.stdout, "server log").toContain(ERROR_LOG_LEVEL);
});
