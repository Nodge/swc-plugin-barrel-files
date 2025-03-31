//! Alias resolution module for the barrel files plugin
//!
//! This module provides functionality for resolving import paths using aliases
//! and finding corresponding barrel files. It handles pattern matching and path resolution
//! to support dynamic imports and re-exports in the barrel files system.

use std::path::Path;

use crate::config::{Alias, Config};
use crate::paths::to_virtual_path;
use crate::pattern_matcher::{
    apply_components_to_template, count_wildcards, extract_pattern_components, path_matches_pattern,
};

/// Resolver for import aliases
pub struct AliasResolver {
    /// Compilation working directory
    cwd: String,

    /// Alises sorted by specificity (fewer wildcards first)
    sorted_alises: Vec<Alias>,
}

impl AliasResolver {
    /// Creates a new visitor with the specified configuration
    pub fn new(config: &Config, cwd: String) -> Self {
        // Pre-sort rules by specificity (fewer wildcards = more specific)
        let sorted_alises = match &config.aliases {
            Some(rules) => {
                let mut sorted = rules.clone();
                sorted.sort_by_key(|rule| count_wildcards(&rule.pattern));
                sorted
            }
            None => Vec::new(),
        };

        AliasResolver { cwd, sorted_alises }
    }

    /// Resolves an import path using configured aliases
    ///
    /// This function attempts to match the import path against configured alias patterns
    /// and resolve it to an actual file path. It tries each potential path template
    /// until it finds one that exists in the filesystem.
    ///
    /// # Arguments
    ///
    /// * `import_path` - The import path to resolve
    ///
    /// # Returns
    ///
    /// * `Ok(Some(String))` - The resolved file path if found
    /// * `Ok(None)` - If no matching alias was found or no matching file exists
    /// * `Err(String)` - If there was an error during resolution
    pub fn resolve(&self, import_path: &str) -> Result<Option<String>, String> {
        if let Some(alias) = self.match_pattern(import_path) {
            let components = extract_pattern_components(import_path, &alias.pattern);

            for path_template in alias.paths.iter() {
                let resolved_path = apply_components_to_template(path_template, &components);
                let path = to_virtual_path(&self.cwd, &resolved_path)?;

                if Path::new(&path).exists() {
                    return Ok(Some(path));
                }
            }

            return Err(format!(
                "E_BARREL_FILE_NOT_FOUND: Could not resolve barrel file for import alias {}",
                import_path,
            ));
        }

        Ok(None)
    }

    /// Matches an import path against the configured patterns
    ///
    /// # Arguments
    ///
    /// * `import_path` - The import path to match
    ///
    /// # Returns
    ///
    /// The matching rule if found, `None` otherwise
    fn match_pattern(&self, import_path: &str) -> Option<&Alias> {
        if self.sorted_alises.is_empty() {
            return None;
        }

        // todo: respect context

        self.sorted_alises
            .iter()
            .find(|alias| path_matches_pattern(import_path, &alias.pattern))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_pattern() {
        let rule1 = Alias {
            pattern: "#features/*".to_string(),
            paths: vec!["src/features/*/index.ts".to_string()],
            context: None,
        };

        let rule2 = Alias {
            pattern: "#features/*/testing".to_string(),
            paths: vec!["src/features/*/testing.ts".to_string()],
            context: None,
        };

        let config = Config {
            aliases: Some(vec![rule2.clone(), rule1.clone()]),
            patterns: vec![],
            cache_duration_ms: Some(1000),
        };

        let visitor = AliasResolver::new(&config, "/".to_string());

        // The more specific rule should be first in sorted_rules
        assert_eq!(visitor.sorted_alises[0].pattern, "#features/*/testing");
        assert_eq!(visitor.sorted_alises[1].pattern, "#features/*");

        // Test matching
        let matched = visitor.match_pattern("#features/some");
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().pattern, "#features/*");

        let matched = visitor.match_pattern("#features/some/testing");
        assert!(matched.is_some());
        assert_eq!(matched.unwrap().pattern, "#features/*/testing");

        let matched = visitor.match_pattern("#other/some");
        assert!(matched.is_none());
    }
}
