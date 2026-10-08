import { GenericContainer, type StartedTestContainer, Wait } from "testcontainers";
import { LOOPBACK } from "../../support/ports.ts";

export const SEAWEEDFS_IMAGE = "chrislusf/seaweedfs:4.48";
export const SEAWEEDFS_REGION = "us-east-1";
export const SEAWEEDFS_CREDENTIALS = {
    accessKeyId: "testkey",
    secretAccessKey: "testsecret",
};
export const S3_PORT = 8333;

const CLEANUP_LABEL = { "nx-cache-test": "e2e" };
const S3_CONFIG_PATH = "/etc/s3.json";
const S3_IDENTITIES_CONFIG = JSON.stringify({
    identities: [
        {
            name: "test",
            credentials: [
                {
                    accessKey: SEAWEEDFS_CREDENTIALS.accessKeyId,
                    secretKey: SEAWEEDFS_CREDENTIALS.secretAccessKey,
                },
            ],
            actions: ["Admin", "Read", "Write", "List"],
        },
    ],
});

class LoopbackContainer extends GenericContainer {
    // withExposedPorts() publishes on 0.0.0.0 and has no host-IP option.
    withLoopbackPort(port: number): this {
        this.withExposedPorts(port);
        this.hostConfig.PortBindings = {
            ...this.hostConfig.PortBindings,
            [`${port}/tcp`]: [{ HostIp: LOOPBACK, HostPort: "0" }],
        };

        return this;
    }
}

export function startSeaweedFs(image: string): Promise<StartedTestContainer> {
    return new LoopbackContainer(image)
        .withCommand([
            "server",
            "-s3",
            `-s3.port=${S3_PORT}`,
            `-s3.config=${S3_CONFIG_PATH}`,
            // Else a PUT creates the missing bucket, and the missing-bucket tests prove nothing.
            "-s3.autoCreateBucket=false",
        ])
        .withCopyContentToContainer([
            // World-readable: the image's entrypoint drops to a non-root user.
            { content: S3_IDENTITIES_CONFIG, target: S3_CONFIG_PATH, mode: 0o644 },
        ])
        .withLoopbackPort(S3_PORT)
        .withLabels(CLEANUP_LABEL)
        .withWaitStrategy(Wait.forLogMessage(/Start Seaweed S3 API Server/))
        .withStartupTimeout(60_000)
        .start();
}
