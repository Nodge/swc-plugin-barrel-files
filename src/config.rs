use serde::Deserialize;

/// Configuration for the barrel files plugin
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Patterns for barrel files
    pub patterns: Vec<String>,

    /// Rules for resolving import aliases (optional)
    pub aliases: Option<Vec<Alias>>,

    /// Cache duration in milliseconds (optional, defaults to 1000)
    pub cache_duration_ms: Option<u64>,
}

/// Rule for resolving import aliases
#[derive(Debug, Deserialize, Clone)]
pub struct Alias {
    /// Pattern to match against import paths.
    pub pattern: String,
    /// Paths to resolve the matched imports to.
    pub paths: Vec<String>,
    /// Directories for which the alias should be applied (optional).
    pub context: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing() {
        let config_json = r#"{
            "aliases": [
                {
                    "pattern": "@features/*",
                    "paths": ["src/features/*/index.ts"],
                    "context": ["src"]
                }
            ],
            "patterns": [
                "src/entities/*/index.ts",
                "src/features/*/index.ts"
            ],
            "cache_duration_ms": 1000
        }"#;

        let config: Config =
            serde_json::from_str(config_json).expect("Failed to parse config JSON");

        assert!(config.aliases.is_some());
        let aliases = config.aliases.unwrap();
        assert_eq!(aliases.len(), 1);
        assert_eq!(aliases[0].pattern, "@features/*");
        assert_eq!(&aliases[0].paths, &vec!["src/features/*/index.ts"]);
        let context = aliases[0].context.as_ref().unwrap();
        assert_eq!(context, &vec!["src"]);
        let patterns = config.patterns;
        assert_eq!(patterns.len(), 2);
        assert_eq!(patterns[0], "src/entities/*/index.ts");
        assert_eq!(patterns[1], "src/features/*/index.ts");
        assert_eq!(config.cache_duration_ms, Some(1000));
    }
}
