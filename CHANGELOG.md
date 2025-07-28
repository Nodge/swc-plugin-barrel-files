# swc-plugin-barrel-files

## 0.5.0

### Minor Changes

- [#19](https://github.com/Nodge/swc-plugin-barrel-files/pull/19) [`de407fa`](https://github.com/Nodge/swc-plugin-barrel-files/commit/de407fa16685b029c5f0af56ce060c74f40d2913) Thanks [@Nodge](https://github.com/Nodge)! - Add symlinks configuration support for external barrel files

  This feature introduces a new `symlinks` configuration option that enables the plugin to work with external files and directories outside the current working directory. This is particularly useful in monorepo setups or when working with external libraries that need barrel file optimization.

  This feature overcomes SWC plugin limitations that restrict file access to the current working directory, enabling barrel file optimization for shared workspace modules.

  **Configuration Example:**

  ```json
  {
    "symlinks": {
      "../external-lib/index.ts": "./node_modules/external-lib/index.ts",
      "../shared-workspace": "./symlinks/workspace/src"
    }
  }
  ```

  **How it Works:**

  When the plugin encounters an import from an external path, it:
  1. Checks if the path matches any symlink mapping
  2. Resolves it to the internal symlinked path
  3. Processes the barrel file using the internal path
  4. Generates optimized direct imports

### Patch Changes

- [#21](https://github.com/Nodge/swc-plugin-barrel-files/pull/21) [`59b55a3`](https://github.com/Nodge/swc-plugin-barrel-files/commit/59b55a3b1ce6697487b6b1cb8b6f50f6496eef3f) Thanks [@Nodge](https://github.com/Nodge)! - Preserve import order when transforming barrel file imports

  This change ensures that when barrel file imports are transformed, the resulting imports maintain the order defined in the barrel file rather than being alphabetically sorted. This provides more predictable and consistent output that respects the original barrel file's export ordering.

## 0.4.0

### Minor Changes

- [#16](https://github.com/Nodge/swc-plugin-barrel-files/pull/16) [`5af9272`](https://github.com/Nodge/swc-plugin-barrel-files/commit/5af92722f5d65d2f67243945e5d173c68ce31e11) Thanks [@Nodge](https://github.com/Nodge)! - Add configuration options for handling unsupported import patterns and invalid barrel files.

  ## New Configuration Options

  ### `unsupported_import_mode`

  Controls how the plugin handles unsupported import patterns (e.g., namespace imports like `import * as foo from 'bar'`):
  - `"error"` (default): Throws an error and stops compilation
  - `"warn"`: Prints a warning to stderr and skips the import transformation
  - `"off"`: Silently skips the import transformation

  ### `invalid_barrel_mode`

  Controls how the plugin handles invalid barrel files (files with unsupported constructs like wildcard exports, default exports, etc):
  - `"error"` (default): Throws an error and stops compilation
  - `"warn"`: Prints a warning to stderr and skips the barrel file processing
  - `"off"`: Silently skips the barrel file processing

  ## Examples

  ```json
  {
    "patterns": ["src/features/*/index.ts"],
    "unsupported_import_mode": "warn",
    "invalid_barrel_mode": "off"
  }
  ```

## 0.3.0

### Minor Changes

- [#4](https://github.com/Nodge/swc-plugin-barrel-files/pull/4) [`2283987`](https://github.com/Nodge/swc-plugin-barrel-files/commit/22839874e4830c8b52e7cfe99d76e1b8af76ada9) Thanks [@Nodge](https://github.com/Nodge)! - Update swc_core to 31.1.0

  This update brings compatibility with the latest SWC core version, ensuring the plugin works with newer versions of SWC-based tools. The update maintains all existing functionality while adapting to the SWC core API changes.

  Supported tool versions:
  - @swc/core: >=1.12.0
  - rspack: >=1.4.0
  - next: >=v15.5.0

  For detailed compatibility information with the new core, refer to: https://plugins.swc.rs/versions/range/364

## 0.2.0

### Minor Changes

- [#13](https://github.com/Nodge/swc-plugin-barrel-files/pull/13) [`97c946e`](https://github.com/Nodge/swc-plugin-barrel-files/commit/97c946ee38325ef9104bf6a8d4a6eddd8f241b49) Thanks [@Nodge](https://github.com/Nodge)! - Performance optimizations for pattern matching and caching
  - Replace regex-based pattern matching with custom implementation for faster matching
  - Add caching for barrel file parsing to avoid re-parsing same files
  - Add caching for file existence checks to reduce filesystem calls
  - Pre-compile patterns and aliases for better performance
  - Remove regex dependency to reduce bundle size
  - Refactor process_import function for better code organization

  These optimizations result in approximately 5x faster processing of barrel file imports.

## 0.1.3

### Patch Changes

- [#10](https://github.com/Nodge/swc-plugin-barrel-files/pull/10) [`7c4df3b`](https://github.com/Nodge/swc-plugin-barrel-files/commit/7c4df3bc489dd49a58737498a6dcba667a0843b6) Thanks [@Nodge](https://github.com/Nodge)! - feat: Add debug logging option

  Introduces a new `debug` option to the plugin configuration. When set to `true`, the plugin will output detailed logs to stdout during the transformation process, aiding in debugging configuration issues and understanding the plugin's behavior.

## 0.1.2

### Patch Changes

- [#5](https://github.com/Nodge/swc-plugin-barrel-files/pull/5) [`93b9e12`](https://github.com/Nodge/swc-plugin-barrel-files/commit/93b9e123281f87b13a8ae52edc53e1c6e6b28479) Thanks [@dependabot](https://github.com/apps/dependabot)! - Bump vite from 6.2.2 to 6.2.5

- [#8](https://github.com/Nodge/swc-plugin-barrel-files/pull/8) [`bc83d6a`](https://github.com/Nodge/swc-plugin-barrel-files/commit/bc83d6afc494959c5cd734a88b4222aecd89cabd) Thanks [@Nodge](https://github.com/Nodge)! - fix: Skip files outside cwd

  Skip transformation for files located outside the current working directory (cwd) to prevent errors due to WASM path restrictions. Added a test case to verify this behavior.

## 0.1.1

### Patch Changes

- [#1](https://github.com/Nodge/swc-plugin-barrel-files/pull/1) [`f15d4b8`](https://github.com/Nodge/swc-plugin-barrel-files/commit/f15d4b84bc56f26eb603248e14234f834fa40f93) Thanks [@Nodge](https://github.com/Nodge)! - Implement automated release workflow.
