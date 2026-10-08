import { expect, test } from "vitest";
import { artifact, uniqueHash } from "../support/artifacts.ts";
import { ERROR_LOG_LEVEL, startServer } from "../support/nx-cache-aws.ts";
import { e2eServerEnv } from "./storage/shared-storage.ts";

test("a missing artifact is a clean 404, not a logged storage failure", async () => {
    const server = await startServer(e2eServerEnv());

    const reply = await server.client("read-write").get(uniqueHash("miss"));
    const logs = await server.stop();

    expect(reply.status).toBe(404);
    expect(logs.stdout).not.toContain(ERROR_LOG_LEVEL);
});

test("the existence check before a first upload logs no storage failure", async () => {
    const server = await startServer(e2eServerEnv());

    const reply = await server.client("read-write").put(uniqueHash("first-put"), artifact(1024));
    const logs = await server.stop();

    expect(reply.status).toBe(200);
    expect(logs.stdout).not.toContain(ERROR_LOG_LEVEL);
});
