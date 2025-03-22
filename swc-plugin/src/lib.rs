//! SWC Plugin for Barrel Files
//!
//! This plugin transforms imports from barrel files (index.ts) into direct imports
//! from the source files. This helps to avoid circular dependencies and improves tree-shaking.

mod cache;
mod import_transformer;
mod pattern_matcher;
mod re_export;
mod resolver;

use serde::Deserialize;
// use swc_core::common::errors::HANDLER;
// use swc_core::common::DUMMY_SP;
use std::collections::HashMap;
use swc_core::ecma::ast::{ImportDecl, Module, ModuleItem, Program};
use swc_core::ecma::visit::{as_folder, FoldWith, VisitMut, VisitMutWith};
use swc_core::plugin::metadata::TransformPluginMetadataContextKind;
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

use cache::FileCache;
use import_transformer::process_import;
use pattern_matcher::{count_wildcards, path_matches_pattern};

/// Configuration for the barrel files plugin
#[derive(Deserialize, Debug)]
struct Config {
    /// Rules for pattern matching (optional)
    rules: Option<Vec<Rule>>,

    /// Cache duration in milliseconds (optional, defaults to 1000)
    cache_duration_ms: Option<u64>,
}

/// Rule for matching import paths and resolving barrel files
#[derive(Deserialize, Debug, Clone)]
struct Rule {
    /// Pattern to match (e.g., '#entities/*')
    pattern: String,

    /// Possible paths to resolve (e.g., ['src/entities/*/index.ts'])
    paths: Vec<String>,
}

/// Visitor for transforming barrel file imports
struct BarrelTransformVisitor {
    /// Compilation working directory
    cwd: String,

    /// Current file
    filename: String,

    /// File system cache
    _file_cache: FileCache,

    /// Map of import declarations to their replacements
    /// The key is the span of the original import, and the value is a vector of replacement imports
    import_replacements: HashMap<u32, Vec<ImportDecl>>,

    /// Rules sorted by specificity (fewer wildcards first)
    sorted_rules: Vec<Rule>,
}

impl BarrelTransformVisitor {
    /// Creates a new visitor with the specified configuration
    fn new(config: Config, cwd: String, filename: String) -> Self {
        let cache_duration_ms = config.cache_duration_ms.unwrap_or(1000);

        // Pre-sort rules by specificity (fewer wildcards = more specific)
        let sorted_rules = match &config.rules {
            Some(rules) => {
                let mut sorted = rules.clone();
                sorted.sort_by_key(|rule| count_wildcards(&rule.pattern));
                sorted
            }
            None => Vec::new(),
        };

        BarrelTransformVisitor {
            cwd,
            filename,
            _file_cache: FileCache::new(cache_duration_ms),
            import_replacements: HashMap::new(),
            sorted_rules,
        }
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
    fn match_pattern(&self, import_path: &str) -> Option<&Rule> {
        if self.sorted_rules.is_empty() {
            return None;
        }

        self.sorted_rules
            .iter()
            .find(|rule| path_matches_pattern(import_path, &rule.pattern))
    }
}

impl VisitMut for BarrelTransformVisitor {
    fn visit_mut_module(&mut self, module: &mut Module) {
        module.visit_mut_children_with(self);
    }

    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        if self.sorted_rules.is_empty() {
            return;
        }

        let import_source = import_decl.src.value.to_string();

        if let Some(rule) = self.match_pattern(&import_source) {
            match process_import(
                &self.cwd,
                &self.filename,
                import_decl,
                &rule.pattern,
                &rule.paths,
            ) {
                Ok(new_imports) => {
                    if !new_imports.is_empty() {
                        // Store the span of the original import as a key
                        // We'll use this to identify the import in visit_mut_module_items
                        let span_lo = import_decl.span.lo.0;

                        // Store all the replacement imports
                        self.import_replacements
                            .insert(span_lo, new_imports.clone());
                    }
                }
                Err(e) => {
                    // Report the error to SWC instead of panicking
                    // let span = import_decl.span;
                    // HANDLER.with(|handler| {
                    //     handler
                    //         .struct_span_err(
                    //             span,
                    //             &format!("Error processing barrel import: {}", e),
                    //         )
                    //         .emit();
                    // });
                    panic!("Error processing barrel import: {}", e)
                }
            }
        }

