---
"swc-plugin-barrel-files": minor
---

Add configuration options for handling unsupported import patterns and invalid barrel files.

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
