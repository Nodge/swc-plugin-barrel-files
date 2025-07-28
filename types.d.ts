/**
 * Configuration for a single alias.
 */
export interface AliasConfig {
    /**
     * The pattern to match for the alias.
     */
    pattern: string;
    /**
     * The paths to replace the matched pattern with, relative to the current working directory.
     */
    paths: string[];
    /**
     * An optional context to limit the alias to specific files or directories.
     */
    context?: string[];
}

/**
 * Configuration for the plugin.
 */
export interface PluginConfig {
    /**
     * An array of paths to barrel files relative to the current working directory.
     */
    patterns: string[];
    /**
     * An optional array of alias configurations.
     */
    aliases?: AliasConfig[];
    /**
     * An optional mapping of external paths to internal symlinked paths.
     * This allows the plugin to process barrel files from external locations
     * by mapping them to internal symlinked paths within the current working directory.
     *
     * @example
     * {
     *   "../shared-workspace/some-pkg/src/features": "./node_modules/some-pkg/src/features"
     * }
     */
    symlinks?: Record<string, string>;
    /**
     * Enables debug logging to stdout.
     * @default false
     */
    debug?: boolean;
    /**
     * How to handle unsupported import patterns (e.g. namespace imports).
     * @default "error"
     */
    unsupported_import_mode?: "error" | "warn" | "off";
    /**
     * How to handle invalid barrel files (files with unsupported constructs).
     * @default "error"
     */
    invalid_barrel_mode?: "error" | "warn" | "off";
}
