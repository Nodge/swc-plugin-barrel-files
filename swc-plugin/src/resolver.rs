//! Resolver module for the barrel files plugin
//!
//! This module provides functionality for resolving barrel files.

use std::path::{Component, Path, PathBuf};

use crate::pattern_matcher::{apply_components_to_template, extract_pattern_components};

/// Virtual filesystem root directory
const SWC_VIRTUAL_FS_ROOT_DIR: &str = "/cwd";

/// Resolves a barrel file based on the import path and rule
///
/// # Arguments
///
/// * `cwd` - Compilation working directory
/// * `import_path` - The import path to resolve
/// * `pattern` - The pattern to match against
/// * `paths` - The possible paths to resolve to
///
/// # Returns
///
/// The resolved path if found, `None` otherwise
pub fn resolve_barrel_file(
    cwd: &str,
    import_path: &str,
    pattern: &str,
    paths: &[String],
) -> Result<Option<String>, String> {
    let components = extract_pattern_components(import_path, pattern);

    for path_template in paths {
        let resolved_path = apply_components_to_template(path_template, &components);
        let path = match resolve_to_virtual_path(cwd, &resolved_path) {
            Ok(path) => path,
            Err(err) => return Err(err),
        };

        if Path::new(&path).exists() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

/// Resolves a path to a virtual path
///
/// # Arguments
///
/// * `cwd` - Compilation working directory
/// * `path` - The path to resolve
///
/// # Returns
///
/// The resolved virtual path
pub fn resolve_to_virtual_path(cwd: &str, path: &str) -> Result<String, String> {
    if path.starts_with(cwd) {
        let without_cwd = &path[cwd.len() + 1..];
        let new_path = Path::new(SWC_VIRTUAL_FS_ROOT_DIR).join(without_cwd);
        let result = new_path.to_string_lossy().to_string();
        return Ok(result);
    }

    if Path::new(&path).is_absolute() {
        return Err(format!(
            "Absolute paths not starting with cwd are not supported: {}",
            path
        ));
    }

    let new_path = Path::new(SWC_VIRTUAL_FS_ROOT_DIR).join(path);
    let result = new_path.to_string_lossy().to_string();
    return Ok(result);
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
/// The relative path from source to target
pub fn resolve_relative_path(from_path: &str, to_path: &str) -> Option<String> {
    let path = Path::new(to_path);
    let base = Path::new(from_path);

    if path.is_absolute() != base.is_absolute() {
        if path.is_absolute() {
            Some(PathBuf::from(path).to_string_lossy().to_string())
        } else {
            None
        }
    } else {
        let mut ita = path.components();
        let mut itb = base.components();
        let mut comps: Vec<Component> = vec![];

        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
                (None, _) => comps.push(Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(b)) if b == Component::CurDir => comps.push(a),
                (Some(_), Some(b)) if b == Component::ParentDir => return None,
                (Some(a), Some(_)) => {
                    comps.push(Component::ParentDir);
                    for _ in itb {
                        comps.push(Component::ParentDir);
                    }
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
            }
        }

        if comps.is_empty() || comps[0] != Component::ParentDir {
            comps.insert(0, Component::CurDir);
        }

        let path_buf: PathBuf = comps.iter().collect();
        Some(path_buf.to_string_lossy().to_string())
    }
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
pub fn path_join(path: &str, path2: &str) -> String {
    let joined_path = Path::new(path).join(path2);

    let mut components = Vec::new();

    for component in joined_path.components() {
        match component {
            std::path::Component::ParentDir => {
                // Remove the last component if it exists (to handle ..)
                if !components.is_empty()
                    && !matches!(components.last(), Some(std::path::Component::RootDir))
                {
                    components.pop();
                } else {
                    // If we're already at the root or the path starts with .., keep it
                    components.push(component);
                }
            }
            std::path::Component::CurDir => {
                // Skip . components as they don't change the path
            }
            _ => {
                // Add normal components
                components.push(component);
            }
        }
    }

    // Reconstruct the path from normalized components
    let normalized_path =
        components
            .iter()
            .fold(std::path::PathBuf::new(), |mut path, component| {
                path.push(component.as_os_str());
                path
            });

    // Convert to string
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
        assert_eq!(resolve_relative_path("/a/b", "/a/b"), Some(".".to_string()));
    }

    #[test]
    fn test_resolve_to_virtual_path() {
        // Test with path starting with cwd
        let cwd = "/home/user/project";
        let path = "/home/user/project/src/main.rs";
        assert_eq!(
            resolve_to_virtual_path(cwd, path).unwrap(),
            "/cwd/src/main.rs"
        );

        // Test with relative path
        let path = "src/main.rs";
        assert_eq!(
            resolve_to_virtual_path(cwd, path).unwrap(),
            "/cwd/src/main.rs"
        );

        // Test with absolute path not starting with cwd
        let path = "/other/path/file.rs";
        assert!(resolve_to_virtual_path(cwd, path).is_err());
        assert_eq!(
            resolve_to_virtual_path(cwd, path).unwrap_err(),
            "Absolute paths not starting with cwd are not supported: /other/path/file.rs"
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

        // Handling of multiple slashes and normalization
        assert_eq!(path_join("a//b", "c"), "a/b/c");
        assert_eq!(path_join("a/./b", "c"), "a/b/c");

        // Complex cases
        assert_eq!(path_join("/a/b/c", "../d/./e"), "/a/b/d/e");
        assert_eq!(path_join("", "a/b"), "a/b");
        assert_eq!(path_join("a/b", ""), "a/b");
    }
}
