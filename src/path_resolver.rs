use std::path::Path;

use crate::paths::{normalize_path, path_join};

/// Virtual filesystem root directory
const SWC_VIRTUAL_FS_ROOT_DIR: &str = "/cwd";

/// Maps host paths to the plugin's virtual filesystem
#[derive(Clone)]
pub struct PathResolver {
    /// Compilation working directory
    cwd: String,
}

impl PathResolver {
    /// Creates a new PathResolver for the given working directory
    pub fn new(cwd: &str) -> Self {
        Self { cwd: cwd.into() }
    }

    /// Resolves a path to a virtual path
    ///
    /// # Arguments
    ///
    /// * `path` - The path to resolve
    ///
    /// # Returns
    ///
    /// The resolved virtual path
    pub fn to_virtual_path(&self, path: &str) -> Result<String, String> {
        let virtual_path = if Path::new(path).starts_with(SWC_VIRTUAL_FS_ROOT_DIR) {
            normalize_path(Path::new(path))
        } else if let Ok(without_cwd) = Path::new(path).strip_prefix(&self.cwd) {
            path_join(SWC_VIRTUAL_FS_ROOT_DIR, &without_cwd.to_string_lossy())
        } else if Path::new(path).is_absolute() {
            return Err(format!(
                "E_INVALID_FILE_PATH: Absolute paths not starting with cwd are not supported: {}",
                path
            ));
        } else {
            path_join(SWC_VIRTUAL_FS_ROOT_DIR, path)
        };

        // `..` segments can still climb out of the virtual root
        if !Path::new(&virtual_path).starts_with(SWC_VIRTUAL_FS_ROOT_DIR) {
            return Err(format!(
                "E_INVALID_FILE_PATH: Paths outside cwd are not supported: {}",
                path
            ));
        }

        Ok(virtual_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_to_virtual_path() {
        let cwd = "/home/user/project";
        let resolver = PathResolver::new(cwd);

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
    fn test_to_virtual_path_compares_whole_components() {
        let resolver = PathResolver::new("/home/user/project");

        assert_eq!(
            resolver.to_virtual_path("/home/user/project").unwrap(),
            "/cwd"
        );
        assert!(resolver
            .to_virtual_path("/home/user/project-lib/index.ts")
            .is_err());
        assert!(resolver.to_virtual_path("/cwdata/index.ts").is_err());
    }

    #[test]
    fn test_to_virtual_path_rejects_escaping_paths() {
        let resolver = PathResolver::new("/home/user/project");

        assert!(resolver.to_virtual_path("../shared/index.ts").is_err());
        assert!(resolver
            .to_virtual_path("/home/user/project/../shared/index.ts")
            .is_err());
        assert!(resolver.to_virtual_path("/cwd/../index.ts").is_err());
        assert_eq!(
            resolver.to_virtual_path("/cwd/src/../index.ts").unwrap(),
            "/cwd/index.ts"
        );
    }

    #[test]
    fn test_to_virtual_path_already_virtual() {
        let resolver = PathResolver::new("/home/user/project");

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
