import { expect } from "vitest";
import type { Reply } from "./client.ts";
import { type Exited, formatOutput } from "./process.ts";

function expectSameBytes(actual: Uint8Array, expected: Uint8Array, context: string): void {
    const offset = firstDifferingOffset(actual, expected);

    expect(
        offset,
        `${context}: body differs from byte ${offset} (got ${actual.length} bytes, expected ${expected.length})`,
    ).toBeUndefined();
}

export function expectHit(reply: Reply, expected: Uint8Array, context: string): void {
    expect(reply.status, context).toBe(200);
    expectSameBytes(reply.body, expected, context);
}

export function expectOneOf(
    actual: Uint8Array,
    candidates: readonly Uint8Array[],
    context: string,
): void {
    const matchesOne = candidates.some(
        (candidate) => firstDifferingOffset(actual, candidate) === undefined,
    );

    expect(matchesOne, `${context}: body (${actual.length} bytes) matches no candidate`).toBe(true);
}

export function expectExit(exited: Exited, expected: number): void {
    expect(
        exited.code,
        `expected exit code ${expected}, got ${exited.code} (signal ${exited.signal})\n${formatOutput(exited)}`,
    ).toBe(expected);
}

function firstDifferingOffset(actual: Uint8Array, expected: Uint8Array): number | undefined {
    const comparable = Math.min(actual.length, expected.length);

    for (let offset = 0; offset < comparable; offset++) {
        if (actual[offset] !== expected[offset]) {
            return offset;
        }
    }

    return actual.length === expected.length ? undefined : comparable;
}
