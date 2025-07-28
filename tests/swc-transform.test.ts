import process from "node:process";
import path from "node:path";
import fs from "node:fs/promises";
import { spawn } from "node:child_process";
import { describe, it, expect, afterEach } from "vitest";

const fixturesDir = path.resolve(__dirname, "fixtures");

interface PluginConfig {
    patterns: string[];
    aliases?: Array<{
        pattern: string;
        paths: string[];
        context?: string[];
    }>;
    symlinks?: Record<string, string>;
    debug?: boolean;
    unsupported_import_mode?: "error" | "warn" | "off";
    invalid_barrel_mode?: "error" | "warn" | "off";
}

interface CompilationOptions {
    filename: string;
    code: string;
    config: PluginConfig;
    cjs?: boolean;
}

interface TransformResult {
    code: string;
    exitCode: number;
    stdout: string;
    stderr: string;
}

async function transpileWithSwc({ filename, code, config, cjs }: CompilationOptions): Promise<TransformResult> {
    return new Promise((resolve, reject) => {
        const scriptContent = `
const { transform } = require("@swc/core");

const config = ${JSON.stringify(config)};
const filename = ${JSON.stringify(filename)};
const cjs = ${JSON.stringify(cjs)};
const code = ${JSON.stringify(code)};

async function run() {
    const result = await transform(code, {
        filename,
        env: {
            targets: {
                node: process.versions.node,
            },
        },
        jsc: {
            experimental: {
                plugins: [[require.resolve("./swc_plugin_barrel_files.wasm"), config]],
            },
        },
        module: {
            type: cjs ? "commonjs" : "es6",
        },
    });

    console.log("__SWC_RESULT_START__");
    console.log(JSON.stringify(result.code));
    console.log("__SWC_RESULT_END__");
}

run().catch(error => {
    console.error(error.message);
    process.exit(1);
});
        `;

        const child = spawn("node", ["-e", scriptContent], {
            stdio: ["pipe", "pipe", "pipe"],
            cwd: process.cwd(),
        });

        let stdout = "";
        let stderr = "";
        let transformResult = "";

        child.stdout.on("data", (data) => {
            const output = data.toString();
            stdout += output;

            const resultMatch = stdout.match(/__SWC_RESULT_START__\n(.*?)\n__SWC_RESULT_END__/s);
            if (resultMatch) {
                try {
                    transformResult = JSON.parse(resultMatch[1]);
                } catch (e) {
                    transformResult = resultMatch[1];
                }
            }
        });

        child.stderr.on("data", (data) => {
            stderr += data.toString();
        });

        child.on("close", (code) => {
            if (code !== 0) {
                resolve({
                    exitCode: code ?? 0,
                    code: transformResult,
                    stdout: cleanupOutput(stdout.replace(/__SWC_RESULT_START__.*?__SWC_RESULT_END__/s, "")),
                    stderr: cleanupOutput(stderr),
                });
            } else {
                resolve({
                    exitCode: code ?? 0,
                    code: transformResult,
                    stdout: cleanupOutput(stdout.replace(/__SWC_RESULT_START__.*?__SWC_RESULT_END__/s, "")),
                    stderr: cleanupOutput(stderr),
                });
            }
        });

        child.on("error", (error) => {
            reject(error);
        });
    });
}

