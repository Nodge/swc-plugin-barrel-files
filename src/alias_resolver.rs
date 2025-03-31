//! Alias resolution module for the barrel files plugin
//!
//! This module provides functionality for resolving import paths using aliases
//! and finding corresponding barrel files. It handles pattern matching and path resolution
//! to support dynamic imports and re-exports in the barrel files system.

use std::path::Path;

use crate::config::{Alias, Config};
use crate::paths::{path_join, to_virtual_path};
use crate::pattern_matcher::{
    apply_components_to_template, count_wildcards, extract_pattern_components, path_matches_pattern,
};

/// Resolver for import aliases
pub struct AliasResolver {
    /// Compilation working directory
    cwd: String,

    /// Alises sorted by specificity (fewer wildcards first)
    aliases: Vec<Alias>,
}

impl AliasResolver {
    /// Creates a new visitor with the specified configuration
    pub fn new(config: &Config, cwd: &str, source_file: &str) -> Result<Self, String> {
        let mut aliases = Vec::new();

        // Filter aliases by context
        for alias in config.aliases.as_ref().unwrap_or(&Vec::new()) {
            match &alias.context {
                None => {
                    aliases.push(alias.clone());
                }
                Some(context) => {
                    for ctx in context {
                        let joined_path = path_join(cwd, ctx);
                        let virtual_path = to_virtual_path(cwd, &joined_path)?;

                        if source_file.starts_with(&virtual_path) {
                            aliases.push(alias.clone());
                            break;
                        }
                    }
                }
            }
        }

        // Pre-sort rules by specificity (fewer wildcards = more specific)
        aliases.sort_by_key(|rule| count_wildcards(&rule.pattern));

        Ok(AliasResolver {
            cwd: cwd.to_string(),
            aliases,
        })
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
        if self.aliases.is_empty() {
            return None;
        }

        self.aliases
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
        };

        let cwd = "/".to_string();
        let source_file = "/some/file".to_string();
        let visitor = AliasResolver::new(&config, &cwd, &source_file).unwrap();

        // The more specific rule should be first in sorted_rules
        assert_eq!(visitor.aliases[0].pattern, "#features/*/testing");
        assert_eq!(visitor.aliases[1].pattern, "#features/*");

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

    #[test]
    fn test_context_filtering() {
        // Create aliases with different context configurations
        let no_context_alias = Alias {
            pattern: "#no-context/*".to_string(),
            paths: vec!["src/no-context/*/index.ts".to_string()],
            context: None,
        };

        let matching_context_alias = Alias {
            pattern: "#matching-context/*".to_string(),
            paths: vec!["src/matching-context/*/index.ts".to_string()],
            context: Some(vec!["/cwd/src".to_string()]),
        };

        let non_matching_context_alias = Alias {
            pattern: "#non-matching-context/*".to_string(),
            paths: vec!["src/non-matching-context/*/index.ts".to_string()],
            context: Some(vec!["/cwd/other".to_string()]),
        };

        let multiple_contexts_alias = Alias {
            pattern: "#multiple-contexts/*".to_string(),
            paths: vec!["src/multiple-contexts/*/index.ts".to_string()],
            context: Some(vec!["/cwd/other".to_string(), "/cwd/src".to_string()]),
        };

        // Create config with all aliases
        let config = Config {
            aliases: Some(vec![
                no_context_alias.clone(),
                matching_context_alias.clone(),
                non_matching_context_alias.clone(),
                multiple_contexts_alias.clone(),
            ]),
            patterns: vec![],
        };

        // Test with source file in /cwd/src
        let cwd = "/cwd".to_string();
        let source_file = "/cwd/src/components/Button.tsx".to_string();
        let resolver = AliasResolver::new(&config, &cwd, &source_file).unwrap();

        // Verify that aliases with no context or matching context are included
        assert_eq!(resolver.aliases.len(), 3);

        // Check if the correct aliases were included
        let patterns: Vec<String> = resolver.aliases.iter().map(|a| a.pattern.clone()).collect();
        assert!(patterns.contains(&no_context_alias.pattern));
        assert!(patterns.contains(&matching_context_alias.pattern));
        assert!(patterns.contains(&multiple_contexts_alias.pattern));

        // Check that non-matching context alias is excluded
        assert!(!patterns.contains(&non_matching_context_alias.pattern));
    }

