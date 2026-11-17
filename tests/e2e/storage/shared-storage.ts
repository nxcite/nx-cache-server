import { inject } from "vitest";
import { READ_ONLY_TOKEN, READ_WRITE_TOKEN, type ServerEnv } from "../../support/server-env.ts";
import { S3Admin } from "./s3-admin.ts";

let admin: S3Admin | undefined;

export function sharedBucket(): string {
    return startedStorage().bucket;
}

export function storageAdmin(): S3Admin {
    const { endpoint, region, credentials } = startedStorage();
    admin ??= new S3Admin(endpoint, region, credentials);

    return admin;
}

export function e2eServerEnv(): ServerEnv {
    const { endpoint, region, bucket, credentials } = startedStorage();

    return {
        S3_ENDPOINT_URL: endpoint,
        AWS_ACCESS_KEY_ID: credentials.accessKeyId,
        AWS_SECRET_ACCESS_KEY: credentials.secretAccessKey,
        S3_BUCKET_NAME: bucket,
        AWS_REGION: region,
        SERVICE_ACCESS_TOKEN: READ_WRITE_TOKEN,
        READ_ONLY_ACCESS_TOKEN: READ_ONLY_TOKEN,
    };
}

function startedStorage() {
    const storage = inject("e2eStorage");
    if (!storage.ok) {
        throw new Error(`the e2e storage failed to start: ${storage.error}`);
    }

    return storage;
}
