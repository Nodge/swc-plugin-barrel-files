//! Pattern matcher module for the barrel files plugin
//!
//! This module provides functionality for matching import paths against patterns with wildcards,
//! extracting components from matched paths, and applying those components to path templates.
//! It implements the pattern matching logic described in the implementation plan.

use regex::Regex;
use std::collections::HashMap;

/// Represents a pattern match result
#[derive(Debug, Clone, PartialEq)]
pub struct PatternMatch {
    /// The pattern that matched
    pub pattern: String,

    /// The captured components from the pattern
    pub components: HashMap<String, String>,

    /// The number of wildcards in the pattern (used for specificity calculation)
    pub wildcard_count: usize,
}

/// Matches an import path against a pattern with wildcards
///
/// # Arguments
///
/// * `path` - The import path to match
/// * `pattern` - The pattern to match against, which may contain wildcards (`*`)
///
/// # Returns
///
/// `true` if the pattern matches the path, `false` otherwise
pub fn path_matches_pattern(path: &str, pattern: &str) -> bool {
    // Convert pattern to regex
    let regex_pattern = pattern
        .replace("*", "([^/]+)")
        .replace(".", "\\.")
        .replace("/", "\\/");

    let regex = Regex::new(&format!("^{}$", regex_pattern)).unwrap_or_else(|e| {
        panic!("Invalid regex pattern generated from '{}': {}", pattern, e);
    });

    regex.is_match(path)
}

/// Counts the number of wildcards in a pattern
///
/// # Arguments
///
/// * `pattern` - The pattern to count wildcards in
///
/// # Returns
///
/// The number of wildcards (`*`) in the pattern
fn count_wildcards(pattern: &str) -> usize {
    pattern.matches('*').count()
}

/// Extracts components from an import path based on a pattern with wildcards
///
/// # Arguments
///
/// * `path` - The import path to extract components from
/// * `pattern` - The pattern with wildcards to match against
///
/// # Returns
///
/// A HashMap containing the extracted components, where the keys are the wildcard
/// positions (p0, p1, etc.) and the values are the matched strings
pub fn extract_pattern_components(path: &str, pattern: &str) -> HashMap<String, String> {
    let mut components = HashMap::new();

    // Convert pattern to regex with named capture groups
    let mut regex_pattern = String::new();
    let mut i = 0;

    for part in pattern.split('*') {
        regex_pattern.push_str(&regex::escape(part));

        if i < pattern.split('*').count() - 1 {
            regex_pattern.push_str(&format!("(?P<p{}>([^/]+))", i));
            i += 1;
        }
    }

    let regex = Regex::new(&format!("^{}$", regex_pattern)).unwrap_or_else(|e| {
        panic!("Invalid regex pattern generated from '{}': {}", pattern, e);
    });

    if let Some(captures) = regex.captures(path) {
        for i in 0..i {
            let name = format!("p{}", i);
            if let Some(capture) = captures.name(&name) {
                components.insert(name, capture.as_str().to_string());
            }
        }
    }

    components
}

/// Applies extracted components to a path template
///
/// # Arguments
///
/// * `template` - The path template with wildcards (`*`)
/// * `components` - The components to apply to the template
///
/// # Returns
///
/// The template with wildcards replaced by the corresponding components
pub fn apply_components_to_template(
    template: &str,
    components: &HashMap<String, String>,
) -> String {
    let mut result = template.to_string();
    let mut values: Vec<_> = components.iter().collect();

    // Sort by key to ensure consistent ordering (p0, p1, p2, etc.)
    values.sort_by(|a, b| a.0.cmp(b.0));

    for (_, value) in values {
        result = result.replacen("*", value, 1);
    }

    result
}

