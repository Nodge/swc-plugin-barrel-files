//! Pattern matcher module for the barrel files plugin
//!
//! This module provides functionality for matching import paths against patterns with wildcards,
//! extracting components from matched paths, and applying those components to path templates.

/// Pre-compiled pattern for optimized matching
#[derive(Clone)]
pub struct CompiledPattern {
    /// Pattern parts separated by wildcards
    pub parts: Vec<String>,
    /// Number of wildcards in the pattern
    pub wildcard_count: usize,
}

impl CompiledPattern {
    /// Creates a new compiled pattern
    pub fn new(pattern: &str) -> Result<Self, String> {
        let parts: Vec<String> = pattern.split('*').map(|s| s.to_string()).collect();
        let wildcard_count = parts.len().saturating_sub(1);

        Ok(CompiledPattern {
            parts,
            wildcard_count,
        })
    }

    /// Checks if a path matches this pattern
    pub fn matches(&self, path: &str) -> bool {
        if self.parts.is_empty() {
            return path.is_empty();
        }

        if self.wildcard_count == 0 {
            return path == self.parts[0];
        }

        // Each wildcard (*) matches [^/]+ (one or more characters except /)
        let mut path_pos = 0;
        let path_len = path.len();

        for (i, part) in self.parts.iter().enumerate() {
            if i == 0 {
                // First part - must match at the beginning
                if !part.is_empty() {
                    if !path.starts_with(part.as_str()) {
                        return false;
                    }
                    path_pos += part.len();
                }
            } else if i == self.parts.len() - 1 {
                // Last part - must match at the end
                if !part.is_empty() {
                    if !path.ends_with(part.as_str()) {
                        return false;
                    }
                    // Make sure there's a valid wildcard match before this part
                    let wildcard_start = path_pos;
                    let wildcard_end = path_len - part.len();
                    if wildcard_start >= wildcard_end {
                        return false;
                    }
                    // Check that the wildcard doesn't contain '/'
                    let wildcard_value = &path[wildcard_start..wildcard_end];
                    if wildcard_value.contains('/') {
                        return false;
                    }
                } else {
                    // Pattern ends with wildcard, check remaining path doesn't contain '/'
                    if path_pos >= path_len {
                        return false;
                    }
                    let wildcard_value = &path[path_pos..];
                    if wildcard_value.contains('/') {
                        return false;
                    }
                }
            } else {
                // Middle parts - find the next occurrence, but ensure wildcard is valid
                if !part.is_empty() {
                    if let Some(pos) = path[path_pos..].find(part) {
                        // Check that the wildcard before this part doesn't contain '/'
                        let wildcard_value = &path[path_pos..path_pos + pos];
                        if wildcard_value.contains('/') || wildcard_value.is_empty() {
                            return false;
                        }
                        path_pos += pos + part.len();
                    } else {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Extracts components from a path using this pattern
    pub fn extract_components(&self, path: &str) -> Vec<String> {
        let mut components = Vec::new();

        if !self.matches(path) {
            return components;
        }

        if self.wildcard_count == 0 {
            return components;
        }

        let mut path_pos = 0;
        let path_len = path.len();
        for (i, part) in self.parts.iter().enumerate() {
            if i == 0 {
                // Skip the first literal part
                if !part.is_empty() {
                    path_pos += part.len();
                }
            } else if i == self.parts.len() - 1 {
                // Extract the last wildcard before the final literal part
                if !part.is_empty() {
                    let end_pos = path_len - part.len();
                    if path_pos < end_pos {
                        let wildcard_value = &path[path_pos..end_pos];
                        components.push(wildcard_value.to_string());
                    }
                } else {
                    // Pattern ends with wildcard
                    if path_pos < path_len {
                        let wildcard_value = &path[path_pos..];
                        components.push(wildcard_value.to_string());
                    }
                }
                break;
            } else {
                // Extract wildcard between parts
                if !part.is_empty() {
                    if let Some(next_pos) = path[path_pos..].find(part) {
                        let wildcard_value = &path[path_pos..path_pos + next_pos];
                        components.push(wildcard_value.to_string());
                        path_pos += next_pos + part.len();
                    } else {
                        break;
                    }
                }
            }
        }

        components
    }
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
pub fn apply_components_to_template(template: &str, components: &[String]) -> String {
    components
        .iter()
        .fold(template.to_string(), |result, value| {
            result.replacen('*', value, 1)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiled_pattern_direct() {
        // Test CompiledPattern directly
        let pattern = CompiledPattern::new("#entities/*").unwrap();
        assert_eq!(pattern.wildcard_count, 1);
        assert_eq!(pattern.parts, vec!["#entities/", ""]);
        assert!(pattern.matches("#entities/user"));
        assert!(!pattern.matches("#entities/user/model"));

        let pattern2 = CompiledPattern::new("#features/*/components/*").unwrap();
        assert_eq!(pattern2.wildcard_count, 2);
        assert_eq!(pattern2.parts, vec!["#features/", "/components/", ""]);
        assert!(pattern2.matches("#features/auth/components/login"));
        assert!(!pattern2.matches("#features/auth/pages/login"));

        // Test component extraction
        let components = pattern2.extract_components("#features/auth/components/login");
        assert_eq!(components.first(), Some(&"auth".to_string()));
        assert_eq!(components.get(1), Some(&"login".to_string()));
    }

    #[test]
    fn test_non_ascii_paths() {
        let pattern = CompiledPattern::new("/cwd/src/*ab").unwrap();
        assert!(!pattern.matches("/cwd/src/xéb"));
        assert!(pattern.matches("/cwd/src/éab"));

        let pattern = CompiledPattern::new("/cwd/é*/index.ts").unwrap();
        assert!(!pattern.matches("/cwd/e"));
        assert!(pattern.matches("/cwd/éx/index.ts"));
    }

    #[test]
    fn test_pattern_matching() {
        // Basic pattern matching
        let pattern1 = CompiledPattern::new("#entities/*").unwrap();
        assert!(pattern1.matches("#entities/user"));
        assert!(!pattern1.matches("#entities/user/model"));
        assert!(!pattern1.matches("#entities/"));

        let pattern2 = CompiledPattern::new("#entities/*/testing").unwrap();
        assert!(pattern2.matches("#entities/user/testing"));
        assert!(!pattern2.matches("#entities/user/model/testing"));

        let pattern3 = CompiledPattern::new("@direct-frontend/stdlib").unwrap();
        assert!(pattern3.matches("@direct-frontend/stdlib"));
        assert!(!pattern3.matches("@direct-frontend/stdlib/testing"));

        // Multiple wildcards
        let pattern4 = CompiledPattern::new("#features/*/components/*").unwrap();
        assert!(pattern4.matches("#features/auth/components/login"));
        assert!(!pattern4.matches("#features/auth/pages/login"));

        // Special characters in patterns
        let pattern5 = CompiledPattern::new("@direct-frontend/components/*").unwrap();
        assert!(pattern5.matches("@direct-frontend/components/Button"));

        let pattern6 = CompiledPattern::new("@direct-frontend/components.ui/*").unwrap();
        assert!(pattern6.matches("@direct-frontend/components.ui/Button"));
        assert!(!pattern6.matches("@direct-frontend/components|ui/Button"));
    }

    #[test]
    fn test_wildcard_counting() {
        let pattern1 = CompiledPattern::new("#entities/*").unwrap();
        assert_eq!(pattern1.wildcard_count, 1);

        let pattern2 = CompiledPattern::new("#entities/*/testing").unwrap();
        assert_eq!(pattern2.wildcard_count, 1);

        let pattern3 = CompiledPattern::new("#features/*/components/*").unwrap();
        assert_eq!(pattern3.wildcard_count, 2);

        let pattern4 = CompiledPattern::new("@direct-frontend/stdlib").unwrap();
        assert_eq!(pattern4.wildcard_count, 0);
    }

    #[test]
    fn test_component_extraction() {
        // Single wildcard
        let pattern1 = CompiledPattern::new("#entities/*").unwrap();
        let components = pattern1.extract_components("#entities/user");
        assert_eq!(components.first(), Some(&"user".to_string()));

        // Wildcard in the middle
        let pattern2 = CompiledPattern::new("#entities/*/testing").unwrap();
        let components = pattern2.extract_components("#entities/user/testing");
        assert_eq!(components.first(), Some(&"user".to_string()));

        // Multiple wildcards
        let pattern3 = CompiledPattern::new("#features/*/components/*").unwrap();
        let components = pattern3.extract_components("#features/auth/components/login");
        assert_eq!(components.first(), Some(&"auth".to_string()));
        assert_eq!(components.get(1), Some(&"login".to_string()));

        // No wildcards
        let pattern4 = CompiledPattern::new("@direct-frontend/stdlib").unwrap();
        let components = pattern4.extract_components("@direct-frontend/stdlib");
        assert!(components.is_empty());

        // Special characters
        let pattern5 = CompiledPattern::new("@direct-frontend/components.ui/*").unwrap();
        let components = pattern5.extract_components("@direct-frontend/components.ui/Button");
        assert_eq!(components.first(), Some(&"Button".to_string()));
    }

    #[test]
    fn test_apply_components_to_template() {
        // Single wildcard
        let components = vec!["user".to_string()];
        let result = apply_components_to_template("./src/entities/*/index.ts", &components);
        assert_eq!(result, "./src/entities/user/index.ts");

        // Multiple wildcards are filled in capture order
        let components = vec!["auth".to_string(), "login".to_string()];
        let result =
            apply_components_to_template("./src/features/*/components/*/index.ts", &components);
        assert_eq!(result, "./src/features/auth/components/login/index.ts");

        // More than ten wildcards keep their order
        let components: Vec<String> = (0..12).map(|i| i.to_string()).collect();
        let result = apply_components_to_template(&"*/".repeat(12), &components);
        assert_eq!(result, "0/1/2/3/4/5/6/7/8/9/10/11/");
    }
}
