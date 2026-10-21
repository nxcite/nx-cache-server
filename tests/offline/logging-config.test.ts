import { expect, test } from "vitest";
import { expectExit } from "../support/assertions.ts";
import { jsonLines } from "../support/logs.ts";
import {
    STARTUP_FAILURE_EXIT_CODE,
    runServerUntilExit,
    startServer,
} from "../support/nx-cache-aws.ts";
import { VALID_ENV, withVariables } from "../support/server-env.ts";

const CLAP_USAGE_EXIT_CODE = 2;
const DEBUG_IGNORED_WARNING = "DEBUG is set but ignored because RUST_LOG is set";

// Uppercase too: LOG_FORMAT ignores case.
test.each(["json", "JSON"])("LOG_FORMAT=%s makes every stdout line JSON", async (format) => {
    const server = await startServer(withVariables(VALID_ENV, { LOG_FORMAT: format }));

    const logs = await server.stop();

    expect(jsonLines(logs.stdout)).toContainEqual(
        expect.objectContaining({ message: expect.stringContaining("Server running on") }),
    );
});

test("an unknown LOG_FORMAT is a usage error", async () => {
    const exited = await runServerUntilExit(withVariables(VALID_ENV, { LOG_FORMAT: "yaml" }));

    expectExit(exited, CLAP_USAGE_EXIT_CODE);
    expect(exited.stderr).toContain("invalid value 'yaml'");
});

test("an invalid RUST_LOG fails the start", async () => {
    const exited = await runServerUntilExit(
        withVariables(VALID_ENV, { RUST_LOG: "nx_cache_server=loud" }),
    );

    expectExit(exited, STARTUP_FAILURE_EXIT_CODE);
    expect(exited.stderr).toContain("RUST_LOG");
});

test("an empty RUST_LOG is ignored", async () => {
    const server = await startServer(withVariables(VALID_ENV, { RUST_LOG: "" }));

    const logs = await server.stop();

    expect(logs.stderr).not.toContain("RUST_LOG");
});

test("DEBUG accepts 1", async () => {
    const server = await startServer(withVariables(VALID_ENV, { DEBUG: "1" }));

    const logs = await server.stop();

    expect(logs.stdout).toContain("Server running on");
});

test("DEBUG is ignored with a warning when RUST_LOG is set", async () => {
    const server = await startServer(
        withVariables(VALID_ENV, { RUST_LOG: "info", DEBUG: "true", LOG_FORMAT: "json" }),
    );

    const logs = await server.stop();

    expect(jsonLines(logs.stdout)).toContainEqual(
        expect.objectContaining({
            level: "WARN",
            message: expect.stringContaining(`${DEBUG_IGNORED_WARNING} (RUST_LOG=info)`),
        }),
    );
});

test("DEBUG=false with RUST_LOG set does not warn", async () => {
    const server = await startServer(
        withVariables(VALID_ENV, { RUST_LOG: "info", DEBUG: "false", LOG_FORMAT: "json" }),
    );

    const logs = await server.stop();

    const warnings = jsonLines(logs.stdout).filter(
        (line) => line.level === "WARN" && line.message?.includes(DEBUG_IGNORED_WARNING),
    );
    expect(warnings).toEqual([]);
});
