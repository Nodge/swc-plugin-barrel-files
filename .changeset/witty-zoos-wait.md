---
"swc-plugin-barrel-files": patch
---

Fix symlink path resolution to handle both absolute and relative paths correctly

This change improves the PathResolver to properly normalize symlink configurations by converting absolute paths to relative paths when needed, and ensures that both absolute and relative input paths are handled consistently during symlink resolution.
