import { execSync } from "child_process";
import fs from "fs";
import path from "path";

const FIXTURES_DIR = path.resolve(__dirname, "./fixtures");
const OUTPUT_DIR = path.resolve(__dirname, "./output");
const SWC_CONFIG_PATH = path.resolve(__dirname, ".swcrc");
const ITERATIONS = 5;

function generateSwcConfig(plugin: unknown): void {
    const config = {
        jsc: {
            parser: {
                syntax: "typescript",
                tsx: true,
                decorators: false,
                dynamicImport: false,
            },
            transform: {
                react: {
                    pragma: "React.createElement",
                    pragmaFrag: "React.Fragment",
                    throwIfNamespace: true,
                    development: false,
                    useBuiltins: false,
                },
            },
            target: "es2022",
            loose: false,
            externalHelpers: false,
            keepClassNames: false,
            experimental: {
                plugins: [plugin],
            },
        },
        module: {
            type: "es6",
        },
        minify: false,
        isModule: true,
    };

    fs.writeFileSync(SWC_CONFIG_PATH, JSON.stringify(config, null, 4), "utf8");
}

function getBarrelFilesConfig(wasmPath: string) {
    return [
        wasmPath,
        {
            patterns: [
                path.resolve(FIXTURES_DIR, "components/*/index.ts").replace(/\\/g, "/"),
                path.resolve(FIXTURES_DIR, "utils/*/index.ts").replace(/\\/g, "/"),
                path.resolve(FIXTURES_DIR, "hooks/*/index.ts").replace(/\\/g, "/"),
                path.resolve(FIXTURES_DIR, "services/*/index.ts").replace(/\\/g, "/"),
                path.resolve(FIXTURES_DIR, "*/index.ts").replace(/\\/g, "/"),
            ],
            aliases: [
                {
                    pattern: "#components/*",
                    paths: [path.resolve(FIXTURES_DIR, "components/*/index.ts").replace(/\\/g, "/")],
                },
                {
                    pattern: "#utils/*",
                    paths: [path.resolve(FIXTURES_DIR, "utils/*/index.ts").replace(/\\/g, "/")],
                },
                {
                    pattern: "#src/*",
                    paths: [path.resolve(FIXTURES_DIR, "*/index.ts").replace(/\\/g, "/")],
                },
            ],
            unsupported_import_mode: "error",
            invalid_barrel_mode: "error",
        },
    ];
}

function runCompilation(plugin: unknown): { duration: number; success: boolean } {
    generateSwcConfig(plugin);

    fs.rmSync(OUTPUT_DIR, { recursive: true, force: true });
    fs.mkdirSync(OUTPUT_DIR, { recursive: true });

    const startTime = process.hrtime.bigint();

    const command = `pnpm exec swc ${FIXTURES_DIR} -d ${OUTPUT_DIR} --config-file ${path.join(__dirname, ".swcrc")}`;

    execSync(command, {
        encoding: "utf8",
        stdio: "pipe",
        cwd: process.cwd(),
    });

    const endTime = process.hrtime.bigint();
    const duration = Number(endTime - startTime) / 1_000_000;

    return { duration, success: true };
}

type Config = { name: "BASE" | "CURRENT"; plugin: unknown };

async function runBenchmark(configs: Config[]) {
    const results: Record<Config["name"], number[]> = {
        BASE: [],
        CURRENT: [],
    };

    for (let i = 0; i < ITERATIONS; i++) {
        for (const config of configs) {
            console.log(`Running ${config.name} ${i + 1}...`);
            const result = runCompilation(config.plugin);
            results[config.name].push(result.duration);
            await new Promise((resolve) => setTimeout(resolve, 1000));
        }
    }

    const averages: Record<string, number> = {};
    for (const [name, durations] of Object.entries(results)) {
        const average = durations.reduce((sum, duration) => sum + duration, 0) / durations.length;
        averages[name] = average;
        console.log(`${name} runs: ${durations.map((d) => (d / 1000).toFixed(2)).join("s, ")}s`);
        console.log(`${name} average: ${(average / 1000).toFixed(2)}s`);
    }

    const baseAverage = averages.BASE || 0;
    const currentAverage = averages.CURRENT || 0;

    const baseTime = (baseAverage / 1000).toFixed(2);
    const currentTime = (currentAverage / 1000).toFixed(2);

    console.log(`\nSUMMARY:`);
    console.log(`BASE:        ${baseTime}s`);
    console.log(`CURRENT:     ${currentTime}s`);

    const difference = currentAverage - baseAverage;
    const percentChange = (difference / baseAverage) * 100;

    const sign = difference > 0 ? "+" : "";
    console.log(`Difference:  ${sign}${(difference / 1000).toFixed(2)}s (${sign}${percentChange.toFixed(1)}%)`);
}

const args = process.argv.slice(2);

if (args.includes("--help") || args.includes("-h")) {
    console.log("Usage: tsx perf/run.ts [options]");
    console.log("Options:");
    console.log("  --base <path>            Run benchmark against custom base build (provide WASM path)");
    console.log("  --help, -h               Show this help message");
    process.exit(0);
}

function getArgValue(argName: string): string | null {
    const index = args.indexOf(argName);
    if (index !== -1 && index + 1 < args.length) {
        return args[index + 1];
    }
    return null;
}

const configs: Config[] = [];

const baseIndex = args.indexOf("--base");
if (baseIndex !== -1) {
    let basePath = getArgValue("--base");
    if (!basePath) {
        console.error("Error: --base option requires a path to the WASM file");
        process.exit(1);
    }
    basePath = path.resolve(process.cwd(), basePath);
    if (!fs.existsSync(basePath)) {
        console.error(`Error: Base WASM file not found at: ${basePath}`);
        process.exit(1);
    }
    configs.push({ name: "BASE", plugin: getBarrelFilesConfig(basePath) });
} else {
    configs.push({ name: "BASE", plugin: ["@swc/plugin-noop", {}] });
}

configs.push({
    name: "CURRENT",
    plugin: getBarrelFilesConfig(path.join(__dirname, "../swc_plugin_barrel_files.wasm")),
});

runBenchmark(configs).catch(console.error);
