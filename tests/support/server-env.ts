import { LOOPBACK } from "./ports.ts";

export const READ_WRITE_TOKEN = "read-write-token";
export const READ_ONLY_TOKEN = "read-only-token";

const UNREACHABLE_S3_ENDPOINT = `http://${LOOPBACK}:1`;

export type ServerEnv = Readonly<Record<string, string>>;

export const VALID_ENV: ServerEnv = {
    S3_ENDPOINT_URL: UNREACHABLE_S3_ENDPOINT,
    S3_TIMEOUT: "2",
    AWS_ACCESS_KEY_ID: "test-access-key",
    AWS_SECRET_ACCESS_KEY: "test-secret-key",
    S3_BUCKET_NAME: "nx-cache-test",
    AWS_REGION: "us-east-1",
    SERVICE_ACCESS_TOKEN: READ_WRITE_TOKEN,
    READ_ONLY_ACCESS_TOKEN: READ_ONLY_TOKEN,
};

export function withVariables(
    env: ServerEnv,
    changes: Readonly<Record<string, string | undefined>>,
): ServerEnv {
    const changed: Record<string, string> = { ...env };

    for (const [name, value] of Object.entries(changes)) {
        if (value === undefined) {
            delete changed[name];
        } else {
            changed[name] = value;
        }
    }

    return changed;
}
