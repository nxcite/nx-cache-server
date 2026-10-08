import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../../support/artifacts.ts";
import { expectHit, expectOneOf } from "../../support/assertions.ts";
import { startServer } from "../../support/nx-cache-aws.ts";
import { e2eServerEnv } from "../storage/shared-storage.ts";

const BODY_LENGTH = 6 * 1024 * 1024;
const CONCURRENT_READS = 10;
// 200 and 200: both PUTs can pass the existence check before either one writes.
const ALLOWED_STATUS_PAIRS = [
    [200, 409],
    [200, 200],
];

test("concurrent GETs and racing PUTs of one hash never return mixed bytes", async () => {
    const server = await startServer(e2eServerEnv());
    const client = server.client("read-write");
    const popular = uniqueHash("same-hash-race-popular");
    const popularBody = artifact(BODY_LENGTH);
    const raced = uniqueHash("same-hash-race-raced");
    const first = artifact(BODY_LENGTH);
    const second = artifact(BODY_LENGTH);

    const put = await client.put(popular, popularBody);
    const reads = await Promise.all(
        Array.from({ length: CONCURRENT_READS }, () => client.get(popular)),
    );
    expect(put.status, "step 1: PUT").toBe(200);
    for (const reply of reads) {
        expectHit(reply, popularBody, "step 1: concurrent GET");
    }

    const [firstPut, secondPut] = await Promise.all([
        client.put(raced, first),
        client.put(raced, second),
    ]);
    const stored = await client.get(raced);
    const observed = `step 2: concurrent PUTs answered ${firstPut.status} and ${secondPut.status}`;
    const statuses = [firstPut.status, secondPut.status].toSorted((x, y) => x - y);
    const acceptedBodies = [
        { reply: firstPut, body: first },
        { reply: secondPut, body: second },
    ]
        .filter(({ reply }) => reply.status === 200)
        .map(({ body }) => body);

    expect(ALLOWED_STATUS_PAIRS, observed).toContainEqual(statuses);
    expect(stored.status, `${observed}; GET`).toBe(200);
    expectOneOf(stored.body, acceptedBodies, `${observed}; GET`);
});
