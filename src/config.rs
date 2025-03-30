use serde::Deserialize;

/// Configuration for the barrel files plugin
#[derive(Deserialize, Debug)]
pub struct Config {
    /// Rules for pattern matching (optional)
    pub rules: Option<Vec<Rule>>,

    /// Cache duration in milliseconds (optional, defaults to 1000)
    pub cache_duration_ms: Option<u64>,
}

/// Rule for matching import paths and resolving barrel files
#[derive(Deserialize, Debug, Clone)]
pub struct Rule {
    /// Pattern to match (e.g., '#entities/*')
    pub pattern: String,

    /// Possible paths to resolve (e.g., ['src/entities/*/index.ts'])
    pub paths: Vec<String>,
}
