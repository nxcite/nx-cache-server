import { type Server, createServer } from "node:net";

export const LOOPBACK = "127.0.0.1";

export async function freePort(): Promise<number> {
    await using heldPort = await holdPort();
    return heldPort.port;
}

interface HeldPort extends AsyncDisposable {
    port: number;
}

export async function holdPort(): Promise<HeldPort> {
    const listener = createServer();

    await new Promise<void>((resolve, reject) => {
        listener.once("error", reject);
        listener.listen(0, LOOPBACK, resolve);
    });

    return {
        port: boundPort(listener),
        [Symbol.asyncDispose]: () => closeListener(listener),
    };
}

export function boundPort(listener: Server): number {
    const address = listener.address();
    if (address === null || typeof address === "string") {
        throw new Error(`expected a TCP address, got ${String(address)}`);
    }

    return address.port;
}

function closeListener(listener: Server): Promise<void> {
    return new Promise((resolve, reject) => {
        listener.close((error) => (error ? reject(error) : resolve()));
    });
}
