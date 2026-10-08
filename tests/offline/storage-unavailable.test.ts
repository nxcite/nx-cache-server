import { createServer } from "node:http";
import { expect, onTestFinished, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import { ERROR_LOG_LEVEL, startServer } from "../support/nx-cache-aws.ts";
import { LOOPBACK, boundPort } from "../support/ports.ts";
import { VALID_ENV, withVariables } from "../support/server-env.ts";

test("an upload is not acknowledged when storage is unreachable", async () => {
    const server = await startServer(VALID_ENV);
    const client = server.client("read-write");

    const upload = await client.put(uniqueHash("unreachable-storage"), artifact(1024));
    const health = await client.request("GET", "/health");

    expect(upload.status, "PUT").not.toBe(200);
    expect(health.status, "GET /health afterwards").toBe(200);
});

test("a download returns no artifact when storage is unreachable", async () => {
    const server = await startServer(VALID_ENV);

    const reply = await server.client("read-write").get(uniqueHash("unreachable-storage"));

    expect(reply.status).not.toBe(200);
});

test("a storage that never answers doesn't hang the request", async () => {
    const silentStoragePort = await startSilentStorage();
    const server = await startServer(
        withVariables(VALID_ENV, {
            S3_ENDPOINT_URL: `http://${LOOPBACK}:${silentStoragePort}`,
            S3_TIMEOUT: "1",
        }),
    );

    const reply = await server.client("read-write").get(uniqueHash("silent-storage"));

    expect(reply.status).not.toBe(200);
});

test("a storage failure is logged as an error", async () => {
    const server = await startServer(VALID_ENV);

    await server.client("read-write").get(uniqueHash("logged-failure"));
    const logs = await server.stop();

    expect(logs.stdout).toContain(ERROR_LOG_LEVEL);
});

async function startSilentStorage(): Promise<number> {
    const silentStorage = createServer();
    await new Promise<void>((resolve) => silentStorage.listen(0, LOOPBACK, resolve));

    onTestFinished(() => {
        // close() alone leaves the SDK's pooled keep-alive connection open.
        silentStorage.closeAllConnections();
        silentStorage.close();
    });

    return boundPort(silentStorage);
}
