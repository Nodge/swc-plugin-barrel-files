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
}
