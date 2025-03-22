import process from "node:process";
import { describe, it, expect } from "vitest";
import { transform } from "@swc/core";
import path from "node:path";

const fixturesDir = path.resolve(__dirname, "fixtures");

async function transpileWithSwc({ filename, code }: { filename: string; code: string }) {
    const result = await transform(code, {
        filename,
        env: {
            targets: {
                node: process.versions.node,
            },
        },
        jsc: {
            experimental: {
                plugins: [
                    [
                        require.resolve("../swc-plugin/target/wasm32-wasi/release/swc_plugin_barrel_files.wasm"),
                        {
                            rules: [
                                {
                                    pattern: "#features/*",
                                    paths: [path.join(fixturesDir, "src/features/*/index.ts")],
                                },
                                {
                                    pattern: "#features/*/testing",
                                    paths: ["tests/fixtures/src/features/*/testing.ts"],
                                },
                            ],
                        },
                    ],
                ],
            },
        },
    });
    return result.code;
}

describe("SWC Barrel Files Transformation", () => {
    it("it should transform index file imports", async () => {
        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/test1.ts"),
            code: `
                import { Button, select } from "#features/some";

                console.log(Button, select);
            `,
        });

        expect(outputCode).toMatchInlineSnapshot(`
            import { Button } from '../../features/some/components/Button';
            import { select } from '../../features/some/model/selector';
            console.log(Button, select);
        `);
    });

    it("it should transform testing file imports", async () => {
        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/test2.ts"),
            code: `
                import { mock } from "#features/some/testing";

                console.log(mock);
            `,
        });

        expect(outputCode).toMatchInlineSnapshot(`
            import { mock } from '../../features/some/api/mocks/test';
            console.log(mock);
        `);
    });
});
