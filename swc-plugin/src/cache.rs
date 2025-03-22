//! Cache module for the barrel files plugin
//!
//! This module provides caching functionality to avoid repeatedly reading
//! and parsing the same files.

use std::collections::HashMap;
use std::fs;
use std::time::{Duration, SystemTime};

use swc_core::ecma::ast::Module;

/// A cache for file system operations to avoid repeatedly reading and parsing the same files
#[derive(Debug)]
pub struct FileCache {
    /// Map of file paths to their parsed AST and last modified time
    cache: HashMap<String, (Module, SystemTime)>,
    /// Cache duration in milliseconds
    cache_duration_ms: u64,
}

impl FileCache {
    /// Creates a new file cache with the specified cache duration
    ///
    /// # Arguments
    ///
    /// * `cache_duration_ms` - The cache duration in milliseconds
    ///
    /// # Returns
    ///
    /// A new `FileCache` instance
    pub fn new(cache_duration_ms: u64) -> Self {
        FileCache {
            cache: HashMap::new(),
            cache_duration_ms,
        }
    }

    /// Gets a cached AST or returns None if not in cache or if modified
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path of the file to get from cache
    ///
    /// # Returns
    ///
    /// The cached AST if available and not modified, None otherwise
    pub fn get(&self, file_path: &str) -> Option<Module> {
        // Check if the file is in the cache
        if let Some((ast, last_modified)) = self.cache.get(file_path) {
            let current_modified = match fs::metadata(file_path) {
                Ok(metadata) => match metadata.modified() {
                    Ok(time) => time,
                    Err(_) => return None,
                },
                Err(_) => return None,
            };

            // Check if the file has been modified within the cache duration
            if current_modified
                .duration_since(*last_modified)
                .unwrap_or(Duration::from_millis(self.cache_duration_ms + 1))
                .as_millis() as u64
                <= self.cache_duration_ms
            {
                return Some(ast.clone());
            }
        }

        None
    }

    /// Stores an AST in the cache
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path of the file
    /// * `ast` - The AST to store
    ///
    /// # Returns
    ///
    /// `true` if the AST was stored successfully, `false` otherwise
    pub fn store(&mut self, file_path: &str, ast: Module) -> bool {
        // Get the file's last modified time
        let last_modified = match fs::metadata(file_path) {
            Ok(metadata) => match metadata.modified() {
                Ok(time) => time,
                Err(_) => return false,
            },
            Err(_) => return false,
        };

        // Store the AST and last modified time
        self.cache
            .insert(file_path.to_string(), (ast, last_modified));
        true
    }

    /// Clears the cache
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Returns the number of entries in the cache
    ///
    /// # Returns
    ///
    /// The number of entries in the cache
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns whether the cache is empty
    ///
    /// # Returns
    ///
    /// `true` if the cache is empty, `false` otherwise
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Returns the cache duration in milliseconds
    ///
    /// # Returns
    ///
    /// The cache duration in milliseconds
    pub fn cache_duration_ms(&self) -> u64 {
        self.cache_duration_ms
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_file_cache_new() {
//         let cache = FileCache::new(1000);
//         assert_eq!(cache.cache_duration_ms, 1000);
//     }

//     #[test]
//     fn test_file_cache_clear() {
//         let mut cache = FileCache::new(1000);
//         // Add some dummy entries
//         cache
//             .cache
//             .insert("test".to_string(), (Module::dummy(), SystemTime::now()));
//         assert_eq!(cache.len(), 1);
//         cache.clear();
//         assert_eq!(cache.len(), 0);
//     }

//     #[test]
//     fn test_file_cache_len() {
//         let mut cache = FileCache::new(1000);
//         assert_eq!(cache.len(), 0);
//         // Add some dummy entries
//         cache
//             .cache
//             .insert("test".to_string(), (Module::dummy(), SystemTime::now()));
//         assert_eq!(cache.len(), 1);
//     }

//     #[test]
//     fn test_file_cache_is_empty() {
//         let mut cache = FileCache::new(1000);
//         assert!(cache.is_empty());
//         // Add some dummy entries
//         cache
//             .cache
//             .insert("test".to_string(), (Module::dummy(), SystemTime::now()));
//         assert!(!cache.is_empty());
//     }

//     #[test]
//     fn test_file_cache_cache_duration_ms() {
//         let cache = FileCache::new(1000);
//         assert_eq!(cache.cache_duration_ms(), 1000);
//     }
// }
