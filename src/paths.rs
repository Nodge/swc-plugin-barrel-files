//! Path utilities for the barrel files plugin
//!
//! This module provides functionality for path resolution and manipulation.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Cache for file existence checks
static FILE_EXISTS_CACHE: Lazy<Mutex<HashMap<String, bool>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Fast file existence check with caching
///
/// # Arguments
///
/// * `path` - The file path to check
///
/// # Returns
///
/// `true` if the file exists, `false` otherwise
pub fn file_exists(path: &str) -> bool {
    // Check cache first
    if let Ok(cache) = FILE_EXISTS_CACHE.lock() {
        if let Some(&exists) = cache.get(path) {
            return exists;
        }
    }

    let exists = Path::new(path).exists();

    // Cache the result
    if let Ok(mut cache) = FILE_EXISTS_CACHE.lock() {
        cache.insert(path.to_string(), exists);
    }

    exists
}

/// Calculates a relative path from one absolute path to another
///
/// # Arguments
///
/// * `from_path` - The source absolute path
/// * `to_path` - The target absolute path
///
/// # Returns
///
/// The relative path from source to target as an Option<String>
pub fn resolve_relative_path(from_path: &str, to_path: &str) -> Option<String> {
    let full_path = {
        let mut path = PathBuf::from(from_path);
        path.push(to_path);

        path
    };

    let diff = pathdiff::diff_paths(full_path, from_path)?;
    if diff.starts_with("../") {
        return diff.to_str().map(|s| s.to_string());
    }

    let mut relative_diff = PathBuf::from("./");
    relative_diff.push(diff);
    relative_diff.to_str().map(|s| s.to_string())
}

/// Joins two path segments together, handling normalization of path components
///
/// # Arguments
///
/// * `path` - The base path
/// * `path2` - The path to join to the base path
///
/// # Returns
///
/// A normalized joined path string
pub fn path_join(base_path: &str, path: &str) -> String {
    let joined_path = Path::new(base_path).join(path);
    normalize_path(&joined_path)
}

/// Normalizes a path by resolving . and .. components
///
/// # Arguments
///
/// * `path` - The path to normalize
///
/// # Returns
///
/// The normalized path string
pub fn normalize_path(path: &Path) -> String {
    let mut components = Vec::new();

    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                // Remove the last component if it exists and it's a normal component (to handle ..)
                if !components.is_empty()
                    && !matches!(components.last(), Some(std::path::Component::RootDir))
                    && !matches!(components.last(), Some(std::path::Component::ParentDir))
                {
                    components.pop();
                } else if matches!(components.last(), Some(std::path::Component::RootDir)) {
                    // If we're at root and need to go up, replace root with ..
                    components.pop(); // Remove the root
                    components.push(component); // Add the ..
                } else {
                    // If we're already at the root or the path starts with .., keep it
                    components.push(component);
                }
            }
            std::path::Component::CurDir => {
                // Skip . components as they don't change the path
            }
            _ => {
                components.push(component);
            }
        }
    }

    let normalized_path =
        components
            .iter()
            .fold(std::path::PathBuf::new(), |mut path, component| {
                path.push(component.as_os_str());
                path
            });

    normalized_path.to_string_lossy().to_string()
}

