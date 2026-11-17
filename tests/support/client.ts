import { READ_ONLY_TOKEN, READ_WRITE_TOKEN } from "./server-env.ts";

export const OCTET_STREAM = "application/octet-stream";

export type Token = "read-write" | "read-only" | "none" | { authorization: string };

export interface Reply {
    status: number;
    headers: Headers;
    body: Uint8Array<ArrayBuffer>;
}

export const MAX_HASH_LENGTH = 128;

export function cachePath(hash: string): string {
    return `/v1/cache/${hash}`;
}

function authorizationHeader(token: Token): string | undefined {
    switch (token) {
        case "none":
            return undefined;
        case "read-write":
            return `Bearer ${READ_WRITE_TOKEN}`;
        case "read-only":
            return `Bearer ${READ_ONLY_TOKEN}`;
        default:
            return token.authorization;
    }
}

export function nxHeaders(method: string, token: Token): Record<string, string> {
    const headers: Record<string, string> = { "content-type": OCTET_STREAM };

    if (method === "GET") {
        headers.accept = OCTET_STREAM;
    }

    const authorization = authorizationHeader(token);
    if (authorization !== undefined) {
        headers.authorization = authorization;
    }

    return headers;
}

export class CacheClient {
    private readonly baseUrl: string;
    private readonly token: Token;
    private readonly timeoutMs: number;

    constructor(baseUrl: string, token: Token, timeoutMs: number) {
        this.baseUrl = baseUrl;
        this.token = token;
        this.timeoutMs = timeoutMs;
    }

    put(hash: string, body: Uint8Array<ArrayBuffer>): Promise<Reply> {
        return this.send("PUT", cachePath(hash), body);
    }

    get(hash: string): Promise<Reply> {
        return this.send("GET", cachePath(hash));
    }

    request(method: string, path: string): Promise<Reply> {
        return this.send(method, path);
    }

    private async send(
        method: string,
        path: string,
        body?: Uint8Array<ArrayBuffer>,
    ): Promise<Reply> {
        // Concatenated, not new URL(path, base): a path starting with "//" would become the host.
        const url = this.baseUrl + path;

        try {
            const response = await fetch(url, {
                method,
                headers: nxHeaders(method, this.token),
                body,
                signal: AbortSignal.timeout(this.timeoutMs),
            });

            return {
                status: response.status,
                headers: response.headers,
                body: new Uint8Array(await response.arrayBuffer()),
            };
        } catch (error) {
            throw new Error(`${method} ${url}: ${String(error)}`, {
                cause: error,
            });
        }
    }
}
