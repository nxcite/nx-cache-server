import { type ChildProcess, spawn } from "node:child_process";
import { setTimeout as sleep } from "node:timers/promises";

const POLL_INTERVAL_MS = 50;

export interface Logs {
    stdout: string;
    stderr: string;
}

export interface Exited extends Logs {
    code: number | null;
    signal: NodeJS.Signals | null;
}

export class RunningProcess {
    private readonly child: ChildProcess;
    // Resolves, not rejects: an unawaited spawn failure would be an unhandled rejection.
    private readonly closed: Promise<Exited | Error>;
    private exited = false;
    private stdout = "";
    private stderr = "";

    private constructor(binary: string, child: ChildProcess) {
        this.child = child;

        child.stdout?.setEncoding("utf8").on("data", (chunk: string) => {
            this.stdout += chunk;
        });
        child.stderr?.setEncoding("utf8").on("data", (chunk: string) => {
            this.stderr += chunk;
        });
        child.once("exit", () => {
            this.exited = true;
        });

        this.closed = new Promise((resolve) => {
            child.once("error", (error) => {
                this.exited = true;
                resolve(
                    new Error(`spawn ${binary}: ${error.message}`, {
                        cause: error,
                    }),
                );
            });
            child.once("close", (code, signal) => {
                resolve({ code, signal, ...this.outputSoFar() });
            });
        });
    }

    static spawn(binary: string, env: Record<string, string>): RunningProcess {
        const child = spawn(binary, [], {
            env: withSystemRoot(env),
            stdio: ["ignore", "pipe", "pipe"],
            windowsHide: true,
        });

        return new RunningProcess(binary, child);
    }

    hasExited(): boolean {
        return this.exited;
    }

    outputSoFar(): Logs {
        return { stdout: this.stdout, stderr: this.stderr };
    }

    async waitForExit(deadlineMs: number): Promise<Exited> {
        try {
            await pollUntil(deadlineMs, "expected exit, still running", () => this.hasExited());
        } catch (timeout) {
            const output = await this.stop();
            const reason = timeout instanceof Error ? timeout.message : String(timeout);
            throw new Error(`${reason}\n${formatOutput(output)}`, { cause: timeout });
        }

        return this.closedOrThrow();
    }

    async stop(): Promise<Exited> {
        if (!this.exited) {
            this.child.kill("SIGKILL");
        }

        return this.closedOrThrow();
    }

    private async closedOrThrow(): Promise<Exited> {
        const closed = await this.closed;
        if (closed instanceof Error) {
            throw closed;
        }

        return closed;
    }
}

function withSystemRoot(env: Record<string, string>): Record<string, string> {
    const systemRoot = process.env.SYSTEMROOT;
    if (process.platform !== "win32" || systemRoot === undefined) {
        return env;
    }

    // Winsock and CNG fail without %SystemRoot%, and the server inherits no other env.
    return { ...env, SYSTEMROOT: systemRoot };
}

export async function pollUntil(
    deadlineMs: number,
    timeoutMessage: string,
    condition: () => boolean | Promise<boolean>,
): Promise<void> {
    const giveUpAt = Date.now() + deadlineMs;

    while (!(await condition())) {
        if (Date.now() >= giveUpAt) {
            throw new Error(`${timeoutMessage} after ${deadlineMs} ms`);
        }
        await sleep(POLL_INTERVAL_MS);
    }
}

export function formatOutput(output: Logs): string {
    return `--- stdout ---\n${output.stdout}\n--- stderr ---\n${output.stderr}`;
}
