import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../../support/artifacts.ts";
import { expectHit } from "../../support/assertions.ts";
import type { Reply } from "../../support/client.ts";
import { ERROR_LOG_LEVEL, startServer } from "../../support/nx-cache-aws.ts";
import { e2eServerEnv, sharedBucket, storageAdmin } from "../storage/shared-storage.ts";

const BODY_LENGTH = 256 * 1024;
const READS_PER_ENTRY = 5;

interface Entry {
    name: string;
    hash: string;
    body: Uint8Array<ArrayBuffer>;
}

interface Outcome {
    entry: Entry;
    reply: Reply;
}

function entry(name: string): Entry {
    return {
        name,
        hash: uniqueHash(`lifecycle-${name.toLowerCase()}`),
        body: artifact(BODY_LENGTH),
    };
}

function sendEach(entries: Entry[], send: (entry: Entry) => Promise<Reply>): Promise<Outcome[]> {
    return Promise.all(entries.map(async (entry) => ({ entry, reply: await send(entry) })));
}

test("stores, hits, expiry and refills stay consistent on one server", async () => {
    const server = await startServer(e2eServerEnv());
    const client = server.client("read-write");
    const a = entry("A");
    const b = entry("B");
    const c = entry("C");
    const d = entry("D");
    const e = entry("E");
    const put = (entry: Entry): Promise<Reply> => client.put(entry.hash, entry.body);
    const get = (entry: Entry): Promise<Reply> => client.get(entry.hash);

    for (const { entry, reply } of await sendEach([a, b], put)) {
        expect(reply.status, `step 1: PUT ${entry.name}`).toBe(200);
    }

    const repeatedReads = Array.from({ length: READS_PER_ENTRY }, () => [a, b]).flat();
    for (const { entry, reply } of await sendEach(repeatedReads, get)) {
        expectHit(reply, entry.body, `step 2: GET ${entry.name}`);
    }

    const putC = await client.put(c.hash, c.body);
    expect(putC.status, "step 3: PUT C").toBe(200);
    for (const { entry, reply } of await sendEach([d, e], put)) {
        expect(reply.status, `step 3: PUT ${entry.name}`).toBe(200);
    }

    const [stored, neverStored] = await Promise.all([
        sendEach([a, c, e], get),
        client.get(uniqueHash("lifecycle-never-stored")),
    ]);
    for (const { entry, reply } of stored) {
        expectHit(reply, entry.body, `step 4: GET ${entry.name}`);
    }
    expect(neverStored.status, "step 4: GET never stored").toBe(404);

    await Promise.all([b, d].map(({ hash }) => storageAdmin().deleteObject(sharedBucket(), hash)));
    const [expired, kept] = await Promise.all([sendEach([b, d], get), sendEach([a, c, e], get)]);
    for (const { entry, reply } of expired) {
        expect(reply.status, `step 5: GET expired ${entry.name}`).toBe(404);
    }
    for (const { entry, reply } of kept) {
        expectHit(reply, entry.body, `step 5: GET kept ${entry.name}`);
    }

    const refilledB = artifact(BODY_LENGTH);
    const refill = await client.put(b.hash, refilledB);
    const afterRefill = await client.get(b.hash);
    expect(refill.status, "step 6: PUT expired B").toBe(200);
    expectHit(afterRefill, refilledB, "step 6: GET refilled B");

    const overwrite = await client.put(a.hash, artifact(BODY_LENGTH));
    const afterOverwrite = await client.get(a.hash);
    expect(overwrite.status, "step 7: PUT A with new bytes").toBe(409);
    expectHit(afterOverwrite, a.body, "step 7: GET A");

    const logs = await server.stop();
    expect(logs.stdout, "step 8: server log").not.toContain(ERROR_LOG_LEVEL);
});
