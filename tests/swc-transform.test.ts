import process from "node:process";
import path from "node:path";
import fs from "node:fs/promises";
import { describe, it, expect, beforeEach } from "vitest";
import { transform } from "@swc/core";

const fixturesDir = path.resolve(__dirname, "fixtures");

interface PluginConfig {
    rules: Array<{
        pattern: string;
        paths: string[];
    }>;
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
                plugins: [
                    [require.resolve("../swc-plugin/target/wasm32-wasi/release/swc_plugin_barrel_files.wasm"), config],
                ],
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
        rules: [
            {
                pattern: "#features/*",
                paths: [path.join(fixturesDir, "src/features/*/index.ts")],
            },
            {
                pattern: "#features/*/testing",
                paths: [path.join(fixturesDir, "src/features/*/testing.ts")],
            },
        ],
    };

    beforeEach(async () => {
        await fs.rm(path.join(fixturesDir, "src/features"), { recursive: true });
    });

    it("it should transform index file imports", async () => {
        await file(
            "src/features/some/index.ts",
            `
                export { Button } from "./components/Button";
                export { select } from "./model/selectors";
            `,
        );
        // await file("src/features/some/components/Button.ts", 'export const Button = "Button";');
        // await file("src/features/some/model/selectors.ts", 'export const select = "select";');

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

    it("it should transform testing file imports", async () => {
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

    // ### Happy path
    // импорт из файла, в котором есть комментарии
    // поиск jsx энтрипоинта по массиву путей
    // импорт с переименованием
    // импорт из файла, в котором экспорт с переименованием
    // экспорт default
    // относительный путь в конфиге паттернов
    // абсолютный путь в конфиге паттернов, который совпадает с cwd

    // ### ошибки
    // импорт из несуществующего файла
    // импорт из файла, в котором нет нужного экспорта
    // импорт из файла, в котором есть код
    // импорт из файла, в котором есть export * {}
    // импорт из файла, в котором есть export * as asd {}
    // импорт из файла, который не удалось распарсить
    // импорт по абслютному пути
    // ре-экспорт по абсолютному пути
    // абсолютный путь в конфиге паттернов, который не совпадает с cwd

    // ### прочее
    // приоритеты паттернов по кол-ву *
});
