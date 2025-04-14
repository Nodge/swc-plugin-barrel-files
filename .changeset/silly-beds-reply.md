---
"swc-plugin-barrel-files": patch
---

fix: Skip files outside cwd

Skip transformation for files located outside the current working directory (cwd) to prevent errors due to WASM path restrictions. Added a test case to verify this behavior.
