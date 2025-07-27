# SWC Barrel Files Plugin

A SWC plugin that transforms imports from barrel files (index.ts) into direct imports from source files.

## Benefits

Using this plugin provides several advantages:

- **Improved Tree-Shaking**: By transforming barrel imports into direct imports, bundlers can better eliminate unused code.
- **Faster Build Times**: Direct imports reduce the work needed by the bundler, resulting in faster builds. This is especially important for development environments where tree-shaking is often turned off.
- **Faster Test Execution**: Tests run faster because fewer modules need to be transpiled and loaded.
- **Prevention of Circular Dependencies**: Eliminates common circular dependency issues caused by barrel files.
- **Clean Code Maintenance**: You can still use clean barrel imports in your source code while getting optimized output.
- **No Runtime Overhead**: All transformations happen at compile time.

## What is a Barrel File?

A barrel file is a file that re-exports from other files, typically named `index.ts`. For example:

```typescript
// src/modules/user/index.ts (barrel file)
export { User } from "./models/User";
export { createUser } from "./api/createUser";
export { userReducer } from "./store/reducer";
```

Barrel files make imports cleaner in your application:

```typescript
// Without barrel files
import { User } from "./modules/user/models/User";
import { createUser } from "./modules/user/api/createUser";
import { userReducer } from "./modules/user/store/reducer";

// With barrel files
import { User, createUser, userReducer } from "./modules/user";
```

However, barrel files can cause issues with tree-shaking, build performance and circular dependencies. This plugin transforms imports from barrel files into direct imports from source files at compile time, giving you the best of both worlds: clean imports in your code and optimized imports in the compiled output.

## Installation

```bash
npm install --save-dev swc-plugin-barrel-files
# or
yarn add --dev swc-plugin-barrel-files
# or
pnpm add --save-dev swc-plugin-barrel-files
```

## Compatibility

