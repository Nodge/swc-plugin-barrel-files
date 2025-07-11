---
"swc-plugin-barrel-files": minor
---

Performance optimizations for pattern matching and caching

- Replace regex-based pattern matching with custom implementation for faster matching
- Add caching for barrel file parsing to avoid re-parsing same files
- Add caching for file existence checks to reduce filesystem calls
- Pre-compile patterns and aliases for better performance
- Remove regex dependency to reduce bundle size
- Refactor process_import function for better code organization

These optimizations result in approximately 5x faster processing of barrel file imports.
