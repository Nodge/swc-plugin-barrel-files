use std::collections::HashMap;
use std::path::Path;

use crate::paths::{normalize_path, path_join};

/// Virtual filesystem root directory
const SWC_VIRTUAL_FS_ROOT_DIR: &str = "/cwd";

/// Handles path resolution including symlink mappings
#[derive(Clone)]
pub struct PathResolver {
    /// Compilation working directory
    cwd: String,

    /// Map of external paths to internal symlinked paths
    symlinks: HashMap<String, String>,
}

impl PathResolver {
    /// Creates a new PathResolver with the given configuration
    pub fn new(symlinks: &Option<HashMap<String, String>>, cwd: &str) -> Self {
        let symlinks = symlinks
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|(path_from, path_to)| {
                let absolute_path = path_join(cwd, path_from);
                (absolute_path, path_to.clone())
            })
            .collect();

        Self {
            cwd: cwd.into(),
            symlinks,
        }
    }

    /// Resolves a path, applying symlink mappings if applicable
    ///
    /// # Arguments
    ///
    /// * `path` - The path to resolve
    ///
    /// # Returns
    ///
    /// The resolved path, or the original path if no symlink mapping applies
    pub fn resolve_path(&self, path: &str) -> String {
        let absolute_path = path_join(&self.cwd, path);

        // First, try exact file-level symlink matches (highest priority)
        if let Some(symlinked_path) = self.symlinks.get(&absolute_path) {
            return symlinked_path.clone();
        }

        // Then, try directory-level symlink matches
        for (external_path, internal_path) in &self.symlinks {
            if let Some(resolved) =
                self.try_directory_symlink(&absolute_path, external_path, internal_path)
            {
                return resolved;
            }
        }

        // No symlink mapping found, return original path
        path.to_string()
    }

    /// Attempts to resolve a path using directory-level symlinks
    ///
    /// # Arguments
    ///
    /// * `path` - The path to resolve
    /// * `external_dir` - The external directory path in the symlink mapping
    /// * `internal_dir` - The internal directory path in the symlink mapping
    ///
    /// # Returns
    ///
    /// The resolved path if the path is within the external directory, None otherwise
    fn try_directory_symlink(
        &self,
        path: &str,
        external_dir: &str,
        internal_dir: &str,
    ) -> Option<String> {
        // Normalize paths to handle trailing slashes consistently
        let normalized_external = self.normalize_directory_path(external_dir);
        let normalized_path = normalize_path(Path::new(path));

        // Check if the path starts with the external directory
        if normalized_path.starts_with(&normalized_external) {
            // Calculate the relative path within the external directory
            let relative_path = if normalized_path.len() > normalized_external.len() {
                let separator_offset = if normalized_external.ends_with('/') {
                    0
                } else {
                    1
                };
                &normalized_path[normalized_external.len() + separator_offset..]
            } else {
                ""
            };

            // Join the internal directory with the relative path
            if relative_path.is_empty() {
                Some(internal_dir.to_string())
            } else {
                Some(path_join(internal_dir, relative_path))
            }
        } else {
            None
        }
    }

    /// Normalizes a directory path by ensuring consistent trailing slash handling
    ///
    /// # Arguments
    ///
    /// * `dir_path` - The directory path to normalize
    ///
    /// # Returns
    ///
    /// The normalized directory path
    fn normalize_directory_path(&self, dir_path: &str) -> String {
        let normalized = normalize_path(Path::new(dir_path));
        if normalized.ends_with('/') && normalized.len() > 1 {
            normalized[..normalized.len() - 1].to_string()
        } else {
            normalized
        }
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
    pub fn to_virtual_path(&self, path: &str) -> Result<String, String> {
        // TODO: TEST THIS
        if path.starts_with(SWC_VIRTUAL_FS_ROOT_DIR) {
            return Ok(path.to_string());
        }
        // END TODO

        if path.starts_with(&self.cwd) {
            let without_cwd = &path[self.cwd.len() + 1..];
            let result = path_join(SWC_VIRTUAL_FS_ROOT_DIR, without_cwd);
            return Ok(result);
        }

        if Path::new(&path).is_absolute() {
            return Err(format!(
                "E_INVALID_FILE_PATH: Absolute paths not starting with cwd are not supported: {}",
                path
            ));
        }

        let result = path_join(SWC_VIRTUAL_FS_ROOT_DIR, path);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_file_symlink_resolution() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "../external/components/index.ts".to_string(),
            "/cwd/src/ui/index.ts".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        let resolved = resolver.resolve_path("../external/components/index.ts");
        assert_eq!(resolved, "/cwd/src/ui/index.ts");
    }

    #[test]
    fn test_directory_symlink_resolution() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "../external/components".to_string(),
            "/cwd/src/ui".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        let resolved = resolver.resolve_path("../external/components/Button/index.ts");
        assert_eq!(resolved, "/cwd/src/ui/Button/index.ts");
    }

    #[test]
    fn test_absolute_file_with_relative_symlink_resolution() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "../external/components".to_string(),
            "/cwd/components".to_string(),
        );
        symlinks.insert(
            "../external/components/file.ts".to_string(),
            "/cwd/components/custom-file.ts".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        let resolved = resolver.resolve_path("/home/user/external/components/file.ts");
        assert_eq!(resolved, "/cwd/components/custom-file.ts");
    }

    #[test]
    fn test_absolute_directory_with_relative_symlink_resolution() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "../external/components".to_string(),
            "/cwd/components".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        let resolved = resolver.resolve_path("/home/user/external/components/Button/index.ts");
        assert_eq!(resolved, "/cwd/components/Button/index.ts");
    }

    #[test]
    fn test_relative_file_with_absolute_symlink_resolution() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "/home/user/external/components".to_string(),
            "/cwd/components".to_string(),
        );
        symlinks.insert(
            "/home/user/external/components/file.ts".to_string(),
            "/cwd/components/custom-file.ts".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        let resolved = resolver.resolve_path("../external/components/file.ts");
        assert_eq!(resolved, "/cwd/components/custom-file.ts");
    }

    #[test]
    fn test_relative_directory_with_absolute_symlink_resolution() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "/home/user/external/components".to_string(),
            "/cwd/components".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        let resolved = resolver.resolve_path("../external/components/Button/index.ts");
        assert_eq!(resolved, "/cwd/components/Button/index.ts");
    }

    #[test]
    fn test_directory_symlink_with_trailing_slash() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "../external/components/".to_string(),
            "/cwd/src/ui".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        let resolved = resolver.resolve_path("../external/components/Button/index.ts");
        assert_eq!(resolved, "/cwd/src/ui/Button/index.ts");
    }

    #[test]
    fn test_file_symlink_priority_over_directory() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "../external/components".to_string(),
            "/cwd/src/ui".to_string(),
        );
        symlinks.insert(
            "../external/components/Button/index.ts".to_string(),
            "/cwd/src/special/index.ts".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        // Specific file symlink should take priority
        let resolved = resolver.resolve_path("../external/components/Button/index.ts");
        assert_eq!(resolved, "/cwd/src/special/index.ts");

        // Other files should use directory symlink
        let resolved2 = resolver.resolve_path("../external/components/Input/index.ts");
        assert_eq!(resolved2, "/cwd/src/ui/Input/index.ts");
    }

    #[test]
    fn test_no_symlink_match() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "../external/components".to_string(),
            "/cwd/src/ui".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        let resolved = resolver.resolve_path("../other/path/index.ts");
        assert_eq!(resolved, "../other/path/index.ts");
    }

    #[test]
    fn test_empty_symlinks() {
        let resolver = PathResolver::new(&Some(HashMap::new()), "/home/user/project");

        let resolved = resolver.resolve_path("../external/file.ts");
        assert_eq!(resolved, "../external/file.ts");
    }

    #[test]
    fn test_nested_directory_symlinks() {
        let mut symlinks = HashMap::new();
        symlinks.insert(
            "../../shared/workspace/features".to_string(),
            "/cwd/src/features".to_string(),
        );

        let resolver = PathResolver::new(&Some(symlinks), "/home/user/project");

        let resolved = resolver.resolve_path("../../shared/workspace/features/auth/api/index.ts");
        assert_eq!(resolved, "/cwd/src/features/auth/api/index.ts");
    }

    #[test]
    fn test_resolve_to_virtual_path() {
        let cwd = "/home/user/project";
        let resolver = PathResolver::new(&Some(HashMap::new()), cwd);

        // Test with path starting with cwd
        let path = "/home/user/project/src/main.rs";
        assert_eq!(resolver.to_virtual_path(path).unwrap(), "/cwd/src/main.rs");

        // Test with relative path
        let path = "src/main.rs";
        assert_eq!(resolver.to_virtual_path(path).unwrap(), "/cwd/src/main.rs");

        // Test with relative path starting with ./
        let path = "./src/main.rs";
        assert_eq!(resolver.to_virtual_path(path).unwrap(), "/cwd/src/main.rs");

        // Test with nested ./ in the path
        let path = "tests/./fixtures/src/features/f1/index.ts";
        assert_eq!(
            resolver.to_virtual_path(path).unwrap(),
            "/cwd/tests/fixtures/src/features/f1/index.ts"
        );

        // Test with absolute path not starting with cwd
        let path = "/other/path/file.rs";
        assert!(resolver.to_virtual_path(path).is_err());
        assert_eq!(
            resolver.to_virtual_path(path).unwrap_err(),
            "E_INVALID_FILE_PATH: Absolute paths not starting with cwd are not supported: /other/path/file.rs"
        );
    }

    #[test]
    fn test_to_virtual_path_already_virtual() {
        let resolver = PathResolver::new(&Some(HashMap::new()), "/home/user/project");

        // Test with path that already starts with virtual root
        let path = "/cwd/src/components/index.ts";
        assert_eq!(
            resolver.to_virtual_path(path).unwrap(),
            "/cwd/src/components/index.ts"
        );

        // Test with nested virtual path
        let path = "/cwd/nested/deep/file.ts";
        assert_eq!(
            resolver.to_virtual_path(path).unwrap(),
            "/cwd/nested/deep/file.ts"
        );
    }
}
