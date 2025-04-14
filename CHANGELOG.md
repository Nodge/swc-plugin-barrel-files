# swc-plugin-barrel-files

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
