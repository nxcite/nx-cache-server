import pRetry from "p-retry";
import type { StartedTestContainer } from "testcontainers";
import type { TestProject } from "vitest/node";
import { LOOPBACK } from "../../support/ports.ts";
import { S3Admin } from "./s3-admin.ts";
import {
    S3_PORT,
    SEAWEEDFS_CREDENTIALS,
    SEAWEEDFS_IMAGE,
    SEAWEEDFS_REGION,
    startSeaweedFs,
} from "./seaweedfs.ts";

declare module "vitest" {
    interface ProvidedContext {
        e2eStorage:
            | {
                  ok: true;
                  endpoint: string;
                  region: string;
                  bucket: string;
                  credentials: { accessKeyId: string; secretAccessKey: string };
              }
            | { ok: false; error: string };
    }
}

const BUCKET_CREATION_DEADLINE_MS = 30_000;
const RETRY_INTERVAL_MS = 100;

export default async function setup(project: TestProject): Promise<() => Promise<void>> {
    const bucket = "nx-cache-e2e";
    const image = process.env.NX_CACHE_E2E_IMAGE ?? SEAWEEDFS_IMAGE;
    console.log(`e2e storage: starting ${image}`);

    let container: StartedTestContainer | undefined;
    try {
        container = await startSeaweedFs(image);

        // Not getHost(): it can be "localhost", and the port is published on 127.0.0.1 only.
        const endpoint = `http://${LOOPBACK}:${container.getMappedPort(S3_PORT)}`;
        const admin = new S3Admin(endpoint, SEAWEEDFS_REGION, SEAWEEDFS_CREDENTIALS);
        await createBucketOnceReachable(admin, bucket);

        project.provide("e2eStorage", {
            ok: true,
            endpoint,
            region: SEAWEEDFS_REGION,
            bucket,
            credentials: SEAWEEDFS_CREDENTIALS,
        });
    } catch (error) {
        // Not thrown: a failing globalSetup would abort the offline project too.
        project.provide("e2eStorage", { ok: false, error: String(error) });
    }

    return async () => {
        await container?.stop();
    };
}

async function createBucketOnceReachable(admin: S3Admin, bucket: string): Promise<void> {
    try {
        await pRetry(() => admin.createBucket(bucket), {
            retries: Infinity,
            factor: 1,
            minTimeout: RETRY_INTERVAL_MS,
            maxRetryTime: BUCKET_CREATION_DEADLINE_MS,
        });
    } catch (lastError) {
        const reason = lastError instanceof Error ? lastError.message : String(lastError);
        throw new Error(
            `bucket ${bucket} not created within ${BUCKET_CREATION_DEADLINE_MS} ms; last error: ${reason}`,
            { cause: lastError },
        );
    }
}
