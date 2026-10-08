import {
    BucketAlreadyExists,
    BucketAlreadyOwnedByYou,
    CreateBucketCommand,
    DeleteBucketCommand,
    DeleteObjectCommand,
    ListObjectsV2Command,
    S3Client,
} from "@aws-sdk/client-s3";

export class S3Admin {
    private readonly client: S3Client;

    constructor(
        endpoint: string,
        region: string,
        credentials: { accessKeyId: string; secretAccessKey: string },
    ) {
        this.client = new S3Client({
            endpoint,
            region,
            forcePathStyle: true,
            credentials,
            // One bounded attempt per call: the caller, not the SDK, owns retries and deadlines.
            maxAttempts: 1,
            requestHandler: {
                connectionTimeout: 2_000,
                requestTimeout: 10_000,
            },
        });
    }

    async createBucket(name: string): Promise<void> {
        try {
            await this.client.send(new CreateBucketCommand({ Bucket: name }));
        } catch (error) {
            if (error instanceof BucketAlreadyOwnedByYou || error instanceof BucketAlreadyExists) {
                return;
            }

            throw error;
        }
    }

    async deleteBucket(name: string): Promise<void> {
        // SeaweedFS deletes a non-empty bucket, but AWS answers BucketNotEmpty.
        await this.emptyBucket(name);
        await this.client.send(new DeleteBucketCommand({ Bucket: name }));
    }

    async deleteObject(bucket: string, key: string): Promise<void> {
        await this.client.send(new DeleteObjectCommand({ Bucket: bucket, Key: key }));
    }

    private async emptyBucket(bucket: string): Promise<void> {
        for (;;) {
            const listing = await this.client.send(new ListObjectsV2Command({ Bucket: bucket }));
            const keys = (listing.Contents ?? []).flatMap((object) =>
                object.Key === undefined ? [] : [object.Key],
            );

            if (keys.length === 0) {
                return;
            }

            await Promise.all(keys.map((key) => this.deleteObject(bucket, key)));
        }
    }
}