/// Finds the best matching pattern for an import path from a list of patterns
///
/// # Arguments
///
/// * `path` - The import path to match
/// * `patterns` - A list of patterns to match against
///
/// # Returns
///
/// The best matching pattern, or None if no pattern matches
///
/// The best match is determined by:
/// 1. More specific patterns (with fewer wildcards) take precedence
/// 2. If patterns have the same number of wildcards, the one that appears earlier in the list takes precedence
pub fn find_best_matching_pattern(path: &str, patterns: &[String]) -> Option<PatternMatch> {
    let mut matches: Vec<PatternMatch> = Vec::new();

    // Find all matching patterns
    for pattern in patterns {
        if path_matches_pattern(path, pattern) {
            let components = extract_pattern_components(path, pattern);
            let wildcard_count = count_wildcards(pattern);

            matches.push(PatternMatch {
                pattern: pattern.clone(),
                components,
                wildcard_count,
            });
        }
    }

    if matches.is_empty() {
        return None;
    }

    // Sort matches by specificity (fewer wildcards = more specific)
    matches.sort_by(|a, b| {
        // First by wildcard count (ascending)
        let count_cmp = a.wildcard_count.cmp(&b.wildcard_count);

        if count_cmp == std::cmp::Ordering::Equal {
            // Then by position in the patterns array (earlier = higher priority)
            let a_pos = patterns
                .iter()
                .position(|p| p == &a.pattern)
                .unwrap_or(usize::MAX);
            let b_pos = patterns
                .iter()
                .position(|p| p == &b.pattern)
                .unwrap_or(usize::MAX);
            a_pos.cmp(&b_pos)
        } else {
            count_cmp
        }
    });

    // Return the best match (most specific or earliest in the list)
    matches.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_matches_pattern() {
        // Basic pattern matching
        assert!(path_matches_pattern("#entities/user", "#entities/*"));
        assert!(path_matches_pattern(
            "#entities/user/testing",
            "#entities/*/testing"
        ));
        assert!(!path_matches_pattern("#entities/user/model", "#entities/*"));
        assert!(!path_matches_pattern("#entities/", "#entities/*"));
        assert!(path_matches_pattern(
            "@direct-frontend/stdlib",
            "@direct-frontend/stdlib"
        ));
        assert!(!path_matches_pattern(
            "@direct-frontend/stdlib/testing",
            "@direct-frontend/stdlib"
        ));

        // Multiple wildcards
        assert!(path_matches_pattern(
            "#features/auth/components/login",
            "#features/*/components/*"
        ));
        assert!(!path_matches_pattern(
            "#features/auth/pages/login",
            "#features/*/components/*"
        ));

        // Wildcard in the middle
        assert!(path_matches_pattern(
            "#entities/user/testing",
            "#entities/*/testing"
        ));
        assert!(!path_matches_pattern(
            "#entities/user/model/testing",
            "#entities/*/testing"
        ));

        // Special characters in patterns
        assert!(path_matches_pattern(
            "@direct-frontend/components/Button",
            "@direct-frontend/components/*"
        ));
        assert!(path_matches_pattern(
            "@direct-frontend/components.ui/Button",
            "@direct-frontend/components.ui/*"
        ));
        assert!(!path_matches_pattern(
            "@direct-frontend/components|ui/Button",
            "@direct-frontend/components.ui/*"
        ));
    }

    #[test]
    fn test_count_wildcards() {
        assert_eq!(count_wildcards("#entities/*"), 1);
        assert_eq!(count_wildcards("#entities/*/testing"), 1);
        assert_eq!(count_wildcards("#features/*/components/*"), 2);
        assert_eq!(count_wildcards("@direct-frontend/stdlib"), 0);
    }

    #[test]
    fn test_extract_pattern_components() {
        // Single wildcard
        let components = extract_pattern_components("#entities/user", "#entities/*");
        assert_eq!(components.get("p0"), Some(&"user".to_string()));

        // Wildcard in the middle
        let components =
            extract_pattern_components("#entities/user/testing", "#entities/*/testing");
        assert_eq!(components.get("p0"), Some(&"user".to_string()));

        // Multiple wildcards
        let components = extract_pattern_components(
            "#features/auth/components/login",
            "#features/*/components/*",
        );
        assert_eq!(components.get("p0"), Some(&"auth".to_string()));
        assert_eq!(components.get("p1"), Some(&"login".to_string()));

        // No wildcards
        let components =
            extract_pattern_components("@direct-frontend/stdlib", "@direct-frontend/stdlib");
        assert!(components.is_empty());

        // Special characters
        let components = extract_pattern_components(
            "@direct-frontend/components.ui/Button",
            "@direct-frontend/components.ui/*",
        );
        assert_eq!(components.get("p0"), Some(&"Button".to_string()));
    }

    #[test]
    fn test_apply_components_to_template() {
        // Single wildcard
        let mut components = HashMap::new();
        components.insert("p0".to_string(), "user".to_string());

        let result = apply_components_to_template("./src/entities/*/index.ts", &components);
        assert_eq!(result, "./src/entities/user/index.ts");

        // Multiple wildcards
        let mut components = HashMap::new();
        components.insert("p0".to_string(), "auth".to_string());
        components.insert("p1".to_string(), "login".to_string());

        let result =
            apply_components_to_template("./src/features/*/components/*/index.ts", &components);
        assert_eq!(result, "./src/features/auth/components/login/index.ts");

        // Ensure correct ordering of replacements
        let mut components = HashMap::new();
        components.insert("p1".to_string(), "second".to_string());
        components.insert("p0".to_string(), "first".to_string());

        let result = apply_components_to_template("*/*/template", &components);
        assert_eq!(result, "first/second/template");
    }

    #[test]
    fn test_find_best_matching_pattern() {
        // Test with specific pattern taking precedence
        let patterns = vec!["#entities/user".to_string(), "#entities/*".to_string()];

        let best_match = find_best_matching_pattern("#entities/user", &patterns);
        assert!(best_match.is_some());
        let match_result = best_match.unwrap();
        assert_eq!(match_result.pattern, "#entities/user");
        assert_eq!(match_result.wildcard_count, 0);

        // Test with wildcard pattern
        let best_match = find_best_matching_pattern("#entities/product", &patterns);
        assert!(best_match.is_some());
        let match_result = best_match.unwrap();
        assert_eq!(match_result.pattern, "#entities/*");
        assert_eq!(match_result.wildcard_count, 1);

        // Test with no matching pattern
        let best_match = find_best_matching_pattern("#features/auth", &patterns);
        assert!(best_match.is_none());

        // Test with multiple patterns having the same wildcard count
        let patterns = vec![
            "#features/*/components".to_string(),
            "#features/*/pages".to_string(),
        ];

        let best_match = find_best_matching_pattern("#features/auth/components", &patterns);
        assert!(best_match.is_some());
        let match_result = best_match.unwrap();
        assert_eq!(match_result.pattern, "#features/*/components");

        // Test with patterns having different wildcard counts
        let patterns = vec![
            "#features/*/components/*".to_string(),
            "#features/auth/components/*".to_string(),
        ];

        let best_match = find_best_matching_pattern("#features/auth/components/button", &patterns);
        assert!(best_match.is_some());
        let match_result = best_match.unwrap();
        assert_eq!(match_result.pattern, "#features/auth/components/*");
        assert_eq!(match_result.wildcard_count, 1);
    }
}
