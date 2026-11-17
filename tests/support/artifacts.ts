import { randomFillSync, randomUUID } from "node:crypto";

export function uniqueHash(label: string): string {
    return `${label}-${randomUUID()}`.toLowerCase();
}

export function artifact(length: number): Uint8Array<ArrayBuffer> {
    return randomFillSync(new Uint8Array(length));
}