/// Gets the directory name of a path
///
/// # Arguments
///
/// * `path` - The path to get the directory name from
///
/// # Returns
///
/// The directory name of the path. Returns an empty string if the path has no parent.
pub fn dirname(path: &str) -> String {
    Path::new(path)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_relative_path() {
        // Common directory one level up
        assert_eq!(
            resolve_relative_path("/a/b", "/a/c"),
            Some("../c".to_string())
        );
        // Common directory two levels up
        assert_eq!(
            resolve_relative_path("/a/b/c", "/a/d/e"),
            Some("../../d/e".to_string())
        );
        // No common directory
        assert_eq!(
            resolve_relative_path("/a/b/c", "/d/e/f"),
            Some("../../../d/e/f".to_string())
        );
        // Subdirectory of base directory
        assert_eq!(
            resolve_relative_path("/a/b", "/a/b/c"),
            Some("./c".to_string())
        );
        // Same directory
        assert_eq!(
            resolve_relative_path("/a/b", "/a/b"),
            Some("./".to_string())
        );
    }

    #[test]
    fn test_dirname() {
        // Test with normal path
        assert_eq!(dirname("/path/to/file.txt"), "/path/to");

        // Test with path ending with directory separator
        assert_eq!(dirname("/path/to/dir/"), "/path/to");

        // Test with root path
        assert_eq!(dirname("/"), "");

        // Test with relative path
        assert_eq!(dirname("path/to/file.txt"), "path/to");

        // Test with single file (no directory)
        assert_eq!(dirname("file.txt"), "");
    }

    #[test]
    fn test_normalize_path() {
        use std::path::Path;

        // Basic normalization
        assert_eq!(normalize_path(Path::new("a/b/c")), "a/b/c");
        assert_eq!(normalize_path(Path::new("a/./b")), "a/b");
        assert_eq!(normalize_path(Path::new("a/b/../c")), "a/c");
        assert_eq!(normalize_path(Path::new("a/../b")), "b");

        // Multiple components
        assert_eq!(normalize_path(Path::new("a/b/../../c")), "c");
        assert_eq!(normalize_path(Path::new("./a/b")), "a/b");

        // Absolute paths
        assert_eq!(normalize_path(Path::new("/a/b/../c")), "/a/c");
        assert_eq!(normalize_path(Path::new("/a/./b")), "/a/b");

        // Edge cases
        assert_eq!(normalize_path(Path::new(".")), "");
        assert_eq!(normalize_path(Path::new("a/..")), "");

        // Going beyond root directory
        assert_eq!(normalize_path(Path::new("/a/../..")), "..");
        assert_eq!(normalize_path(Path::new("/a/../../b")), "../b");
        assert_eq!(normalize_path(Path::new("/a/b/../../../c")), "../c");

        // Multiple parent directories at start
        assert_eq!(normalize_path(Path::new("../a/b")), "../a/b");
        assert_eq!(normalize_path(Path::new("../../a")), "../../a");
        assert_eq!(
            normalize_path(Path::new("../../../a/b/c")),
            "../../../a/b/c"
        );

        // Mixed cases with parent directories
        assert_eq!(normalize_path(Path::new("../a/../b")), "../b");
        assert_eq!(normalize_path(Path::new("../../a/b/../c")), "../../a/c");

        // Complex cases with current directory
        assert_eq!(normalize_path(Path::new("./a/../b")), "b");
        assert_eq!(normalize_path(Path::new("a/./b/../c")), "a/c");

        // Empty path components
        assert_eq!(normalize_path(Path::new("a//b")), "a/b");
        assert_eq!(normalize_path(Path::new("a/./b//c")), "a/b/c");

        // Root directory edge cases
        assert_eq!(normalize_path(Path::new("/")), "/");
        assert_eq!(normalize_path(Path::new("/..")), "..");
        assert_eq!(normalize_path(Path::new("/.")), "/");
    }

    #[test]
    fn test_path_join() {
        // Basic path joining
        assert_eq!(path_join("a", "b"), "a/b");
        assert_eq!(path_join("a/", "b"), "a/b");

        // Absolute path in second arg replaces first
        assert_eq!(path_join("a", "/b"), "/b");

        // Handling of . and .. components
        assert_eq!(path_join("a/b", "../c"), "a/c");
        assert_eq!(path_join("a/b", "./c"), "a/b/c");
        assert_eq!(path_join("a/b", "../../c"), "c");
        assert_eq!(path_join("/cwd/a", "../../../c"), "../c");

        // Handling of multiple slashes and normalization
        assert_eq!(path_join("a//b", "c"), "a/b/c");
        assert_eq!(path_join("a/./b", "c"), "a/b/c");

        // Complex cases
        assert_eq!(path_join("/a/b/c", "../d/./e"), "/a/b/d/e");
        assert_eq!(path_join("", "a/b"), "a/b");
        assert_eq!(path_join("a/b", ""), "a/b");
    }
}