        import_decl.visit_mut_children_with(self);
    }

    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        // First, collect all import declarations and their positions
        let mut import_positions = Vec::new();
        for (i, item) in items.iter().enumerate() {
            if let ModuleItem::ModuleDecl(swc_core::ecma::ast::ModuleDecl::Import(import)) = item {
                import_positions.push((i, import.span.lo.0));
            }
        }

        // Now visit all items
        for item in items.iter_mut() {
            item.visit_mut_with(self);
        }

        // Then replace original imports with their replacements
        if !self.import_replacements.is_empty() {
            // Process imports in reverse order to avoid invalidating indices
            import_positions.sort_by(|a, b| b.0.cmp(&a.0));

            for (pos, span_lo) in import_positions {
                if let Some(mut replacements) = self.import_replacements.get(&span_lo).cloned() {
                    // Sort replacements by module source
                    replacements
                        .sort_by(|a, b| a.src.value.to_string().cmp(&b.src.value.to_string()));

                    // Remove the original import
                    items.remove(pos);

                    // Insert all replacements at the position of the removed import
                    let mut insert_pos = pos;

                    for import in replacements.iter() {
                        items.insert(
                            insert_pos,
                            ModuleItem::ModuleDecl(swc_core::ecma::ast::ModuleDecl::Import(
                                import.clone(),
                            )),
                        );
                        insert_pos += 1;
                    }
                }
            }

            self.import_replacements.clear();
        }
    }
}

/// SWC plugin transform entry point
///
/// This function is called by SWC to transform the AST.
#[plugin_transform]
pub fn process_transform(program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    let cwd = match metadata.get_context(&TransformPluginMetadataContextKind::Cwd) {
        Some(cwd) => cwd,
        None => {
            panic!("Current working directory is not available");
        }
    };

    let filename = match metadata.get_context(&TransformPluginMetadataContextKind::Filename) {
        Some(filename) => filename,
        None => {
            panic!("Current filename is not available");
        }
    };

    let config: Config = match serde_json::from_str(
        &metadata
            .get_transform_plugin_config()
            .unwrap_or_else(|| "{}".to_string()),
    ) {
        Ok(config) => config,
        Err(e) => {
            panic!("Error parsing barrel plugin configuration: {}", e);
        }
    };

    let visitor = BarrelTransformVisitor::new(config, cwd, filename);

    program.fold_with(&mut as_folder(visitor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing() {
        let config_json = r#"{
            "rules": [
                {
                    "pattern": "@features/*",
                    "paths": ["src/features/*/index.ts"]
                }
            ],
            "cache_duration_ms": 1000
        }"#;

        let config: Config =
            serde_json::from_str(config_json).expect("Failed to parse config JSON");

        assert!(config.rules.is_some());
        let rules = config.rules.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "@features/*");
        assert_eq!(rules[0].paths, vec!["src/features/*/index.ts"]);
        assert_eq!(config.cache_duration_ms, Some(1000));
    }

    #[test]
    fn test_match_pattern() {
        let rule1 = Rule {
            pattern: "#features/*".to_string(),
            paths: vec!["src/features/*/index.ts".to_string()],
        };

        let rule2 = Rule {
            pattern: "#features/*/testing".to_string(),
            paths: vec!["src/features/*/testing.ts".to_string()],
        };

        // Create config with rules in reverse order of specificity
        // to ensure sorting works
        let config = Config {
            rules: Some(vec![rule2.clone(), rule1.clone()]),
            cache_duration_ms: Some(1000),
        };

        let visitor = BarrelTransformVisitor::new(config, "/".to_string(), "test.ts".to_string());

        // The more specific rule should be first in sorted_rules
        assert_eq!(visitor.sorted_rules[0].pattern, "#features/*/testing");
        assert_eq!(visitor.sorted_rules[1].pattern, "#features/*");

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
}
