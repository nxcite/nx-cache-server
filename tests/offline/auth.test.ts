import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import type { Reply, Token } from "../support/client.ts";
import { startServer } from "../support/nx-cache-aws.ts";
import { VALID_ENV, withVariables } from "../support/server-env.ts";

async function getWith(token: Token): Promise<Reply> {
    const server = await startServer(VALID_ENV);

    return server.client(token).get("some-hash");
}

test("a download without an Authorization header is rejected", async () => {
    const reply = await getWith("none");

    expect(reply.status).toBe(401);
});

test("an upload without an Authorization header is rejected", async () => {
    const server = await startServer(VALID_ENV);

    const reply = await server.client("none").put(uniqueHash("unauthenticated"), artifact(1024));

    expect(reply.status).toBe(401);
});

test("an unknown bearer token is rejected", async () => {
    const reply = await getWith({ authorization: "Bearer unknown-token" });

    expect(reply.status).toBe(401);
});

test("without a read-only token configured, only the service access token is accepted", async () => {
    const server = await startServer(
        withVariables(VALID_ENV, { READ_ONLY_ACCESS_TOKEN: undefined }),
    );
    const hash = uniqueHash("no-read-only-token");

    const readOnly = await server.client("read-only").get(hash);
    const readWrite = await server.client("read-write").get(hash);

    expect(readOnly.status, "read-only GET").toBe(401);
    expect(readWrite.status, "read-write GET").not.toBe(401);
});
