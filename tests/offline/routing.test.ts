import { expect, test } from "vitest";
import { startServer } from "../support/nx-cache-aws.ts";
import { VALID_ENV } from "../support/server-env.ts";

test("health check is public", async () => {
    const server = await startServer(VALID_ENV);

    const reply = await server.client("none").request("GET", "/health");

    expect(reply.status).toBe(200);
    expect(new TextDecoder().decode(reply.body)).toBe("OK");
});
