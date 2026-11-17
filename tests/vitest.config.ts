import { defineConfig } from "vitest/config";

export default defineConfig({
    test: {
        globalSetup: ["./support/build-binary.ts"],
        // Node's fetch honours HTTP_PROXY when NODE_USE_ENV_PROXY=1, even for 127.0.0.1.
        // Both spellings: the developer's environment may already set either one.
        env: { NO_PROXY: "127.0.0.1", no_proxy: "127.0.0.1" },
        projects: [
            {
                extends: true,
                test: {
                    name: "offline",
                    include: ["offline/**/*.test.ts"],
                    testTimeout: 30_000,
                },
            },
            {
                extends: true,
                test: {
                    name: "e2e",
                    include: ["e2e/**/*.test.ts"],
                    globalSetup: ["./e2e/storage/global-setup.ts"],
                    testTimeout: 90_000,
                },
            },
        ],
    },
});
