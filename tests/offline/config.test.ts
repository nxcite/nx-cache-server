import { expect, test } from "vitest";
import { expectExit } from "../support/assertions.ts";
import {
    BIND_FAILURE_MESSAGE,
    STARTUP_FAILURE_EXIT_CODE,
    runServerUntilExit,
} from "../support/nx-cache-aws.ts";
import { LOOPBACK, holdPort } from "../support/ports.ts";
import { VALID_ENV, withVariables } from "../support/server-env.ts";

test("an empty service access token is rejected", async () => {
    const exited = await runServerUntilExit(withVariables(VALID_ENV, { SERVICE_ACCESS_TOKEN: "" }));

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("Service access token is required");
});

test("an empty bucket name is rejected", async () => {
    const exited = await runServerUntilExit(withVariables(VALID_ENV, { S3_BUCKET_NAME: "" }));

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("S3 bucket name is required");
});

test("port 0 is rejected", async () => {
    const exited = await runServerUntilExit(withVariables(VALID_ENV, { PORT: "0" }));

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("port must be greater than 0");
});

test("a port already in use fails the start", async () => {
    await using takenPort = await holdPort();

    const exited = await runServerUntilExit(
        withVariables(VALID_ENV, { PORT: String(takenPort.port) }),
    );

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain(BIND_FAILURE_MESSAGE);
});

test("a read-only token equal to the service access token is rejected", async () => {
    const exited = await runServerUntilExit(
        withVariables(VALID_ENV, {
            READ_ONLY_ACCESS_TOKEN: VALID_ENV.SERVICE_ACCESS_TOKEN,
        }),
    );

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("must differ from SERVICE_ACCESS_TOKEN");
});

test("an empty read-only token is rejected", async () => {
    const exited = await runServerUntilExit(
        withVariables(VALID_ENV, { READ_ONLY_ACCESS_TOKEN: "" }),
    );

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("must not be empty when provided");
});

test("an endpoint URL without a scheme is rejected", async () => {
    const exited = await runServerUntilExit(
        withVariables(VALID_ENV, { S3_ENDPOINT_URL: `${LOOPBACK}:1` }),
    );

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("must start with http://");
});

test("an access key without a secret is rejected", async () => {
    const exited = await runServerUntilExit(
        withVariables(VALID_ENV, { AWS_SECRET_ACCESS_KEY: undefined }),
    );

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("You provided AWS_ACCESS_KEY_ID but not");
});

test("a secret without an access key is rejected", async () => {
    const exited = await runServerUntilExit(
        withVariables(VALID_ENV, { AWS_ACCESS_KEY_ID: undefined }),
    );

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("You provided AWS_SECRET_ACCESS_KEY but not");
});

test("a missing region is rejected", async () => {
    const exited = await runServerUntilExit(withVariables(VALID_ENV, { AWS_REGION: undefined }));

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("Could not determine AWS region");
});
