import { devNull } from "node:os";
import { inject, onTestFinished } from "vitest";
import { CacheClient, type Token } from "./client.ts";
import { LOOPBACK, freePort } from "./ports.ts";
import { type Exited, type Logs, RunningProcess, formatOutput, pollUntil } from "./process.ts";
import type { ServerEnv } from "./server-env.ts";

export const STARTUP_FAILURE_EXIT_CODE = 1;
export const BIND_FAILURE_MESSAGE = "Server error:";

// Padded to match tracing's level column, not the word ERROR inside a message.
export const ERROR_LOG_LEVEL = " ERROR ";

const START_DEADLINE_MS = 15_000;
const EXIT_DEADLINE_MS = 10_000;
const MAX_START_ATTEMPTS = 3;
const SERVER_DEFAULT_S3_TIMEOUT_SECONDS = 30;
const S3_TIMEOUT_HEADROOM_SECONDS = 10;

const NO_HOST_AWS_CONFIG = {
    AWS_EC2_METADATA_DISABLED: "true",
    AWS_CONFIG_FILE: devNull,
    AWS_SHARED_CREDENTIALS_FILE: devNull,
};

class Server {
    readonly port: number;
    private readonly process: RunningProcess;
    private readonly clientTimeoutMs: number;

    constructor(process: RunningProcess, port: number, clientTimeoutMs: number) {
        this.process = process;
        this.port = port;
        this.clientTimeoutMs = clientTimeoutMs;
    }

    client(token: Token): CacheClient {
        return new CacheClient(`http://${LOOPBACK}:${this.port}`, token, this.clientTimeoutMs);
    }

    stop(): Promise<Logs> {
        return this.process.stop();
    }
}

export async function startServer(env: ServerEnv): Promise<Server> {
    for (let attempt = 1; ; attempt++) {
        const started = await startOnce(env);
        if (started instanceof Server) {
            return started;
        }

        if (!started.lostPortRace || attempt === MAX_START_ATTEMPTS) {
            throw started.error;
        }
    }
}

interface FailedStart {
    error: Error;
    lostPortRace: boolean;
}

async function startOnce(env: ServerEnv): Promise<Server | FailedStart> {
    const launchedEnv = await launchEnv(env);
    const port = Number(launchedEnv.PORT);
    const child = RunningProcess.spawn(nxCacheAwsBinary(), launchedEnv);

    try {
        await waitUntilReady(child, port);
    } catch (error) {
        const exitedOnItsOwn = child.hasExited();
        const output = await child.stop();
        const reason = error instanceof Error ? error.message : String(error);

        return {
            error: new Error(
                `start nx-cache-aws: ${reason} (exit code ${output.code})\n${formatOutput(output)}`,
                { cause: error },
            ),
            lostPortRace: exitedOnItsOwn && output.stderr.includes(BIND_FAILURE_MESSAGE),
        };
    }

    const server = new Server(child, port, clientTimeoutMs(env));
    onTestFinished(async (context) => {
        const logs = await server.stop();
        if (context.task.result?.state === "fail") {
            console.error(formatOutput(logs));
        }
    });

    return server;
}

export async function runServerUntilExit(env: ServerEnv): Promise<Exited> {
    const child = RunningProcess.spawn(nxCacheAwsBinary(), await launchEnv(env));

    return child.waitForExit(EXIT_DEADLINE_MS);
}

function nxCacheAwsBinary(): string {
    return inject("nxCacheAwsBinary");
}

async function launchEnv(env: ServerEnv): Promise<Record<string, string>> {
    return {
        ...NO_HOST_AWS_CONFIG,
        PORT: String(await freePort()),
        // Loopback, not the default 0.0.0.0: binding it raises no Windows Firewall prompt.
        BIND_ADDRESS: LOOPBACK,
        ...env,
    };
}

function waitUntilReady(child: RunningProcess, port: number): Promise<void> {
    const readyLine = `Server running on ${LOOPBACK}:${port}`;

    return pollUntil(START_DEADLINE_MS, `no "${readyLine}" line`, () => {
        if (child.hasExited()) {
            throw new Error("nx-cache-aws exited before it was ready");
        }
        return child.outputSoFar().stdout.includes(readyLine);
    });
}

function clientTimeoutMs(env: ServerEnv): number {
    const s3TimeoutSeconds =
        env.S3_TIMEOUT === undefined ? SERVER_DEFAULT_S3_TIMEOUT_SECONDS : Number(env.S3_TIMEOUT);

    return (s3TimeoutSeconds + S3_TIMEOUT_HEADROOM_SECONDS) * 1000;
}