    #[test]
    fn test_context_filtering_with_different_source_file() {
        // Create aliases with different context configurations
        let no_context_alias = Alias {
            pattern: "#no-context/*".to_string(),
            paths: vec!["src/no-context/*/index.ts".to_string()],
            context: None,
        };

        let matching_context_alias = Alias {
            pattern: "#matching-context/*".to_string(),
            paths: vec!["src/matching-context/*/index.ts".to_string()],
            context: Some(vec!["/cwd/src".to_string()]),
        };

        let other_context_alias = Alias {
            pattern: "#other-context/*".to_string(),
            paths: vec!["src/other-context/*/index.ts".to_string()],
            context: Some(vec!["/cwd/other".to_string()]),
        };

        // Create config with all aliases
        let config = Config {
            aliases: Some(vec![
                no_context_alias.clone(),
                matching_context_alias.clone(),
                other_context_alias.clone(),
            ]),
            patterns: vec![],
        };

        // Test with source file in /cwd/other
        let cwd = "/cwd".to_string();
        let source_file = "/cwd/other/components/Button.tsx".to_string();
        let resolver = AliasResolver::new(&config, &cwd, &source_file).unwrap();

        // Verify that aliases with no context or matching context are included
        assert_eq!(resolver.aliases.len(), 2);

        // Check if the correct aliases were included
        let patterns: Vec<String> = resolver.aliases.iter().map(|a| a.pattern.clone()).collect();
        assert!(patterns.contains(&no_context_alias.pattern));
        assert!(patterns.contains(&other_context_alias.pattern));

        // Check that non-matching context alias is excluded
        assert!(!patterns.contains(&matching_context_alias.pattern));
    }

    #[test]
    fn test_context_filtering_with_no_matching_context() {
        // Create aliases with different context configurations
        let no_context_alias = Alias {
            pattern: "#no-context/*".to_string(),
            paths: vec!["src/no-context/*/index.ts".to_string()],
            context: None,
        };

        let src_context_alias = Alias {
            pattern: "#src-context/*".to_string(),
            paths: vec!["src/src-context/*/index.ts".to_string()],
            context: Some(vec!["/cwd/src".to_string()]),
        };

        let other_context_alias = Alias {
            pattern: "#other-context/*".to_string(),
            paths: vec!["src/other-context/*/index.ts".to_string()],
            context: Some(vec!["/cwd/other".to_string()]),
        };

        // Create config with all aliases
        let config = Config {
            aliases: Some(vec![
                no_context_alias.clone(),
                src_context_alias.clone(),
                other_context_alias.clone(),
            ]),
            patterns: vec![],
        };

        // Test with source file in /cwd/tests which doesn't match any context
        let cwd = "/cwd".to_string();
        let source_file = "/cwd/tests/components/Button.test.tsx".to_string();
        let resolver = AliasResolver::new(&config, &cwd, &source_file).unwrap();

        // Verify that only aliases with no context are included
        assert_eq!(resolver.aliases.len(), 1);

        // Check if the correct aliases were included
        let patterns: Vec<String> = resolver.aliases.iter().map(|a| a.pattern.clone()).collect();
        assert!(patterns.contains(&no_context_alias.pattern));

        // Check that context-specific aliases are excluded
        assert!(!patterns.contains(&src_context_alias.pattern));
        assert!(!patterns.contains(&other_context_alias.pattern));
    }

    #[test]
    fn test_empty_aliases_list() {
        // Create config with empty aliases list
        let config = Config {
            aliases: Some(vec![]),
            patterns: vec![],
        };

        let cwd = "/cwd".to_string();
        let source_file = "/cwd/src/components/Button.tsx".to_string();
        let resolver = AliasResolver::new(&config, &cwd, &source_file).unwrap();

        // Verify that the aliases list is empty
        assert_eq!(resolver.aliases.len(), 0);

        // Test match_pattern with empty aliases
        let matched = resolver.match_pattern("#features/some");
        assert!(matched.is_none());
    }

    #[test]
    fn test_null_aliases_list() {
        // Create config with null aliases list
        let config = Config {
            aliases: None,
            patterns: vec![],
        };

        let cwd = "/cwd".to_string();
        let source_file = "/cwd/src/components/Button.tsx".to_string();
        let resolver = AliasResolver::new(&config, &cwd, &source_file).unwrap();

        // Verify that the aliases list is empty
        assert_eq!(resolver.aliases.len(), 0);

        // Test match_pattern with empty aliases
        let matched = resolver.match_pattern("#features/some");
        assert!(matched.is_none());
    }

    #[test]
    fn test_no_duplicate_aliases_with_multiple_matching_contexts() {
        // Create an alias with multiple matching contexts
        let alias_with_multiple_contexts = Alias {
            pattern: "#multi-context/*".to_string(),
            paths: vec!["src/multi-context/*/index.ts".to_string()],
            context: Some(vec![
                "/cwd/src".to_string(),
                "/cwd/src/components".to_string(),
                "/cwd/src/features".to_string(),
            ]),
        };

        // Create config with the alias
        let config = Config {
            aliases: Some(vec![alias_with_multiple_contexts.clone()]),
            patterns: vec![],
        };

        // Test with source file that matches multiple contexts
        let cwd = "/cwd".to_string();
        let source_file = "/cwd/src/components/Button.tsx".to_string();
        let resolver = AliasResolver::new(&config, &cwd, &source_file).unwrap();

        // Verify that the alias is added only once
        assert_eq!(resolver.aliases.len(), 1);
        assert_eq!(resolver.aliases[0].pattern, "#multi-context/*");
    }
}
