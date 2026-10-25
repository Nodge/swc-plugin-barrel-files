---
"swc-plugin-barrel-files": minor
---

Update swc_core to 82.2.0 and remove the `symlinks` option

The plugin now targets `swc_plugin_runner` 34 (`@swc/core` 1.16, Next.js 16.4, Rspack 2.2).

**Breaking:** the `symlinks` option is removed. Current SWC hosts run plugins in a WASI sandbox that does not follow symlinks, so a barrel file reached through a symlink cannot be read, and remapping paths onto symlinks never worked there. A config that still sets `symlinks` now fails with `E_INVALID_CONFIG`. For monorepos, run SWC from the repository root and point `patterns` and `aliases` at real paths (`packages/lib/src/...`, not `node_modules/lib/src/...`).
