import process from "node:process";
import { describe, it, expect, beforeEach, vi } from "vitest";
// import { transformFile } from "@swc/core";
import { Volume, createFsFromVolume } from "memfs";

// Create a virtual file system
const vol = Volume.fromJSON({});
const memfs = createFsFromVolume(vol);

// Mock the fs module
vi.mock("fs", () => memfs);
vi.mock("fs/promises", () => memfs.promises);

async function transpileWithSwc(filename: string) {
    const { transformFile } = await import("@swc/core");
    const result = await transformFile(filename, {
        env: {
            targets: {
                node: process.versions.node,
            },
        },
        jsc: {
            parser: {
                syntax: "ecmascript",
                jsx: true,
            },
            experimental: {
                plugins: [
                    // [require.resolve("../swc-plugin/target/wasm32-wasi/release/swc_plugin_barrel_files.wasm"), {}],
                ],
            },
        },
    });

    return result.code;
}

describe("SWC Barrel Files Transformation", () => {
    beforeEach(() => {
        vol.reset();

        vol.mkdirSync("/src/features/some/components", { recursive: true });
        vol.mkdirSync("/src/features/some/model", { recursive: true });
        vol.mkdirSync("/src/features/some/api/mocks", { recursive: true });

        vol.writeFileSync(
            "/src/features/some/index.ts",
            `export { Button } from "./components/Button";
export { select } from "./model/selector";`,
        );

        vol.writeFileSync("/src/features/some/testing.ts", `export { mock } from "./api/mocks/test";`);

        vol.writeFileSync("/src/features/some/components/Button.ts", `export const Button = () => {};`);

        vol.writeFileSync("/src/features/some/model/selector.ts", `export const select = () => {};`);

        vol.writeFileSync("/src/features/some/api/mocks/test.ts", `export const mock = () => {};`);
    });

    it("it should transform index file imports", async () => {
        vol.mkdirSync("/src/features/test", { recursive: true });
        vol.writeFileSync("/src/features/test/test.ts", `import { Button, select } from '#features/some';`);

        const outputCode = await transpileWithSwc("/src/features/test/test.ts");

        expect(outputCode).toMatchInlineSnapshot(`
            import { Button } from '../features/some/components/Button';
            import { select } from '../features/some/model/selector';
        `);
    });

    it("it should transform testing file imports", async () => {
        vol.mkdirSync("/src/features/test", { recursive: true });
        vol.writeFileSync("/src/features/test/test.ts", `import { mock } from '#features/some/testing';`);

        const outputCode = await transpileWithSwc("/src/features/test/test.ts");

        expect(outputCode).toMatchInlineSnapshot(`
            import { mock } from '../features/some/api/mocks/test';
        `);
    });
});
