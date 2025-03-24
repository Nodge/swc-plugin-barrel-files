//! Pattern matcher module for the barrel files plugin
//!
//! This module provides functionality for matching import paths against patterns with wildcards,
//! extracting components from matched paths, and applying those components to path templates.
//! It implements the pattern matching logic described in the implementation plan.

use regex::Regex;
use std::collections::HashMap;

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
pub fn count_wildcards(pattern: &str) -> usize {
    pattern.matches('*').count()
}

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
}
