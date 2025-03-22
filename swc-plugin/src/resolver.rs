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

    println!("import_path: {}", import_path);

    for path_template in paths {
        println!("Try path template: {}", path_template);

        let resolved_path = apply_components_to_template(path_template, &components);
        let path = match resolve_to_virtual_path(cwd, &resolved_path) {
            Ok(path) => path,
            Err(err) => return Err(err),
        };

        println!("Try path: {}", path);

        if Path::new(&path).exists() {
            println!("Resolved path: {}", path);
            return Ok(Some(path));
        }
    }

    println!("Not found");

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
        // Collect into a PathBuf and then convert to String
        let path_buf: PathBuf = comps.iter().collect();
        Some(path_buf.to_string_lossy().to_string())
    }
}

pub fn dirname(path: &str) -> String {
    Path::new(path)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_string_lossy()
        .to_string()
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_relative_path() {
        // Test the example provided by the user
        assert_eq!(
            resolve_relative_path("/a/b", "/a/c"),
            Some("../c".to_string())
        );

        // Additional test cases
        assert_eq!(
            resolve_relative_path("/a/b/c", "/a/d/e"),
            Some("../../d/e".to_string())
        );
        assert_eq!(
            resolve_relative_path("/a/b", "/a/b/c"),
            Some("c".to_string())
        );
        assert_eq!(resolve_relative_path("/a/b", "/a/b"), Some("".to_string()));
        assert_eq!(
            resolve_relative_path("/a/b/c", "/d/e/f"),
            Some("../../../d/e/f".to_string())
        );
    }

    #[test]
    fn test_path_join() {
        // Basic path joining
        assert_eq!(path_join("a", "b"), "a/b");
        assert_eq!(path_join("a/", "b"), "a/b");
        assert_eq!(path_join("a", "/b"), "/b"); // Absolute path in second arg replaces first

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
