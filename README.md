# SWC Barrel Files Plugin

A barrel file is a module that only re-exports other modules, usually an `index.ts`:

```typescript
// src/modules/user/index.ts
export { User } from "./models/User";
export { createUser } from "./api/createUser";
export { userReducer } from "./store/reducer";
```

An import from a barrel loads every module it re-exports, even when the code needs one name. This plugin for [SWC](https://swc.rs) rewrites such imports at compile time into direct imports from the source files:

```typescript
// src/pages/profile.ts, before
import { User, createUser } from "@modules/user";

// after
import { User } from "../modules/user/models/User";
import { createUser } from "../modules/user/api/createUser";
```

The source code keeps the short barrel imports, and the compiled output loads only the modules it uses. That gives the bundler less to parse and tree-shake, so builds get faster, which matters most in development, where tree-shaking is often off. Tests transpile and load fewer modules too. Circular dependencies that exist only because two modules meet in a barrel go away. Nothing is added at runtime.

The plugin does not resolve modules the way a bundler does. It reads the barrel file, matches paths against the patterns from its config, and leaves every import it does not recognize unchanged.

## Installation

npm

```bash
npm install --save-dev swc-plugin-barrel-files
```

Yarn

```bash
yarn add --dev swc-plugin-barrel-files
```

pnpm

```bash
pnpm add --save-dev swc-plugin-barrel-files
```

## Compatibility

An SWC plugin is a Wasm module built against one version of `swc_core`, and it runs only in hosts whose plugin runner speaks the same format. A plugin built for older hosts fails on newer ones with a deserialization error, so pick the plugin version by your SWC host:

| Plugin version | Built with      | Hosts                                                            |
| -------------- | --------------- | ---------------------------------------------------------------- |
| >=0.1.0 <0.3.0 | swc_core 0.87   | [Compatibility table](https://plugins.swc.rs/versions/range/10)  |
| >=0.3.0 <0.6.0 | swc_core 31     | [Compatibility table](https://plugins.swc.rs/versions/range/364) |
| >=0.6.0        | swc_core 82.2.0 | `swc_plugin_runner` 34, see below                                |

For 0.6.0, plugins.swc.rs has no compatibility range yet. The newest range it lists is for `swc_core` 77 (checked on 2026-10-25). On the same date the plugin was run on `@swc/core` 1.16.20, Next.js 16.4.3 (Turbopack and webpack) and Rspack 2.2.9. For another host, check that it ships `swc_plugin_runner` 34. The host prints its runner version in the error message when a plugin is incompatible.

## Configuration

The minimal config lists the barrel files in `patterns`:

```json
{
    "jsc": {
        "experimental": {
            "plugins": [
                [
                    "swc-plugin-barrel-files",
                    {
                        "patterns": ["src/modules/*/index.ts"]
                    }
                ]
            ]
        }
    }
}
```

A `*` matches one path segment, so this pattern covers `src/modules/user/index.ts` but not `src/modules/user/admin/index.ts`. Patterns and paths are relative to the current working directory. An absolute path works too, as long as it starts with the current working directory.

A pattern names the barrel file itself, file name and extension included. `src/modules/*/` matches nothing. The plugin does not try extensions or `index` files the way Node.js does, so a relative import is rewritten only when it spells out the same file:

```typescript
import { User } from "./modules/user/index.ts"; // rewritten
import { User } from "./modules/user"; // left unchanged
import { User } from "./modules/user/"; // left unchanged
```

Imports through aliases do not have this restriction, because the alias config names the file.

### Aliases

Most projects import barrels through path aliases, the same ones `tsconfig.json` declares in `compilerOptions.paths`:

```json
{
    "compilerOptions": {
        "paths": {
            "@modules/*": ["src/modules/*"]
        }
    }
}
```

The plugin does not read `tsconfig.json`, so the aliases go into its own config, with the barrel file at the end of each path:

```json
{
    "patterns": ["src/modules/*/index.ts"],
    "aliases": [
        {
            "pattern": "@modules/*",
            "paths": ["src/modules/*/index.ts"]
        }
    ]
}
```

With this config, `import { User } from "@modules/user"` resolves to `src/modules/user/index.ts`. The plugin tries the paths in order and takes the first file that exists. When the alias matches but none of its files exist, compilation fails with `E_BARREL_FILE_NOT_FOUND`.

### Context-specific aliases

The `context` option limits an alias to files inside the listed directories. This is how one alias resolves to different barrels in different apps:

```json
{
    "patterns": ["apps/*/src/modules/*/index.ts"],
    "aliases": [
        {
            "pattern": "@modules/*",
            "paths": ["apps/app-1/src/modules/*/index.ts"],
            "context": ["apps/app-1"]
        },
        {
            "pattern": "@modules/*",
            "paths": ["apps/app-2/src/modules/*/index.ts"],
            "context": ["apps/app-2"]
        }
    ]
}
```

A context matches whole directory names: `apps/app-1` does not cover `apps/app-10`. `"."` covers the whole working directory.

### Error handling

Two options decide what happens when the plugin meets something it cannot rewrite. Both accept `"error"` (the default, compilation fails), `"warn"` (the import is left unchanged and a warning goes to stderr) and `"off"` (the import is left unchanged silently):

```json
{
    "patterns": ["src/modules/*/index.ts"],
    "unsupported_import_mode": "warn",
    "invalid_barrel_mode": "error"
}
```

- `unsupported_import_mode` covers namespace imports such as `import * as user from "@modules/user"`. A namespace import needs the whole barrel, so the declaration that contains it stays as it is.
- `invalid_barrel_mode` covers barrel files that the plugin cannot analyze: wildcard exports, declarations and any other code next to the re-exports.

With `"warn"`, you can enable the plugin in a large codebase first and fix the barrel files one by one.

### Debug logging

`"debug": true` prints what the plugin does with each file to stdout: which alias an import resolved to, which pattern it did not match, what the import was replaced with. This is the first thing to turn on when an import is not rewritten.

## Monorepos

SWC runs plugins in a sandbox. The plugin sees only the current working directory of the process that runs SWC, and **it cannot read files through symlinks**, whether the link points inside that directory or outside it. pnpm, Yarn and npm workspaces link packages into `node_modules` with symlinks, so a barrel file reached through `node_modules/@scope/lib` cannot be read. The import then fails with `E_BARREL_FILE_NOT_FOUND`.

To make it work, run SWC from the monorepo root and point `patterns` and `aliases` at the real location of the package (`packages/lib/src/...`, not `node_modules/@scope/lib/src/...`):

```json
{
    "patterns": ["packages/lib/src/features/*/index.ts"],
    "aliases": [
        {
            "pattern": "@scope/lib/src/features/*",
            "paths": ["packages/lib/src/features/*/index.ts"]
        }
    ]
}
```

The working directory is the one of the process, `process.cwd()` in Node.js. The `cwd` option of `@swc/core` does not move the sandbox. Bundlers usually hand SWC the real path of a file rather than the path through the link (webpack does it by default, see `resolve.symlinks`), so files from workspace packages fall inside the patterns above.

A path that leads outside the working directory, such as `../shared/*/index.ts` or an absolute path elsewhere, fails with `E_INVALID_FILE_PATH`.

### Migrating from `symlinks`

Up to version 0.5, the plugin had a `symlinks` option that mapped external paths onto symlinked paths inside the working directory. Reading through such a path worked on `@swc/core` 1.3. On 1.13 and newer the host refuses to follow the symlink, so the option could not help there. Version 0.6.0 removed it, and a config that still sets it fails with `E_INVALID_CONFIG`.

To migrate, delete `symlinks`, run SWC from the root that contains both the app and the linked packages, and replace each symlinked path in `patterns` and `aliases` with the real path from [Monorepos](#monorepos). A hook that rewrote file names into `node_modules` paths for the plugin is no longer needed.

## Integrations

### Next.js

Next.js passes plugins through `experimental.swcPlugins`, both with Turbopack and with webpack:

```javascript
// next.config.mjs
export default {
    experimental: {
        swcPlugins: [
            [
                "swc-plugin-barrel-files",
                {
                    patterns: ["src/modules/*/index.ts"],
                    aliases: [{ pattern: "@modules/*", paths: ["src/modules/*/index.ts"] }],
                },
            ],
        ],
    },
};
```

### Rspack

`builtin:swc-loader` takes the same options as `.swcrc`:

```javascript
// rspack.config.mjs
export default {
    module: {
        rules: [
            {
                test: /\.tsx?$/,
                loader: "builtin:swc-loader",
                options: {
                    jsc: {
                        parser: { syntax: "typescript" },
                        experimental: {
                            plugins: [["swc-plugin-barrel-files", { patterns: ["src/modules/*/index.ts"] }]],
                        },
                    },
                },
            },
        ],
    },
};
```

### Jest

Jest uses SWC through `@swc/jest`:

```javascript
// jest.config.js
module.exports = {
    transform: {
        "^.+\\.(t|j)sx?$": [
            "@swc/jest",
            {
                jsc: {
                    experimental: {
                        plugins: [
                            [
                                "swc-plugin-barrel-files",
                                {
                                    patterns: ["src/modules/*/index.ts"],
                                    aliases: [{ pattern: "@modules/*", paths: ["src/modules/*/index.ts"] }],
                                },
                            ],
                        ],
                    },
                },
            },
        ],
    },
};
```

The plugin rewrites imports, but not the module names in `jest.mock()`. A mock of the barrel no longer applies, because the code under test imports the source file directly:

```typescript
import { User } from "@modules/user";
// compiled to: import { User } from "../modules/user/models/User";

jest.mock("@modules/user", () => ({ User: jest.fn() })); // does not affect User
```

Mock the source file instead:

```typescript
jest.mock("../modules/user/models/User", () => ({ User: jest.fn() }));
```

## Limitations

The plugin rewrites static ESM imports with named and default specifiers. It leaves alone:

- dynamic imports (`import()`) and CommonJS `require()`,
- namespace imports (`import * as x from "./ui"`), see `unsupported_import_mode`,
- side-effect imports (`import "./ui/index.ts"`), which have no names to rewrite.

A barrel file contains only re-exports with a source, in these forms:

```typescript
export { ComponentA, ComponentB } from "./components";
export { ComponentA as Component } from "./components";
export { default as Button } from "./Button";
export { Button as default } from "./Button";
```

Anything else makes the file invalid for the plugin, see `invalid_barrel_mode`: wildcard exports (`export * from`), namespace exports (`export * as x from`), local declarations, imports and other statements.

### Side effects

After the rewrite, only the modules that provide the imported names are loaded. The other modules the barrel re-exports are not loaded anymore, and their side effects (registering a custom element, importing global CSS, patching a global) no longer run:

```typescript
// src/ui/index.ts
export { Button } from "./Button";
export { Modal } from "./Modal"; // Modal.ts registers a custom element

// src/app.ts
import { Button } from "./ui/index.ts";
// compiled to: import { Button } from "./ui/Button";
// Modal.ts no longer runs
```

The `sideEffects` field in `package.json` does not change this. That field tells the bundler which unused modules it may drop, while after the rewrite the module is not imported at all. When code depends on such a side effect, import that module explicitly where the effect is needed, and run the tests after you enable the plugin.

## Error codes

| Code                      | When                                                                                                          | What to do                                                                                                |
| ------------------------- | ------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `E_INVALID_ENV`           | The host did not pass the current working directory or the file name to the plugin.                           | Check how your build tool runs SWC.                                                                       |
| `E_INVALID_CONFIG`        | The config is not valid JSON, a required field is missing, a mode has an unknown value, or `symlinks` is set. | Fix the config. For `symlinks`, see [Migrating from `symlinks`](#migrating-from-symlinks).                |
| `E_NO_NAMESPACE_IMPORTS`  | A namespace import from a barrel file.                                                                        | Use named imports, or set `unsupported_import_mode` to `"warn"` or `"off"`.                               |
| `E_UNRESOLVED_EXPORTS`    | The barrel file does not export an imported name, or has no re-exports at all.                                | Check the import against the barrel file. The message lists the missing names.                            |
| `E_FILE_READ`             | The plugin could not read the barrel file.                                                                    | Check that the file exists inside the working directory and is not reached through a symlink.             |
| `E_FILE_PARSE`            | The barrel file has a syntax error. `.tsx` files are parsed with JSX, all others as TypeScript.               | Fix the syntax error in the barrel file.                                                                  |
| `E_INVALID_BARREL_FILE`   | The barrel file contains something other than re-exports.                                                     | Move the code out of the barrel file, or set `invalid_barrel_mode` to `"warn"` or `"off"`.                |
| `E_INVALID_FILE_PATH`     | A pattern or an alias path leads outside the working directory.                                               | Use paths inside the working directory. In a monorepo, see [Monorepos](#monorepos).                       |
| `E_BARREL_FILE_NOT_FOUND` | An alias matched the import, but none of its paths points to an existing file.                                | Check the alias paths against the file structure, and that they don't go through `node_modules` symlinks. |

When an import is left unchanged without any error, the import and the config disagree: the alias pattern does not match the import, a `context` excludes the file, or a relative import omits the file name. `"debug": true` shows which of these it is.

## Contributing

The plugin is written in Rust and compiled to `wasm32-wasip1`. The integration tests run the built Wasm file through `@swc/core`:

```bash
pnpm install
pnpm build   # builds the plugin and copies swc_plugin_barrel_files.wasm to the root
pnpm test    # integration tests, Vitest
cargo test   # unit tests
```

`rust-toolchain.toml` pins stable Rust with the `wasm32-wasip1` target, so rustup installs the target on the first build.

The project uses [changesets](https://github.com/changesets/changesets) for versions and the changelog. A pull request that touches Rust or TypeScript sources or `package.json` needs a changeset, and CI checks that it is there:

```bash
pnpm changeset
```

The command asks for the type of change (patch, minor or major) and a summary, which goes into the changelog. Commit the generated file together with the change.

Releases go through GitHub Actions. After a merge to `main`, an action opens a pull request that bumps the version and updates the changelog. Merging that pull request publishes the new version to npm.

## License

MIT