| Plugin Versions | Runtime Version Ranges                                           |
| --------------- | ---------------------------------------------------------------- |
| >=0.1.0 <0.3.0  | [Compatibility table](https://plugins.swc.rs/versions/range/10)  |
| >=0.3.0         | [Compatibility table](https://plugins.swc.rs/versions/range/364) |

## Configuration

### Basic Configuration

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

This configuration will transform imports from any file matching the pattern `src/modules/*/index.ts`.

**Important:** When specifying patterns and paths, you must include the full file path including the filename (e.g., `index.ts`). The plugin does not implement full file system resolution logic, so it relies on exact path matching.

Examples of correct and incorrect patterns:

```json
// Correct - includes the full path with filename
{
  "patterns": [
    "src/modules/*/index.ts"
  ]
}

// Incorrect - missing filename
{
  "patterns": [
    "src/modules/*/"
  ]
}
```

The same applies to import statements in your code. When importing from barrel files, make sure your imports match the patterns exactly as configured.

Examples of correct and incorrect imports in code:

```typescript
// Assuming configuration with pattern: "src/modules/*/index.ts"
// and alias pattern: "@modules/*" -> "src/modules/*/index.ts"

// Correct imports
import { User } from "./modules/user/index.ts"; // Full path with filename
import { User } from "@modules/user"; // Using alias (plugin resolves to full path)

// Incorrect imports
import { User } from "./modules/user"; // Missing filename
import { User } from "./modules/user/"; // Missing filename with trailing slash
```

The plugin will only transform imports that exactly match the configured patterns, including the full file path with filename.

### Configuration with Import Aliases

If you're using path aliases (similar to TypeScript's `paths` configuration), you can configure them in the plugin:

```json
{
    "jsc": {
        "experimental": {
            "plugins": [
                [
                    "swc-plugin-barrel-files",
                    {
                        "patterns": ["src/modules/*/index.ts"],
                        "aliases": [
                            {
                                "pattern": "@modules/*",
                                "paths": ["src/modules/*/index.ts"]
                            }
                        ]
                    }
                ]
            ]
        }
    }
}
```

This configuration is similar to TypeScript's `paths` configuration in `tsconfig.json`:

```json
{
    "compilerOptions": {
        "paths": {
            "@modules/*": ["src/modules/*"]
        }
    }
}
```

The plugin will transform imports like:

```typescript
import { User, createUser } from "@modules/user";
```

into:

```typescript
import { User } from "src/modules/user/models/User";
import { createUser } from "src/modules/user/api/createUser";
```

### Context-Specific Aliases

You can limit aliases to specific directories using the `context` option. This allows the same alias to resolve to different paths depending on where the import statement is written:

```json
{
    "jsc": {
        "experimental": {
            "plugins": [
                [
                    "swc-plugin-barrel-files",
                    {
                        "patterns": ["src/modules/*/index.ts"],
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
                ]
            ]
        }
    }
}
```

### Symlinks Configuration

The plugin supports symlinks configuration to work with external files and directories outside the current working directory. This is particularly useful in monorepo setups or when working with external libraries that need barrel file optimization.

**Note**: Due to SWC plugin limitations, the plugin can only access files within the current working directory. The symlinks feature works by mapping external paths to internal symlinked paths, allowing you to process external barrel files as if they were inside your project.

```json
{
    "jsc": {
        "experimental": {
            "plugins": [
                [
                    "swc-plugin-barrel-files",
                    {
                        "patterns": ["src/modules/*/index.ts"],
                        "symlinks": {
                            "../external-lib/index.ts": "./node_modules/external-lib/index.ts",
                            "../shared-workspace": "./symlinks/workspace/src"
                        }
                    }
                ]
            ]
        }
    }
}
```

The symlinks configuration maps external paths (outside the current working directory) to internal symlinked paths (inside the current working directory). When the plugin encounters an import from an external path, it:

1. Checks if the path matches any symlink mapping
2. Resolves it to the internal symlinked path
3. Processes the barrel file using the internal path
4. Generates optimized direct imports

#### File-Level vs Directory-Level Symlinks

You can configure symlinks at both file and directory levels:

```json
{
    "symlinks": {
        // File-level symlink: maps a specific external file to an internal file
        "../external-lib/index.ts": "./node_modules/external-lib/index.ts",

        // Directory-level symlink: maps an entire external directory to an internal directory
        "../shared-workspace": "./symlinks/workspace/src"
    }
}
```

**Priority**: File-level symlinks take priority over directory-level symlinks. If both match a path, the file-level symlink will be used.

#### Example Usage

Given the configuration above, these imports:

```typescript
import { Button, Input } from "../external-lib/index.ts";
import { AuthService } from "../shared-workspace/features/auth/index.ts";
```

Will be resolved as if they were:

```typescript
// Resolved through file-level symlink
import { Button, Input } from "./node_modules/external-lib/index.ts";

// Resolved through directory-level symlink
import { AuthService } from "./symlinks/workspace/src/features/auth/index.ts";
```

#### Integration with Aliases

Symlinks work seamlessly with aliases. You can combine both features:

```json
{
    "patterns": ["src/modules/*/index.ts"],
    "aliases": [
        {
            "pattern": "@external/*",
            "paths": ["../external-lib/src/*/index.ts"]
        }
    ],
    "symlinks": {
        "../external-lib": "./node_modules/external-lib"
    }
}
```

This allows you to use clean alias imports that resolve through symlinks:

```typescript
import { Button } from "@external/components";

// resolves via alias and symlink
import { Button } from "./node_modules/external-lib/components/index.ts";
```

### Error Handling Configuration

The plugin provides configurable error handling for unsupported import patterns and invalid barrel files:

```json
{
    "jsc": {
        "experimental": {
            "plugins": [
                [
                    "swc-plugin-barrel-files",
                    {
                        "patterns": ["src/modules/*/index.ts"],
                        "unsupported_import_mode": "warn",
                        "invalid_barrel_mode": "error"
                    }
                ]
            ]
        }
    }
}
```

#### `unsupported_import_mode`

Controls how the plugin handles unsupported import patterns like namespace imports (`import * as x from 'y'`).

- **`"error"`** (default): Throws an error and stops compilation
- **`"warn"`**: Prints a warning and skips the import (leaves it unchanged)
- **`"off"`**: Silently skips the import (leaves it unchanged)

#### `invalid_barrel_mode`

Controls how the plugin handles invalid barrel files (files that contain unsupported constructs like wildcard exports, variable declarations, etc.).

- **`"error"`** (default): Throws an error and stops compilation
- **`"warn"`**: Prints a warning and skips the import (leaves it unchanged)
- **`"off"`**: Silently skips the import (leaves it unchanged)

These options allow you to gradually adopt the plugin by treating errors as warnings during development.

## Limitations

### ESM Syntax Only

The plugin only supports ESM syntax (import/export statements) and does not support:

- Dynamic imports (`import()`)
- CommonJS syntax (`require()`)
- Namespace imports (`import * as x from 'y'`)
- Wildcard exports (`export * from './module'`)
- Namespace exports (`export * as x from './module'`)

### Barrel Files Format

Barrel files used with this plugin must adhere to the following format requirements:

- Must be valid JavaScript/TypeScript modules
- Must contain only re-export statements and no other code
- Should use named re-exports in one of these formats:

    ```typescript
    // Standard named re-exports
    export { ComponentA, ComponentB } from "./components";

    // Default export re-exports
    export { default as Component } from "./Component";
    ```

The plugin does not support barrel files that:

- Contain any non-export statements
- Include runtime code or initialization logic
- Use dynamic exports or conditional export patterns

### Side Effects

Side effects in JavaScript/TypeScript are operations that affect state outside their local scope, such as modifying global variables or making API calls.

**Important**: If a barrel file re-exports modules that produce side effects (e.g., modules listed in the `sideEffects` field of package.json), this plugin will still transform those imports into direct imports. This transformation might cause bundlers to incorrectly eliminate code with side effects during tree-shaking.

For example:

```typescript
// barrel file: index.ts
export { Button } from "./Button"; // Button.ts has side effects

// Your code
import { Button } from "./ui"; // This import triggers Button.ts side effects

// After transformation
import { Button } from "./ui/Button"; // Still triggers side effects correctly
```

If you have modules with important side effects, consider:

- Explicitly importing those modules separately
- Testing thoroughly after implementing this plugin

### Path Limitations

All paths in the configuration must be relative to the current working directory. This is a limitation of the SWC plugin system, which only provides access to the current working directory inside the WASM runtime.

For example:

```json
// Correct
{
  "patterns": [
    "src/modules/*/index.ts"
  ]
}

// Also correct
{
  "patterns": [
    "./src/modules/*/index.ts"
  ]
}

// Correct for absolute paths that start with the current working directory
{
  "patterns": [
    "/Users/username/project/src/modules/*/index.ts"
  ]
}

// Incorrect - paths outside the current working directory
{
  "patterns": [
    "../other-project/src/modules/*/index.ts"
  ]
}

// Incorrect - absolute paths outside the current working directory
{
  "patterns": [
    "/Users/username/other-project/src/modules/*/index.ts"
  ]
}
```

## Integration with Jest

### Configuration

To use this plugin with Jest, you need to configure Jest to use SWC for transformation:

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
                                    aliases: [
                                        {
                                            pattern: "@modules/*",
                                            paths: ["src/modules/*/index.ts"],
                                        },
                                    ],
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

### Jest.mock and Path Mismatches

When using `jest.mock()` with this plugin, you may encounter issues because the paths in your code will be transformed, but the paths in `jest.mock()` calls will not. This can lead to mismatches between the paths used in your code and the paths used in your mocks.

For example:

```typescript
// Your code
import { User } from "@modules/user";

// Jest will transform this to:
import { User } from "./src/modules/user/models/User";

// But if you have a jest.mock call:
jest.mock("@modules/user", () => ({
    User: jest.fn(),
}));

// This will not match the transformed import path
```

To work around this issue, you can use `jest.mock()` with the original source path instead of the barrel file:

```typescript
jest.mock("./src/modules/user/models/User", () => ({
    User: jest.fn(),
}));
```

## Common Errors and Troubleshooting

### Specific Error Codes

The plugin may throw the following specific error codes:

#### E_INVALID_ENV

**Error message**: "Current working directory is not available" or "Current filename is not available"

**Cause**: The plugin cannot access the context needed for operation.

**Solution**:

- Check that your build tool is properly integrated with SWC

#### E_INVALID_CONFIG

**Error message**: "Error parsing barrel plugin configuration"

**Cause**: The plugin configuration is invalid or malformed.

**Solution**:

- Check your configuration JSON syntax
- Ensure all required fields are present
- Verify that patterns and aliases are correctly formatted

#### E_NO_NAMESPACE_IMPORTS

**Error message**: "Namespace imports are not supported for barrel file optimization"

**Cause**: You're using namespace imports (`import * as x from 'y'`), which are not supported.

**Solution**:

- Convert namespace imports to named imports
- Set `unsupported_import_mode` to `"warn"` or `"off"` to handle these imports gracefully
- See the [Limitations](#limitations) section for details on supported syntax

#### E_UNRESOLVED_EXPORTS

**Error message**: "The following exports were not found in the barrel file: ..." or "No re-exports found in barrel file: ..."

**Cause**: The plugin cannot find the specified exports in the barrel file.

**Solution**:

- Check that the barrel file actually exports the symbols you're importing
- Verify that the barrel file contains valid re-export statements
- Ensure the barrel file is correctly identified by the plugin

#### E_FILE_READ

**Error message**: "Failed to load file: ..."

**Cause**: The plugin cannot read a file that it needs to process.

**Solution**:

- Check file permissions
- Verify that the file exists at the specified path
- Ensure the path is relative to the current working directory

#### E_FILE_PARSE

**Error message**: "Failed to parse file: ..."

**Cause**: The plugin cannot parse a file as valid TypeScript/JavaScript.

**Solution**:

- Check for syntax errors in the file
- Ensure the file is a valid TypeScript/JavaScript file
- Verify that the file encoding is correct

#### E_INVALID_BARREL_FILE

**Error message**: "Invalid barrel file ...: ..."

**Cause**: The barrel file contains unsupported constructs.

**Solution**:

- Ensure the barrel file only contains re-export statements
- Remove any non-export code from the barrel file
- Convert wildcard exports to named exports
- Set `invalid_barrel_mode` to `"warn"` or `"off"` to handle these files gracefully

#### E_INVALID_FILE_PATH

**Error message**: "Absolute paths not starting with cwd are not supported: ..."

**Cause**: You're using absolute paths that don't start with the current working directory.

**Solution**:

- Use relative paths instead of absolute paths
- If using absolute paths, ensure they start with the current working directory
- See the [Path Limitations](#path-limitations) section for details

#### E_BARREL_FILE_NOT_FOUND

**Error message**: "Could not resolve barrel file for import alias ..."

**Cause**: The plugin cannot find a barrel file that matches the alias pattern.

**Solution**:

- Check that the barrel file exists at the path specified in the alias configuration
- Verify that the alias pattern correctly matches the import path
- Ensure the paths in your alias configuration match your file structure

### Path Pattern Mismatches

**Error**: Imports are not being transformed as expected.

**Possible causes**:

- The pattern in your configuration doesn't match the actual file paths
- The import statement in your code doesn't match the configured pattern exactly

**Solution**:

- Ensure your patterns include the full file path with filename (e.g., `src/modules/*/index.ts`)
- Check that your import statements match the patterns exactly
- Use the exact same path format in both your configuration and imports

```typescript
// If your pattern is "src/modules/*/index.ts"
// This will work:
import { User } from "./modules/user/index.ts";
// This won't work:
import { User } from "./modules/user";
```

### Alias Resolution Failures

**Error**: Imports using aliases are not being transformed.

**Possible causes**:

- Alias pattern doesn't match the import statement
- The context restriction is preventing the alias from being applied
- The alias paths don't match the actual file structure

**Solution**:

- Verify that your alias patterns match your import statements
- Check if you've restricted the alias to specific contexts
- Ensure the paths in your alias configuration match your file structure

### Unsupported Syntax

**Error**: Some imports or exports are not being transformed.

**Possible causes**:

- Using unsupported syntax like dynamic imports, namespace imports, or wildcard exports
- Using CommonJS require() instead of ESM imports

**Solution**:

- Convert unsupported syntax to supported ESM syntax
- See the [Limitations](#limitations) section for details on supported syntax

### Path Resolution Issues

**Error**: The plugin can't find the source files referenced in barrel files.

**Possible causes**:

- The paths in your barrel files don't match your file structure
- The plugin doesn't have access to the correct working directory
- Using paths outside the current working directory

**Solution**:

- Ensure all paths are relative to the current working directory
- Check that the paths in your barrel files match your actual file structure
- Avoid using paths that go outside the current working directory

### Build Tool Integration Issues

**Error**: The plugin doesn't work with your build tool (webpack, Vite, etc.).

**Possible causes**:

- Incorrect SWC configuration in your build tool
- Conflicting plugins or transformations
- Build tool doesn't support SWC plugins

**Solution**:

- Check your build tool's documentation for SWC integration
- Ensure the plugin is correctly configured in your build tool
- Verify that your build tool supports SWC plugins

### Jest Integration Issues

**Error**: Tests fail with module not found errors after transformation.

**Possible causes**:

- Path mismatches between transformed imports and jest.mock calls
- Jest configuration doesn't include the plugin

**Solution**:

- Use the approaches described in the [Jest Integration](#integration-with-jest) section
- Ensure your Jest configuration includes the plugin
- Consider using jest.requireActual() to handle transformed paths

## How It Works

The plugin works by:

1. Identifying imports that match the configured patterns
2. Analyzing the barrel files to find the original source files
3. Transforming the imports to directly reference the original source files

This happens at compile time, so there's no runtime overhead.

## Contributing

This project uses [changesets](https://github.com/changesets/changesets) to manage versions and generate changelogs.

### Making Changes

1. Fork the repository and create a new branch for your changes
2. Make your changes to the codebase
3. Add a changeset to describe your changes:

```bash
pnpm changeset
```

This will prompt you to:

- Select the type of change (patch, minor, or major)
- Write a summary of the changes (this will appear in the changelog)

4. Commit the changeset along with your changes
5. Create a pull request

### Release Process

Releases are automated through GitHub Actions. When changes are merged to the main branch:

1. A GitHub Action will create a PR to update versions and changelogs
2. Once this PR is approved and merged, another GitHub Action will publish the new version to npm

## License

MIT
