import { type ChildProcess, spawn } from "node:child_process";
import { createInterface } from "node:readline";
import type { Readable } from "node:stream";
import { fileURLToPath } from "node:url";
import type { TestProject } from "vitest/node";

declare module "vitest" {
    interface ProvidedContext {
        nxCacheAwsBinary: string;
    }
}

const REPOSITORY_ROOT = fileURLToPath(new URL("../..", import.meta.url));

interface CargoMessage {
    reason?: string;
    target?: { name?: string };
    executable?: string | null;
}

export default async function setup(project: TestProject): Promise<void> {
    project.provide("nxCacheAwsBinary", await buildNxCacheAws());
}

async function buildNxCacheAws(): Promise<string> {
    const cargo = spawn(
        "cargo",
        ["build", "--locked", "--bin", "nx-cache-aws", "--message-format=json-render-diagnostics"],
        { cwd: REPOSITORY_ROOT, stdio: ["ignore", "pipe", "inherit"] },
    );

    const [executable, exitCode] = await Promise.all([
        findExecutable(cargo.stdout),
        exitCodeOf(cargo),
    ]);

    if (exitCode !== 0) {
        throw new Error(`cargo build of nx-cache-aws exited with ${exitCode}`);
    }
    if (executable === undefined) {
        throw new Error("cargo build did not report the nx-cache-aws executable");
    }

    return executable;
}

async function findExecutable(cargoStdout: Readable | null): Promise<string | undefined> {
    if (cargoStdout === null) {
        throw new Error("cargo build has no stdout pipe");
    }

    let executable: string | undefined;
    for await (const line of createInterface({ input: cargoStdout })) {
        const message = JSON.parse(line) as CargoMessage;
        if (
            message.reason === "compiler-artifact" &&
            message.target?.name === "nx-cache-aws" &&
            typeof message.executable === "string"
        ) {
            executable = message.executable;
        }
    }

    return executable;
}

function exitCodeOf(child: ChildProcess): Promise<number | null> {
    return new Promise((resolve, reject) => {
        child.once("error", (error) =>
            reject(new Error(`spawn cargo: ${error.message}`, { cause: error })),
        );
        child.once("close", resolve);
    });
}
