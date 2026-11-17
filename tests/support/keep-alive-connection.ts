import { once } from "node:events";
import { Agent, type ClientRequest, type IncomingMessage, request } from "node:http";
import { onTestFinished } from "vitest";
import { type Token, cachePath, nxHeaders } from "./client.ts";
import { LOOPBACK } from "./ports.ts";

// Exceeds the OS socket buffers, so the client can't finish sending unless the server reads.
export const IN_FLIGHT_UPLOAD_LENGTH = 8 * 1024 * 1024;

const IO_TIMEOUT_MS = 10_000;
const ZERO_CHUNK = Buffer.alloc(64 * 1024);

export interface ConnectionReply {
    status: number;
    reusedConnection: boolean;
}

// node:http, not fetch: fetch can't half-close mid-body or report socket reuse.
export class KeepAliveConnection {
    private readonly agent = new Agent({ keepAlive: true, maxSockets: 1 });
    private readonly port: number;

    constructor(port: number) {
        this.port = port;
        onTestFinished(() => this.agent.destroy());
    }

    // Rejects if the server hangs up mid-body, even after answering.
    putStreaming(hash: string, token: Token, length: number): Promise<ConnectionReply> {
        return this.send("PUT", cachePath(hash), token, length, async (req) => {
            await writeZeros(req, length);
            req.end();
        });
    }

    putTruncated(
        hash: string,
        token: Token,
        declaredLength: number,
        sentLength: number,
    ): Promise<ConnectionReply> {
        return this.send("PUT", cachePath(hash), token, declaredLength, async (req) => {
            await writeZeros(req, sentLength);
            req.socket?.end();
        });
    }

    get(path: string, token: Token): Promise<ConnectionReply> {
        return this.send("GET", path, token, 0, async (req) => {
            req.end();
        });
    }

    private async send(
        method: string,
        path: string,
        token: Token,
        contentLength: number,
        writeBody: (req: ClientRequest) => Promise<void>,
    ): Promise<ConnectionReply> {
        const req = request({
            agent: this.agent,
            host: LOOPBACK,
            port: this.port,
            method,
            path,
            headers: { ...nxHeaders(method, token), "content-length": contentLength },
            timeout: IO_TIMEOUT_MS,
        });
        req.on("timeout", () =>
            req.destroy(new Error(`no socket activity for ${IO_TIMEOUT_MS} ms`)),
        );

        const [responseEvent] = await Promise.all([
            once(req, "response") as Promise<[IncomingMessage]>,
            once(req, "socket").then(() => writeBody(req)),
        ]);
        const [response] = responseEvent;
        response.resume();
        await once(response, "end");

        if (response.statusCode === undefined) {
            throw new Error(`${method} ${path}: response without a status code`);
        }

        return { status: response.statusCode, reusedConnection: req.reusedSocket };
    }
}

async function writeZeros(req: ClientRequest, length: number): Promise<void> {
    for (let sent = 0; sent < length; sent += ZERO_CHUNK.length) {
        const chunk = ZERO_CHUNK.subarray(0, Math.min(ZERO_CHUNK.length, length - sent));
        if (!req.write(chunk)) {
            await drainedOrClosed(req);
        }
    }
}

// "drain" never fires once the socket is gone, and neither does the idle timeout.
async function drainedOrClosed(req: ClientRequest): Promise<void> {
    const stopListening = new AbortController();
    const { signal } = stopListening;

    try {
        await Promise.race([once(req, "drain", { signal }), once(req, "close", { signal })]);
    } catch (error) {
        throw closedWhileUploading(error);
    } finally {
        stopListening.abort();
    }

    if (req.destroyed) {
        throw closedWhileUploading(undefined);
    }
}

function closedWhileUploading(cause: unknown): Error {
    return new Error("connection closed while the client was still uploading", { cause });
}
