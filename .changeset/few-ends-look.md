---
"swc-plugin-barrel-files": minor
---

Add symlinks configuration support for external barrel files

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