function cleanupOutput(output: string) {
    return output
        .replaceAll(process.cwd(), "/cwd")
        .replace(/(\s)column: \d+/, "$1column: ?")
        .trim();
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

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/test1.ts"),
            code: `
                import { Button, select, type SomeType } from "#features/some";
                import type { ButtonProps } from "#features/some";

                console.log(Button, select);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "../../features/some/components/Button";
          import { select } from "../../features/some/model/selectors";
          console.log(Button, select);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should transform testing file imports", async () => {
        await file("src/features/some/testing.ts", 'export { mock } from "./api/mocks/test";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/test2.ts"),
            code: `
                import { mock, type Mock } from "#features/some/testing";

                console.log(mock);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { mock } from "../../features/some/api/mocks/test";
          console.log(mock);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
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

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/comments.ts"),
            code: `
                import { Component, reducer } from "#features/comments";
                console.log(Component, reducer);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Component } from "../../features/comments/components/Component";
          import { reducer } from "../../features/comments/model/reducer";
          console.log(Component, reducer);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
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

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/jsx.ts"),
            code: `
                import { Button } from "#ui/button";
                console.log(Button);
            `,
            config: jsxConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "../../ui/button/Button";
          console.log(Button);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should transform imports with renamed import", async () => {
        await file("src/features/renamed/index.ts", 'export { Modal } from "./components/Modal";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/renamed.ts"),
            code: `
                import { Modal as CustomModal } from "#features/renamed";
                console.log(CustomModal);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Modal as CustomModal } from "../../features/renamed/components/Modal";
          console.log(CustomModal);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should transform barrel file with renamed export", async () => {
        await file(
            "src/features/renamed-exports/index.ts",
            'export { setVisible as toggle } from "./model/visibility";',
        );

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/renamed-exports.ts"),
            code: `
                import { toggle } from "#features/renamed-exports";
                console.log(toggle);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { setVisible as toggle } from "../../features/renamed-exports/model/visibility";
          console.log(toggle);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should transform barrel file with renamed import and export", async () => {
        await file(
            "src/features/renamed-exports/index.ts",
            'export { setVisible as toggle } from "./model/visibility";',
        );

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/renamed-exports.ts"),
            code: `
                import { toggle as switcher } from "#features/renamed-exports";
                console.log(switcher);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { setVisible as switcher } from "../../features/renamed-exports/model/visibility";
          console.log(switcher);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should transform default re-exports inside barrel files", async () => {
        await file("src/features/defaults/index.ts", 'export { default as Button } from "./components/Button";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/defaults.ts"),
            code: `
                import { Button } from "#features/defaults";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import Button from "../../features/defaults/components/Button";
          console.log(Button);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should show error for barrel files with default exports", async () => {
        await file("src/features/defaults/index.ts", `export default Button;`);

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/defaults.ts"),
            code: `
                import Button from "#features/defaults";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`""`);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`
          "x Error processing barrel import: E_INVALID_BARREL_FILE: Invalid barrel file /cwd/tests/fixtures/src/features/defaults/index.ts: Barrel file contains non-export code: Default export expressions are not allowed in barrel files
             ,-[/cwd/tests/fixtures/src/pages/test/defaults.ts:2:1]
           1 | 
           2 |                 import Button from "#features/defaults";
             :                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
           3 |                 console.log(Button);
           4 |             
             \`----"
        `);
    });

    it("should show error for barrel files with wilsdcard exports", async () => {
        await file("src/features/wildcard/index.ts", 'export * from "./components/Button";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/star-export.ts"),
            code: `
                import { Button } from "#features/wildcard";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`""`);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`
          "x Error processing barrel import: E_INVALID_BARREL_FILE: Invalid barrel file /cwd/tests/fixtures/src/features/wildcard/index.ts: Wildcard exports are not supported in barrel files: Wildcard exports are not allowed in barrel files
             ,-[/cwd/tests/fixtures/src/pages/test/star-export.ts:2:1]
           1 | 
           2 |                 import { Button } from "#features/wildcard";
             :                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
           3 |                 console.log(Button);
           4 |             
             \`----"
        `);
    });

    it("should show error for barrel files with namespaced exports", async () => {
        await file("src/features/namespace/index.ts", 'export * as components from "./components";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/namespace.ts"),
            code: `
                import { components } from "#features/namespace";
                console.log(components);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`""`);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`
          "x Error processing barrel import: E_INVALID_BARREL_FILE: Invalid barrel file /cwd/tests/fixtures/src/features/namespace/index.ts: Namespace exports are not supported in barrel files: export * as components from './components'
             ,-[/cwd/tests/fixtures/src/pages/test/namespace.ts:2:1]
           1 | 
           2 |                 import { components } from "#features/namespace";
             :                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
           3 |                 console.log(components);
           4 |             
             \`----"
        `);
    });

    it("should show error for namespaced imports from barrel file", async () => {
        await file("src/features/f1/index.ts", 'export { Button } from "./components/Button";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/namespace.ts"),
            code: `
                import * as f1 from "#features/f1";
                console.log(f1);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`""`);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`
          "x Error processing barrel import: E_NO_NAMESPACE_IMPORTS: Namespace imports are not supported for barrel file optimization
             ,-[/cwd/tests/fixtures/src/pages/test/namespace.ts:2:1]
           1 | 
           2 |                 import * as f1 from "#features/f1";
             :                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
           3 |                 console.log(f1);
           4 |             
             \`----"
        `);
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

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/page.ts"),
            code: `
                import { Button } from "#entities/e1";
                import { toggle } from "#features/f1";
                console.log(Button, toggle);
            `,
            config: relativeConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "../../entities/e1/Button";
          import { toggle } from "../../features/f1/model";
          console.log(Button, toggle);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
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

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/absolute.ts"),
            code: `
                import { formatDate } from "#libs/utils";
                console.log(formatDate);
            `,
            config: absoluteConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { formatDate } from "../../libs/utils/date";
          console.log(formatDate);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
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

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/non-matching.ts"),
            code: `
                import { Component } from "#external/ui";
                console.log(Component);
            `,
            config: nonMatchingConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`""`);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`
          "thread '<unnamed>' panicked at src/lib.rs:44:61:
          Error creating visitor: "E_INVALID_FILE_PATH: Absolute paths not starting with cwd are not supported: /non-existent-path/external/*/index.ts"
          note: run with \`RUST_BACKTRACE=1\` environment variable to display a backtrace
          plugin

            x failed to invoke plugin on 'Some("/cwd/tests/fixtures/src/pages/test/non-matching.ts")'"
        `);
    });

    it("should handle imports from non-existent files", async () => {
        await file("src/features/existing/index.ts", 'export { Component } from "./components/Component";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/non-existent.ts"),
            code: `
                import { Component } from "#features/non-existent";
                console.log(Component);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`""`);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`
          "x Error processing barrel import: E_BARREL_FILE_NOT_FOUND: Could not resolve barrel file for import alias #features/non-existent
             ,-[/cwd/tests/fixtures/src/pages/test/non-existent.ts:2:1]
           1 | 
           2 |                 import { Component } from "#features/non-existent";
             :                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
           3 |                 console.log(Component);
           4 |             
             \`----"
        `);
    });

    it("should handle imports from barrel files without required exports", async () => {
        await file("src/features/f1/index.ts", 'export { Component } from "./components/Component";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/page.ts"),
            code: `
                import { Action } from "#features/f1";
                console.log(Action);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`""`);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`
          "x Error processing barrel import: E_UNRESOLVED_EXPORTS: The following exports were not found in the barrel file /cwd/tests/fixtures/src/features/f1/index.ts: Action
             ,-[/cwd/tests/fixtures/src/pages/test/page.ts:2:1]
           1 | 
           2 |                 import { Action } from "#features/f1";
             :                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
           3 |                 console.log(Action);
           4 |             
             \`----"
        `);
    });

    it("should handle imports from barrel files with code", async () => {
        await file("src/features/with-code/index.ts", 'export const VERSION = "1.0.0";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/with-code.ts"),
            code: `
                import { VERSION } from "#features/with-code";
                console.log(VERSION);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`""`);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`
          "x Error processing barrel import: E_INVALID_BARREL_FILE: Invalid barrel file /cwd/tests/fixtures/src/features/with-code/index.ts: Barrel file contains non-export code: Variable declarations are not allowed in barrel files
             ,-[/cwd/tests/fixtures/src/pages/test/with-code.ts:2:1]
           1 | 
           2 |                 import { VERSION } from "#features/with-code";
             :                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
           3 |                 console.log(VERSION);
           4 |             
             \`----"
        `);
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

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/invalid.ts"),
            code: `
                import { Button } from "#features/invalid";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`""`);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`
          "x Error processing barrel import: E_FILE_PARSE: Failed to parse file: Error { error: (118..119, Unexpected { got: "=", expected: "yield, an identifier, [ or {" }) }
             ,-[/cwd/tests/fixtures/src/pages/test/invalid.ts:2:1]
           1 | 
           2 |                 import { Button } from "#features/invalid";
             :                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
           3 |                 console.log(Button);
           4 |             
             \`----"
        `);
    });

    it("should handle re-exports using absolute paths", async () => {
        await file("src/features/re-export/index.ts", 'export { Button } from "/root/src/components/Button";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/re-export.ts"),
            code: `
                import { Button } from "#features/re-export";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "/root/src/components/Button";
          console.log(Button);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should handle re-exports from packages", async () => {
        await file("src/features/re-export/index.ts", 'export { Button } from "ui-lib";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/re-export.ts"),
            code: `
                import { Button } from "#features/re-export";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "ui-lib";
          console.log(Button);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should handle absolute paths in imports", async () => {
        await file("src/features/absolute/index.ts", 'export { Button } from "./components/Button";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/absolute.ts"),
            code: `
                import { Button } from "${path.join(fixturesDir, "src/features/absolute/index.ts")}";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "../../features/absolute/components/Button";
          console.log(Button);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should ignore absolute paths outside cwd", async () => {
        await file("src/features/absolute/index.ts", 'export { Button } from "./components/Button";');

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/absolute.ts"),
            code: `
                import { Button } from "/root/file.ts";
                console.log(Button);
            `,
            config: defaultConfig,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "/root/file.ts";
          console.log(Button);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
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

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "lib-a/src/pages/test/test1.ts"),
            code: `
                import { Button } from "#features/some";
                console.log(Button);
            `,
            config,
        });

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "../../features/some/components/Button";
          console.log(Button);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);

        const result2 = await transpileWithSwc({
            filename: path.join(fixturesDir, "lib-b/src/pages/test/test1.ts"),
            code: `
                import { select } from "#features/some";
                console.log(select);
            `,
            config,
        });

        expect(result2.code).toMatchInlineSnapshot(`
          "import { select } from "../../features/some/model/selectors";
          console.log(select);
          "
        `);
        expect(result2.stdout).toMatchInlineSnapshot(`""`);
        expect(result2.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should match barrel files without aliases", async () => {
        await file("src/features/some/index.ts", 'export { Button } from "./components/Button";');

        const result = await transpileWithSwc({
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

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "../../features/some/index.ts";
          console.log(Button);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should skip files outside cwd", async () => {
        const result = await transpileWithSwc({
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

        expect(result.code).toMatchInlineSnapshot(`
          "import { Button } from "/dev/null";
          console.log(Button);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    it("should works with commonjs compilation target", async () => {
        await file(
            "src/features/some/index.ts",
            `
                export { Button } from "./components/Button";
                export { select } from "./model/selectors";
            `,
        );

        const result = await transpileWithSwc({
            filename: path.join(fixturesDir, "src/pages/test/test1.ts"),
            code: `
                import { Button, select, type SomeType } from "#features/some";
                import type { ButtonProps } from "#features/some";

                console.log(Button, select);
            `,
            config: defaultConfig,
            cjs: true,
        });

        expect(result.code).toMatchInlineSnapshot(`
          ""use strict";
          Object.defineProperty(exports, "__esModule", {
              value: true
          });
          const _Button = require("../../features/some/components/Button");
          const _selectors = require("../../features/some/model/selectors");
          console.log(_Button.Button, _selectors.select);
          "
        `);
        expect(result.stdout).toMatchInlineSnapshot(`""`);
        expect(result.stderr).toMatchInlineSnapshot(`""`);
    });

    describe("unsupported_import_mode configuration", () => {
        it("should error on namespace imports by default", async () => {
            await file("src/features/f1/index.ts", 'export { Button } from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/namespace.ts"),
                code: `
                    import * as f1 from "#features/f1";
                    console.log(f1);
                `,
                config: defaultConfig,
            });

            expect(result.code).toMatchInlineSnapshot(`""`);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`
              "x Error processing barrel import: E_NO_NAMESPACE_IMPORTS: Namespace imports are not supported for barrel file optimization
                 ,-[/cwd/tests/fixtures/src/pages/test/namespace.ts:2:1]
               1 | 
               2 |                     import * as f1 from "#features/f1";
                 :                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
               3 |                     console.log(f1);
               4 |                 
                 \`----"
            `);
        });

        it("should error on namespace imports when mode is 'error'", async () => {
            await file("src/features/f1/index.ts", 'export { Button } from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/namespace.ts"),
                code: `
                    import * as f1 from "#features/f1";
                    console.log(f1);
                `,
                config: {
                    ...defaultConfig,
                    unsupported_import_mode: "error",
                },
            });

            expect(result.code).toMatchInlineSnapshot(`""`);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`
              "x Error processing barrel import: E_NO_NAMESPACE_IMPORTS: Namespace imports are not supported for barrel file optimization
                 ,-[/cwd/tests/fixtures/src/pages/test/namespace.ts:2:1]
               1 | 
               2 |                     import * as f1 from "#features/f1";
                 :                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
               3 |                     console.log(f1);
               4 |                 
                 \`----"
            `);
        });

        it("should warn on namespace imports when mode is 'warn'", async () => {
            await file("src/features/f1/index.ts", 'export { Button } from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/namespace.ts"),
                code: `
                        import * as f1 from "#features/f1";
                        console.log(f1);
                    `,
                config: {
                    ...defaultConfig,
                    unsupported_import_mode: "warn",
                },
            });

            expect(result.code).toMatchInlineSnapshot(`
                  "import * as f1 from "#features/f1";
                  console.log(f1);
                  "
                `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(
                `"Warning: Namespace imports are not supported for barrel file optimization. Import from #features/f1 will be skipped."`,
            );
        });

        it("should ignore namespace imports when mode is 'off'", async () => {
            await file("src/features/f1/index.ts", 'export { Button } from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/namespace.ts"),
                code: `
                    import * as f1 from "#features/f1";
                    console.log(f1);
                `,
                config: {
                    ...defaultConfig,
                    unsupported_import_mode: "off",
                },
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import * as f1 from "#features/f1";
              console.log(f1);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should handle mixed imports with namespace import skipped", async () => {
            await file("src/features/f1/index.ts", 'export { Button } from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/mixed.ts"),
                code: `
                    import * as f1 from "#features/f1";
                    import { Button } from "#features/f1";
                    console.log(Button, f1);
                `,
                config: {
                    ...defaultConfig,
                    unsupported_import_mode: "off",
                },
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import * as f1 from "#features/f1";
              import { Button } from "../../features/f1/components/Button";
              console.log(Button, f1);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should reject invalid unsupported_import_mode values", async () => {
            await file("src/features/f1/index.ts", 'export { Button } from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/invalid-config.ts"),
                code: `
                    import { Button } from "#features/f1";
                    console.log(Button);
                `,
                config: {
                    ...defaultConfig,
                    unsupported_import_mode: "invalid" as any,
                },
            });

            expect(result.code).toMatchInlineSnapshot(`""`);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`
              "thread '<unnamed>' panicked at src/lib.rs:41:6:
              E_INVALID_CONFIG: Error parsing barrel plugin configuration: Error("Invalid unsupported_import_mode 'invalid'. Valid options are: error, warn, off", line: 1, column: ?)
              note: run with \`RUST_BACKTRACE=1\` environment variable to display a backtrace
              plugin

                x failed to invoke plugin on 'Some("/cwd/tests/fixtures/src/pages/test/invalid-config.ts")'"
            `);
        });
    });

    describe("invalid_barrel_mode configuration", () => {
        it("should error on invalid barrel files by default", async () => {
            await file("src/features/invalid/index.ts", 'export * from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/invalid.ts"),
                code: `
                    import { Button } from "#features/invalid";
                    console.log(Button);
                `,
                config: defaultConfig,
            });

            expect(result.code).toMatchInlineSnapshot(`""`);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`
              "x Error processing barrel import: E_INVALID_BARREL_FILE: Invalid barrel file /cwd/tests/fixtures/src/features/invalid/index.ts: Wildcard exports are not supported in barrel files: Wildcard exports are not allowed in barrel files
                 ,-[/cwd/tests/fixtures/src/pages/test/invalid.ts:2:1]
               1 | 
               2 |                     import { Button } from "#features/invalid";
                 :                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
               3 |                     console.log(Button);
               4 |                 
                 \`----"
            `);
        });

        it("should error on invalid barrel files when mode is 'error'", async () => {
            await file("src/features/invalid/index.ts", 'export * from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/invalid.ts"),
                code: `
                    import { Button } from "#features/invalid";
                    console.log(Button);
                `,
                config: {
                    ...defaultConfig,
                    invalid_barrel_mode: "error",
                },
            });

            expect(result.code).toMatchInlineSnapshot(`""`);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`
              "x Error processing barrel import: E_INVALID_BARREL_FILE: Invalid barrel file /cwd/tests/fixtures/src/features/invalid/index.ts: Wildcard exports are not supported in barrel files: Wildcard exports are not allowed in barrel files
                 ,-[/cwd/tests/fixtures/src/pages/test/invalid.ts:2:1]
               1 | 
               2 |                     import { Button } from "#features/invalid";
                 :                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
               3 |                     console.log(Button);
               4 |                 
                 \`----"
            `);
        });

        it("should warn on invalid barrel files when mode is 'warn'", async () => {
            await file("src/features/invalid/index.ts", 'export * from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/invalid.ts"),
                code: `
                    import { Button } from "#features/invalid";
                    console.log(Button);
                `,
                config: {
                    ...defaultConfig,
                    invalid_barrel_mode: "warn",
                },
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { Button } from "#features/invalid";
              console.log(Button);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(
                `"Warning: E_INVALID_BARREL_FILE: Invalid barrel file /cwd/tests/fixtures/src/features/invalid/index.ts: Wildcard exports are not supported in barrel files: Wildcard exports are not allowed in barrel files"`,
            );
        });

        it("should ignore invalid barrel files when mode is 'off'", async () => {
            await file("src/features/invalid/index.ts", 'export * from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/invalid.ts"),
                code: `
                    import { Button } from "#features/invalid";
                    console.log(Button);
                `,
                config: {
                    ...defaultConfig,
                    invalid_barrel_mode: "off",
                },
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { Button } from "#features/invalid";
              console.log(Button);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should handle barrel files with variable declarations when mode is 'warn'", async () => {
            await file("src/features/with-vars/index.ts", 'export const VERSION = "1.0.0";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/with-vars.ts"),
                code: `
                    import { VERSION } from "#features/with-vars";
                    console.log(VERSION);
                `,
                config: {
                    ...defaultConfig,
                    invalid_barrel_mode: "warn",
                },
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { VERSION } from "#features/with-vars";
              console.log(VERSION);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(
                `"Warning: E_INVALID_BARREL_FILE: Invalid barrel file /cwd/tests/fixtures/src/features/with-vars/index.ts: Barrel file contains non-export code: Variable declarations are not allowed in barrel files"`,
            );
        });

        it("should handle barrel files with namespace exports when mode is 'off'", async () => {
            await file("src/features/namespace/index.ts", 'export * as components from "./components";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/namespace.ts"),
                code: `
                    import { components } from "#features/namespace";
                    console.log(components);
                `,
                config: {
                    ...defaultConfig,
                    invalid_barrel_mode: "off",
                },
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { components } from "#features/namespace";
              console.log(components);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should reject invalid invalid_barrel_mode values", async () => {
            await file("src/features/f1/index.ts", 'export { Button } from "./components/Button";');

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test/invalid-config.ts"),
                code: `
                    import { Button } from "#features/f1";
                    console.log(Button);
                `,
                config: {
                    ...defaultConfig,
                    invalid_barrel_mode: "invalid" as any,
                },
            });

            expect(result.code).toMatchInlineSnapshot(`""`);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`
              "thread '<unnamed>' panicked at src/lib.rs:41:6:
              E_INVALID_CONFIG: Error parsing barrel plugin configuration: Error("Invalid invalid_barrel_mode 'invalid'. Valid options are: error, warn, off", line: 1, column: ?)
              note: run with \`RUST_BACKTRACE=1\` environment variable to display a backtrace
              plugin

                x failed to invoke plugin on 'Some("/cwd/tests/fixtures/src/pages/test/invalid-config.ts")'"
            `);
        });
    });

    describe("symlinks configuration", () => {
        it("should transform imports from external paths using symlinks", async () => {
            await file("src/ui/index.ts", 'export { Button } from "./Button";\nexport { Input } from "./Input";');

            const config: PluginConfig = {
                patterns: [path.join(fixturesDir, "src/ui/index.ts")],
                symlinks: {
                    "/var/external-lib/src/components": path.join(fixturesDir, "src/ui/index.ts"),
                },
            };

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test.ts"),
                code: `
                    import { Button, Input } from "/var/external-lib/src/components";
                    console.log(Button, Input);
                `,
                config,
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { Button } from "../ui/Button";
              import { Input } from "../ui/Input";
              console.log(Button, Input);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should handle symlinks with aliases for external imports", async () => {
            await file("src/ui/index.ts", 'export { Button } from "./Button";\nexport { Input } from "./Input";');

            const config: PluginConfig = {
                patterns: [path.join(fixturesDir, "src/ui/index.ts")],
                aliases: [
                    {
                        pattern: "#external/*",
                        paths: ["/var/external-lib/src/*/index.ts"],
                    },
                ],
                symlinks: {
                    "/var/external-lib": fixturesDir,
                },
            };

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/login.ts"),
                code: `
                    import { Button, Input } from "#external/ui";
                    console.log(Button, Input);
                `,
                config,
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { Button } from "../ui/Button";
              import { Input } from "../ui/Input";
              console.log(Button, Input);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should resolve relative paths correctly in symlinked external imports", async () => {
            await file(
                "src/features/user/index.ts",
                'export { UserProfile } from "./components/UserProfile";\nexport { fetchUser } from "./api/user";',
            );

            const config: PluginConfig = {
                patterns: [path.join(fixturesDir, "src/features/*/index.ts")],
                symlinks: {
                    "../external-nested/src/features/user": path.join(fixturesDir, "src/features/user/index.ts"),
                },
            };

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/user.ts"),
                code: `
                    import { UserProfile, fetchUser } from "../../../../../../external-nested/src/features/user";
                    console.log(UserProfile, fetchUser);
                `,
                config,
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { fetchUser } from "../features/user/api/user";
              import { UserProfile } from "../features/user/components/UserProfile";
              console.log(UserProfile, fetchUser);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should handle absolute external paths with symlinks", async () => {
            await file("src/components/index.ts", 'export { GlobalComponent } from "/absolute/path/to/component";');

            const config: PluginConfig = {
                patterns: [path.join(fixturesDir, "src/components/index.ts")],
                symlinks: {
                    "/external/absolute/components": path.join(fixturesDir, "src/components/index.ts"),
                },
            };

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/global.ts"),
                code: `
                    import { GlobalComponent } from "/external/absolute/components";
                    console.log(GlobalComponent);
                `,
                config,
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { GlobalComponent } from "/absolute/path/to/component";
              console.log(GlobalComponent);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should handle external imports that don't match symlinks", async () => {
            await file("src/components/index.ts", 'export { Button } from "./Button";');

            const config: PluginConfig = {
                patterns: [path.join(fixturesDir, "src/components/index.ts")],
                symlinks: {
                    "../some-other-lib/index.ts": path.join(fixturesDir, "src/components/index.ts"),
                },
            };

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test.ts"),
                code: `
                    import { Button } from "../non-existent-lib/components";
                    console.log(Button);
                `,
                config,
            });

            // Should leave the import unchanged since no symlink matches
            expect(result.code).toMatchInlineSnapshot(`
              "import { Button } from "../non-existent-lib/components";
              console.log(Button);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should handle context-specific aliases with external symlinks", async () => {
            await file("app-a/src/features/shared/index.ts", 'export { ComponentA } from "./ComponentA";');
            await file("app-b/src/features/shared/index.ts", 'export { ComponentB } from "./ComponentB";');

            const config: PluginConfig = {
                patterns: [
                    path.join(fixturesDir, "app-a/src/features/*/index.ts"),
                    path.join(fixturesDir, "app-b/src/features/*/index.ts"),
                ],
                aliases: [
                    {
                        pattern: "#external/*",
                        paths: ["../external-app-a/features/*/index.ts"],
                        context: [path.join(fixturesDir, "app-a")],
                    },
                    {
                        pattern: "#external/*",
                        paths: ["../external-app-b/features/*/index.ts"],
                        context: [path.join(fixturesDir, "app-b")],
                    },
                ],
                symlinks: {
                    "../external-app-a/features/shared/index.ts": path.join(
                        fixturesDir,
                        "app-a/src/features/shared/index.ts",
                    ),
                    "../external-app-b/features/shared/index.ts": path.join(
                        fixturesDir,
                        "app-b/src/features/shared/index.ts",
                    ),
                },
            };

            // Test app-a context
            const resultA = await transpileWithSwc({
                filename: path.join(fixturesDir, "app-a/src/pages/test.ts"),
                code: `
                    import { ComponentA } from "#external/shared";
                    console.log(ComponentA);
                `,
                config,
            });

            expect(resultA.code).toMatchInlineSnapshot(`
              "import { ComponentA } from "../features/shared/ComponentA";
              console.log(ComponentA);
              "
            `);

            // Test app-b context
            const resultB = await transpileWithSwc({
                filename: path.join(fixturesDir, "app-b/src/pages/test.ts"),
                code: `
                    import { ComponentB } from "#external/shared";
                    console.log(ComponentB);
                `,
                config,
            });

            expect(resultB.code).toMatchInlineSnapshot(`
              "import { ComponentB } from "../features/shared/ComponentB";
              console.log(ComponentB);
              "
            `);
        });

        it("should handle directory-level symlinks for single files", async () => {
            await file("src/features/auth/index.ts", 'export { login } from "./api/login";');

            const config: PluginConfig = {
                patterns: [path.join(fixturesDir, "src/features/*/index.ts")],
                symlinks: {
                    "../external-lib/features": path.join(fixturesDir, "src/features"),
                },
            };

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/test.ts"),
                code: `
                    import { login } from "../../../../../../external-lib/features/auth/index.ts";
                    console.log(login);
                `,
                config,
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { login } from "../features/auth/api/login";
              console.log(login);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should handle directory-level symlinks with aliases", async () => {
            await file("src/libs/ui/index.ts", 'export { Button } from "./Button";');
            await file("src/libs/utils/index.ts", 'export { helper } from "./helper";');

            const config: PluginConfig = {
                patterns: [path.join(fixturesDir, "src/libs/*/index.ts")],
                aliases: [
                    {
                        pattern: "#external/*",
                        paths: ["../external-workspace/libs/*/index.ts"],
                    },
                ],
                symlinks: {
                    "../external-workspace/libs": path.join(fixturesDir, "src/libs"),
                },
            };

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/libs.ts"),
                code: `
                    import { Button } from "#external/ui";
                    import { helper } from "#external/utils";
                    console.log(Button, helper);
                `,
                config,
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { Button } from "../libs/ui/Button";
              import { helper } from "../libs/utils/helper";
              console.log(Button, helper);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(
                `""`,
            );
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should prioritize specific file symlinks over directory symlinks", async () => {
            await file("src/components/Button/index.ts", 'export { Button } from "./Button";');
            await file("src/components/special/index.ts", 'export { SpecialButton } from "./SpecialButton";');

            const config: PluginConfig = {
                patterns: [path.join(fixturesDir, "src/components/*/index.ts")],
                symlinks: {
                    "../external-lib/components": path.join(fixturesDir, "src/components"),
                    "../external-lib/components/Button/index.ts": path.join(
                        fixturesDir,
                        "src/components/special/index.ts",
                    ),
                },
            };

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/priority.ts"),
                code: `
                    import { SpecialButton } from "../../../../../../external-lib/components/Button/index.ts";
                    console.log(SpecialButton);
                `,
                config,
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { SpecialButton } from "../components/special/SpecialButton";
              console.log(SpecialButton);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });

        it("should handle directory-level symlinks with trailing slashes", async () => {
            await file("src/features/auth/index.ts", 'export { login } from "./api/login";');

            const config: PluginConfig = {
                patterns: [path.join(fixturesDir, "src/features/*/index.ts")],
                symlinks: {
                    "../external-lib/features/": path.join(fixturesDir, "src/features"),
                },
            };

            const result = await transpileWithSwc({
                filename: path.join(fixturesDir, "src/pages/trailing.ts"),
                code: `
                    import { login } from "../../../../../../external-lib/features/auth/index.ts";
                    console.log(login);
                `,
                config,
            });

            expect(result.code).toMatchInlineSnapshot(`
              "import { login } from "../features/auth/api/login";
              console.log(login);
              "
            `);
            expect(result.stdout).toMatchInlineSnapshot(`""`);
            expect(result.stderr).toMatchInlineSnapshot(`""`);
        });
    });
});
