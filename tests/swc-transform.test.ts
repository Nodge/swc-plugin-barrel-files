import process from "node:process";
import path from "node:path";
import fs from "node:fs/promises";
import { describe, it, expect, afterEach } from "vitest";
import { transform } from "@swc/core";

const fixturesDir = path.resolve(__dirname, "fixtures");

interface PluginConfig {
    patterns: string[];
    aliases?: Array<{
        pattern: string;
        paths: string[];
        context?: string[];
    }>;
    debug?: boolean;
}

async function transpileWithSwc({ filename, code, config }: { filename: string; code: string; config: PluginConfig }) {
    const result = await transform(code, {
        filename,
        env: {
            targets: {
                node: process.versions.node,
            },
        },
        jsc: {
            experimental: {
                plugins: [[require.resolve("../swc_plugin_barrel_files.wasm"), config]],
            },
        },
    });
    return result.code;
}

async function file(filename: string, content: string) {
    const target = path.join(fixturesDir, filename);
    await fs.mkdir(path.dirname(target), { recursive: true });
    await fs.writeFile(target, content);
}

describe("SWC Barrel Files Transformation", () => {
    const defaultConfig: PluginConfig = {
        aliases: [
            {
                pattern: "#features/*",
                paths: [path.join(fixturesDir, "src/features/*/index.ts")],
            },
            {
                pattern: "#features/*/testing",
                paths: [path.join(fixturesDir, "src/features/*/testing.ts")],
            },
        ],
        patterns: [
            path.join(fixturesDir, "src/features/*/index.ts"),
            path.join(fixturesDir, "src/features/*/testing.ts"),
        ],
    };

    afterEach(async () => {
        await fs.rm(fixturesDir, { recursive: true, force: true });
    });

    it("should transform index file imports", async () => {
        await file(
            "src/features/some/index.ts",
            `
                export { Button } from "./components/Button";
                export { select } from "./model/selectors";
            `,
        );

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/test1.ts"),
            code: `
                import { Button, select, type SomeType } from "#features/some";
                import type { ButtonProps } from "#features/some";

                console.log(Button, select);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "../../features/some/components/Button";
          import { select } from "../../features/some/model/selectors";
          console.log(Button, select);
          "
        `);
    });

    it("should transform testing file imports", async () => {
        await file("src/features/some/testing.ts", 'export { mock } from "./api/mocks/test";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/test2.ts"),
            code: `
                import { mock, type Mock } from "#features/some/testing";

                console.log(mock);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { mock } from "../../features/some/api/mocks/test";
          console.log(mock);
          "
        `);
    });

    it("should transform barrel files with comments", async () => {
        await file(
            "src/features/comments/index.ts",
            `
                // This is a comment
                export { Component } from "./components/Component"; // End of line comment
                /* Multi-line comment
                   spanning multiple lines */
                export { reducer } from "./model/reducer";
            `,
        );

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/comments.ts"),
            code: `
                import { Component, reducer } from "#features/comments";
                console.log(Component, reducer);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Component } from "../../features/comments/components/Component";
          import { reducer } from "../../features/comments/model/reducer";
          console.log(Component, reducer);
          "
        `);
    });

    it("should find barrel file from an array of paths", async () => {
        const jsxConfig: PluginConfig = {
            aliases: [
                {
                    pattern: "#ui/*",
                    paths: [path.join(fixturesDir, "src/ui/*/index.ts"), path.join(fixturesDir, "src/ui/*/index.tsx")],
                },
            ],
            patterns: [path.join(fixturesDir, "src/ui/*/index.ts"), path.join(fixturesDir, "src/ui/*/index.tsx")],
        };

        await file("src/ui/button/index.tsx", 'export { Button } from "./Button";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/jsx.ts"),
            code: `
                import { Button } from "#ui/button";
                console.log(Button);
            `,
            config: jsxConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "../../ui/button/Button";
          console.log(Button);
          "
        `);
    });

    it("should transform imports with renamed import", async () => {
        await file("src/features/renamed/index.ts", 'export { Modal } from "./components/Modal";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/renamed.ts"),
            code: `
                import { Modal as CustomModal } from "#features/renamed";
                console.log(CustomModal);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Modal as CustomModal } from "../../features/renamed/components/Modal";
          console.log(CustomModal);
          "
        `);
    });

    it("should transform barrel file with renamed export", async () => {
        await file(
            "src/features/renamed-exports/index.ts",
            'export { setVisible as toggle } from "./model/visibility";',
        );

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/renamed-exports.ts"),
            code: `
                import { toggle } from "#features/renamed-exports";
                console.log(toggle);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { setVisible as toggle } from "../../features/renamed-exports/model/visibility";
          console.log(toggle);
          "
        `);
    });

    it("should transform barrel file with renamed import and export", async () => {
        await file(
            "src/features/renamed-exports/index.ts",
            'export { setVisible as toggle } from "./model/visibility";',
        );

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/renamed-exports.ts"),
            code: `
                import { toggle as switcher } from "#features/renamed-exports";
                console.log(switcher);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { setVisible as switcher } from "../../features/renamed-exports/model/visibility";
          console.log(switcher);
          "
        `);
    });

    it("should transform default re-exports inside barrel files", async () => {
        await file("src/features/defaults/index.ts", 'export { default as Button } from "./components/Button";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/defaults.ts"),
            code: `
                import { Button } from "#features/defaults";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import Button from "../../features/defaults/components/Button";
          console.log(Button);
          "
        `);
    });

    it("should show error for barrel files with default exports", async () => {
        await file("src/features/defaults/index.ts", `export default Button;`);

        const res = transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/defaults.ts"),
            code: `
                import Button from "#features/defaults";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        await expect(res).rejects.toThrow("E_INVALID_BARREL_FILE");
    });

    it("should show error for barrel files with wilsdcard exports", async () => {
        await file("src/features/wildcard/index.ts", 'export * from "./components/Button";');

        const res = transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/star-export.ts"),
            code: `
                import { Button } from "#features/wildcard";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        await expect(res).rejects.toThrow("E_INVALID_BARREL_FILE");
    });

    it("should show error for barrel files with namespaced exports", async () => {
        await file("src/features/namespace/index.ts", 'export * as components from "./components";');

        const res = transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/namespace.ts"),
            code: `
                import { components } from "#features/namespace";
                console.log(components);
            `,
            config: defaultConfig,
        });

        await expect(res).rejects.toThrow("E_INVALID_BARREL_FILE");
    });

    it("should show error for namespaced imports from barrel file", async () => {
        await file("src/features/f1/index.ts", 'export { Button } from "./components/Button";');

        const res = transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/namespace.ts"),
            code: `
                import * as f1 from "#features/f1";
                console.log(f1);
            `,
            config: defaultConfig,
        });

        await expect(res).rejects.toThrow("E_NO_NAMESPACE_IMPORTS");
    });

    it("should handle relative paths in patterns and aliases", async () => {
        const relativeConfig: PluginConfig = {
            aliases: [
                {
                    pattern: "#entities/*",
                    paths: ["tests/fixtures/src/entities/*/index.ts"],
                },
                {
                    pattern: "#features/*",
                    paths: ["./tests/fixtures/src/features/*/index.ts"],
                },
            ],
            patterns: ["./tests/fixtures/src/entities/*/index.ts", "tests/fixtures/src/features/*/index.ts"],
        };

        await file("src/entities/e1/index.ts", 'export { Button } from "./Button";');
        await file("src/features/f1/index.ts", 'export { toggle } from "./model";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/page.ts"),
            code: `
                import { Button } from "#entities/e1";
                import { toggle } from "#features/f1";
                console.log(Button, toggle);
            `,
            config: relativeConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "../../entities/e1/Button";
          import { toggle } from "../../features/f1/model";
          console.log(Button, toggle);
          "
        `);
    });

    it("should handle absolute paths in pattern config that match cwd", async () => {
        const absoluteConfig: PluginConfig = {
            aliases: [
                {
                    pattern: "#libs/*",
                    paths: [path.resolve(process.cwd(), "tests/fixtures/src/libs/*/index.ts")],
                },
            ],
            patterns: [path.resolve(process.cwd(), "tests/fixtures/src/libs/*/index.ts")],
        };

        await file("src/libs/utils/index.ts", 'export { formatDate } from "./date";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/absolute.ts"),
            code: `
                import { formatDate } from "#libs/utils";
                console.log(formatDate);
            `,
            config: absoluteConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { formatDate } from "../../libs/utils/date";
          console.log(formatDate);
          "
        `);
    });

    it("should show error for absolute paths in pattern config that don't match cwd", async () => {
        const nonMatchingConfig: PluginConfig = {
            aliases: [
                {
                    pattern: "#external/*",
                    paths: ["/non-existent-path/external/*/index.ts"],
                },
            ],
            patterns: ["/non-existent-path/external/*/index.ts"],
        };

        await file("src/external/ui/index.ts", 'export { Component } from "./Component";');

        const res = transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/non-matching.ts"),
            code: `
                import { Component } from "#external/ui";
                console.log(Component);
            `,
            config: nonMatchingConfig,
        });

        await expect(res).rejects.toThrow();
    });

    it("should handle imports from non-existent files", async () => {
        await file("src/features/existing/index.ts", 'export { Component } from "./components/Component";');

        const res = transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/non-existent.ts"),
            code: `
                import { Component } from "#features/non-existent";
                console.log(Component);
            `,
            config: defaultConfig,
        });

        await expect(res).rejects.toThrow("E_BARREL_FILE_NOT_FOUND");
    });

    it("should handle imports from barrel files without required exports", async () => {
        await file("src/features/f1/index.ts", 'export { Component } from "./components/Component";');

        const res = transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/page.ts"),
            code: `
                import { Action } from "#features/f1";
                console.log(Action);
            `,
            config: defaultConfig,
        });

        await expect(res).rejects.toThrow("E_UNRESOLVED_EXPORTS");
    });

    it("should handle imports from barrel files with code", async () => {
        await file("src/features/with-code/index.ts", 'export const VERSION = "1.0.0";');

        const res = transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/with-code.ts"),
            code: `
                import { VERSION } from "#features/with-code";
                console.log(VERSION);
            `,
            config: defaultConfig,
        });

        await expect(res).rejects.toThrow("E_INVALID_BARREL_FILE");
    });

    it("should handle imports from files that couldn't be parsed", async () => {
        await file(
            "src/features/invalid/index.ts",
            `
                export { Button } from "./components/Button";
                // Syntax error
                const = "invalid";
            `,
        );

        const res = transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/invalid.ts"),
            code: `
                import { Button } from "#features/invalid";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        await expect(res).rejects.toThrow("E_FILE_PARSE");
    });

    it("should handle re-exports using absolute paths", async () => {
        await file("src/features/re-export/index.ts", 'export { Button } from "/root/src/components/Button";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/re-export.ts"),
            code: `
                import { Button } from "#features/re-export";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "/root/src/components/Button";
          console.log(Button);
          "
        `);
    });

    it("should handle re-exports from packages", async () => {
        await file("src/features/re-export/index.ts", 'export { Button } from "ui-lib";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/re-export.ts"),
            code: `
                import { Button } from "#features/re-export";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "ui-lib";
          console.log(Button);
          "
        `);
    });

    it("should handle absolute paths in imports", async () => {
        await file("src/features/absolute/index.ts", 'export { Button } from "./components/Button";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/absolute.ts"),
            code: `
                import { Button } from "${path.join(fixturesDir, "src/features/absolute/index.ts")}";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "../../features/absolute/components/Button";
          console.log(Button);
          "
        `);
    });

    it("should ignore absolute paths outside cwd", async () => {
        await file("src/features/absolute/index.ts", 'export { Button } from "./components/Button";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/absolute.ts"),
            code: `
                import { Button } from "/root/file.ts";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "/root/file.ts";
          console.log(Button);
          "
        `);
    });

    it("should use context to resolve aliases", async () => {
        await file("lib-a/src/features/some/index.ts", 'export { Button } from "./components/Button";');
        await file("lib-b/src/features/some/index.ts", 'export { select } from "./model/selectors";');

        const config: PluginConfig = {
            aliases: [
                {
                    pattern: "#features/*",
                    paths: [path.join(fixturesDir, "lib-a/src/features/*/index.ts")],
                    context: [path.join(fixturesDir, "lib-a")],
                },
                {
                    pattern: "#features/*",
                    paths: [path.join(fixturesDir, "lib-b/src/features/*/index.ts")],
                    context: [path.join(fixturesDir, "lib-b")],
                },
            ],
            patterns: [
                path.join(fixturesDir, "lib-a/src/features/*/index.ts"),
                path.join(fixturesDir, "lib-b/src/features/*/index.ts"),
            ],
        };

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "lib-a/src/pages/test/test1.ts"),
            code: `
                import { Button } from "#features/some";
                console.log(Button);
            `,
            config,
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "../../features/some/components/Button";
          console.log(Button);
          "
        `);

        const outputCode2 = await transpileWithSwc({
            filename: path.join(fixturesDir, "lib-b/src/pages/test/test1.ts"),
            code: `
                import { select } from "#features/some";
                console.log(select);
            `,
            config,
        });

        expect(outputCode2).toMatchInlineSnapshot(`
          "import { select } from "../../features/some/model/selectors";
          console.log(select);
          "
        `);
    });

    it("should match barrel files without aliases", async () => {
        await file("src/features/some/index.ts", 'export { Button } from "./components/Button";');

        const outputCode = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/test1.ts"),
            code: `
                import { Button } from "../../features/some/index.ts";
                console.log(Button);
            `,
            config: {
                aliases: [],
                patterns: [path.join(fixturesDir, "src/features/*/index.ts")],
            },
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "../../features/some/components/Button";
          console.log(Button);
          "
        `);
    });

    it("should skip files outside cwd", async () => {
        const outputCode = await transpileWithSwc({
            filename: "/dev/null",
            code: `
                import { Button } from "/dev/null";
                console.log(Button);
            `,
            config: {
                aliases: [],
                patterns: [],
            },
        });

        expect(outputCode).toMatchInlineSnapshot(`
          "import { Button } from "/dev/null";
          console.log(Button);
          "
        `);
    });
});
