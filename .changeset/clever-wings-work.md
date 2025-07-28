---
"swc-plugin-barrel-files": patch
---

Preserve import order when transforming barrel file imports

This change ensures that when barrel file imports are transformed, the resulting imports maintain the order defined in the barrel file rather than being alphabetically sorted. This provides more predictable and consistent output that respects the original barrel file's export ordering.
