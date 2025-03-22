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
use swc_core::ecma::ast::{ImportDecl, Module, ModuleItem, Program};
use swc_core::ecma::visit::{as_folder, FoldWith, VisitMut, VisitMutWith};
use swc_core::plugin::metadata::TransformPluginMetadataContextKind;
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

use cache::FileCache;
use import_transformer::process_import;
use pattern_matcher::path_matches_pattern;

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
    /// Plugin configuration
    config: Config,

    /// Compilation working directory
    cwd: String,

    /// Current file
    filename: String,

    /// File system cache
    file_cache: FileCache,

    /// Additional imports to be added to the module
    additional_imports: Vec<ModuleItem>,
}

impl BarrelTransformVisitor {
    /// Creates a new visitor with the specified configuration
    fn new(config: Config, cwd: String, filename: String) -> Self {
        let cache_duration_ms = config.cache_duration_ms.unwrap_or(1000);

        BarrelTransformVisitor {
            config,
            cwd,
            filename,
            file_cache: FileCache::new(cache_duration_ms),
            additional_imports: Vec::new(),
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
        // If no rules are provided, return None
        let rules = match &self.config.rules {
            Some(rules) => rules,
            None => return None,
        };

        // Check if the import path matches any patterns
        // Sort rules by specificity (fewer wildcards first)
        let mut matching_rules: Vec<&Rule> = rules
            .iter()
            .filter(|rule| path_matches_pattern(import_path, &rule.pattern))
            .collect();

        // Sort by specificity (fewer wildcards = more specific)
        matching_rules.sort_by_key(|rule| rule.pattern.matches('*').count());

        // Return the most specific matching rule
        matching_rules.first().copied()
    }
}

impl VisitMut for BarrelTransformVisitor {
    fn visit_mut_module(&mut self, module: &mut Module) {
        // Visit all module items
        module.visit_mut_children_with(self);
    }

    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        // If no rules are provided, do nothing
        if self.config.rules.is_none() {
            return;
        }

        // Get a copy of the import source value
        let import_source = import_decl.src.value.to_string();

        // Check if the import source matches any of our patterns
        if let Some(rule) = self.match_pattern(&import_source) {
            // Clone the rule to avoid borrowing issues
            let rule_clone = rule.clone();

            // Process the import based on the matched rule
            match process_import(
                &self.cwd,
                &self.filename,
                import_decl,
                &rule_clone.pattern,
                &rule_clone.paths,
                &mut self.file_cache,
            ) {
                Ok(new_imports) => {
                    if !new_imports.is_empty() {
                        // Replace the original import with the new direct imports
                        // We'll do this by replacing the current import with the first new import
                        // and adding the rest as new module items
                        *import_decl = new_imports[0].clone();

                        // Store additional imports to be added later
                        if new_imports.len() > 1 {
                            // We need to add these imports to the module
                            // This is handled in visit_mut_module_items
                            for import in new_imports.iter().skip(1) {
                                self.additional_imports.push(ModuleItem::ModuleDecl(
                                    swc_core::ecma::ast::ModuleDecl::Import(import.clone()),
                                ));
                            }
                        }
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

        // Continue traversing
        import_decl.visit_mut_children_with(self);
    }

    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        // First visit all items
        for item in items.iter_mut() {
            item.visit_mut_with(self);
        }

        // Then add any additional imports that were generated
        if !self.additional_imports.is_empty() {
            items.extend(self.additional_imports.drain(..));
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

    println!("CWD: {}", cwd);
    println!("Filename: {}", filename);

    // Parse the configuration
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
    use swc_core::common::DUMMY_SP;
    use swc_core::ecma::ast::{
        Ident, ImportDefaultSpecifier, ImportNamedSpecifier, ImportSpecifier, ModuleDecl,
        ModuleExportName, ModuleItem, Str,
    };

    #[test]
    fn test_config_parsing() {
        // Create a config directly instead of parsing JSON
        let rule = Rule {
            pattern: "#features/*".to_string(),
            paths: vec!["src/features/*/index.ts".to_string()],
        };

        let config = Config {
            rules: Some(vec![rule]),
            cache_duration_ms: Some(1000),
        };

        assert!(config.rules.is_some());
        let rules = config.rules.unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].pattern, "#features/*");
        assert_eq!(rules[0].paths, vec!["src/features/*/index.ts"]);
    }

    #[test]
    fn test_match_pattern() {
        // Create a visitor with test rules
        let rule1 = Rule {
            pattern: "#features/*".to_string(),
            paths: vec!["src/features/*/index.ts".to_string()],
        };

        let rule2 = Rule {
            pattern: "#features/*/testing".to_string(),
            paths: vec!["src/features/*/testing.ts".to_string()],
        };

        let config = Config {
            rules: Some(vec![rule1.clone(), rule2.clone()]),
            cache_duration_ms: Some(1000),
        };

        let visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

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

    #[test]
    fn test_visitor_with_no_rules() {
        // Create a visitor with no rules
        let config = Config {
            rules: None,
            cache_duration_ms: Some(1000),
        };

        let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

        // Create a test import declaration
        let mut import_decl = ImportDecl {
            span: DUMMY_SP,
            src: Box::new(Str {
                span: DUMMY_SP,
                value: "#features/some".into(),
                raw: None,
            }),
            type_only: false,
            with: None,
            specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
                span: DUMMY_SP,
                local: Ident {
                    span: DUMMY_SP,
                    sym: "Button".into(),
                    optional: false,
                },
                imported: None,
                is_type_only: false,
            })],
        };

        // Visit the import declaration
        visitor.visit_mut_import_decl(&mut import_decl);

        // The import declaration should not be modified
        assert_eq!(import_decl.src.value.to_string(), "#features/some");
    }

    #[test]
    fn test_visitor_with_empty_rules() {
        // Create a visitor with empty rules
        let config = Config {
            rules: Some(vec![]),
            cache_duration_ms: Some(1000),
        };

        let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

        // Create a test import declaration
        let mut import_decl = ImportDecl {
            span: DUMMY_SP,
            src: Box::new(Str {
                span: DUMMY_SP,
                value: "#features/some".into(),
                raw: None,
            }),
            type_only: false,
            with: None,
            specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
                span: DUMMY_SP,
                local: Ident {
                    span: DUMMY_SP,
                    sym: "Button".into(),
                    optional: false,
                },
                imported: None,
                is_type_only: false,
            })],
        };

        // Visit the import declaration
        visitor.visit_mut_import_decl(&mut import_decl);

        // The import declaration should not be modified
        assert_eq!(import_decl.src.value.to_string(), "#features/some");
    }

    #[test]
    fn test_visitor_with_non_matching_rules() {
        // Create a visitor with rules that don't match
        let rule = Rule {
            pattern: "#other/*".to_string(),
            paths: vec!["src/other/*/index.ts".to_string()],
        };

        let config = Config {
            rules: Some(vec![rule]),
            cache_duration_ms: Some(1000),
        };

        let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

        // Create a test import declaration
        let mut import_decl = ImportDecl {
            span: DUMMY_SP,
            src: Box::new(Str {
                span: DUMMY_SP,
                value: "#features/some".into(),
                raw: None,
            }),
            type_only: false,
            with: None,
            specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
                span: DUMMY_SP,
                local: Ident {
                    span: DUMMY_SP,
                    sym: "Button".into(),
                    optional: false,
                },
                imported: None,
                is_type_only: false,
            })],
        };

        // Visit the import declaration
        visitor.visit_mut_import_decl(&mut import_decl);

        // The import declaration should not be modified
        assert_eq!(import_decl.src.value.to_string(), "#features/some");
    }

    // #[test]
    // fn test_visit_mut_module_items() {
    //     // Create a visitor with test rules
    //     let rule = Rule {
    //         pattern: "#features/*".to_string(),
    //         paths: vec!["src/features/*/index.ts".to_string()],
    //     };

    //     let config = Config {
    //         rules: Some(vec![rule]),
    //         cache_duration_ms: Some(1000),
    //     };

    //     let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

    //     // Create a module with imports
    //     let mut module_items = vec![ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
    //         span: DUMMY_SP,
    //         src: Box::new(Str {
    //             span: DUMMY_SP,
    //             value: "#features/some".into(),
    //             raw: None,
    //         }),
    //         type_only: false,
    //         with: None,
    //         specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
    //             span: DUMMY_SP,
    //             local: Ident {
    //                 span: DUMMY_SP,
    //                 sym: "Button".into(),
    //                 optional: false,
    //             },
    //             imported: None,
    //             is_type_only: false,
    //         })],
    //     }))];

    //     // Add some additional imports to the visitor
    //     visitor
    //         .additional_imports
    //         .push(ModuleItem::ModuleDecl(ModuleDecl::Import(ImportDecl {
    //             span: DUMMY_SP,
    //             src: Box::new(Str {
    //                 span: DUMMY_SP,
    //                 value: "#features/other".into(),
    //                 raw: None,
    //             }),
    //             type_only: false,
    //             with: None,
    //             specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
    //                 span: DUMMY_SP,
    //                 local: Ident {
    //                     span: DUMMY_SP,
    //                     sym: "TextField".into(),
    //                     optional: false,
    //                 },
    //                 imported: None,
    //                 is_type_only: false,
    //             })],
    //         })));

    //     // Visit the module items
    //     visitor.visit_mut_module_items(&mut module_items);

    //     // The module items should now include the additional imports
    //     assert_eq!(module_items.len(), 2);
    //     if let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = &module_items[1] {
    //         assert_eq!(import.src.value.to_string(), "#features/other");
    //         if let ImportSpecifier::Named(named) = &import.specifiers[0] {
    //             assert_eq!(named.local.sym.to_string(), "TextField");
    //         } else {
    //             panic!("Expected named import specifier");
    //         }
    //     } else {
    //         panic!("Expected import declaration");
    //     }
    // }

    // #[test]
    // fn test_visitor_with_multiple_specifiers() {
    //     // Create a visitor with test rules
    //     let rule = Rule {
    //         pattern: "#features/*".to_string(),
    //         paths: vec!["src/features/*/index.ts".to_string()],
    //     };

    //     let config = Config {
    //         rules: Some(vec![rule]),
    //         cache_duration_ms: Some(1000),
    //     };

    //     let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

    //     // Create a test import declaration with multiple specifiers
    //     let mut import_decl = ImportDecl {
    //         span: DUMMY_SP,
    //         src: Box::new(Str {
    //             span: DUMMY_SP,
    //             value: "#features/some".into(),
    //             raw: None,
    //         }),
    //         type_only: false,
    //         with: None,
    //         specifiers: vec![
    //             ImportSpecifier::Named(ImportNamedSpecifier {
    //                 span: DUMMY_SP,
    //                 local: Ident {
    //                     span: DUMMY_SP,
    //                     sym: "Button".into(),
    //                     optional: false,
    //                 },
    //                 imported: None,
    //                 is_type_only: false,
    //             }),
    //             ImportSpecifier::Named(ImportNamedSpecifier {
    //                 span: DUMMY_SP,
    //                 local: Ident {
    //                     span: DUMMY_SP,
    //                     sym: "TextField".into(),
    //                     optional: false,
    //                 },
    //                 imported: None,
    //                 is_type_only: false,
    //             }),
    //         ],
    //     };

    //     // Mock the process_import function to return multiple imports
    //     // This is a bit tricky since we can't easily mock in Rust
    //     // For a real test, we would need to use a mocking library or refactor the code

    //     // Visit the import declaration
    //     visitor.visit_mut_import_decl(&mut import_decl);

    //     // Since we can't easily mock process_import, we'll just check that the code doesn't panic
    //     // In a real test, we would verify that the import declaration was correctly transformed
    // }

    // #[test]
    // fn test_visitor_with_default_import() {
    //     // Create a visitor with test rules
    //     let rule = Rule {
    //         pattern: "#features/*".to_string(),
    //         paths: vec!["src/features/*/index.ts".to_string()],
    //     };

    //     let config = Config {
    //         rules: Some(vec![rule]),
    //         cache_duration_ms: Some(1000),
    //     };

    //     let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

    //     // Create a test import declaration with a default import
    //     let mut import_decl = ImportDecl {
    //         span: DUMMY_SP,
    //         src: Box::new(Str {
    //             span: DUMMY_SP,
    //             value: "#features/some".into(),
    //             raw: None,
    //         }),
    //         type_only: false,
    //         with: None,
    //         specifiers: vec![ImportSpecifier::Default(ImportDefaultSpecifier {
    //             span: DUMMY_SP,
    //             local: Ident {
    //                 span: DUMMY_SP,
    //                 sym: "Button".into(),
    //                 optional: false,
    //             },
    //         })],
    //     };

    //     // Visit the import declaration
    //     visitor.visit_mut_import_decl(&mut import_decl);

    //     // Since we can't easily mock process_import, we'll just check that the code doesn't panic
    //     // In a real test, we would verify that the import declaration was correctly transformed
    // }

    // #[test]
    // fn test_visitor_with_renamed_import() {
    //     // Create a visitor with test rules
    //     let rule = Rule {
    //         pattern: "#features/*".to_string(),
    //         paths: vec!["src/features/*/index.ts".to_string()],
    //     };

    //     let config = Config {
    //         rules: Some(vec![rule]),
    //         cache_duration_ms: Some(1000),
    //     };

    //     let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

    //     // Create a test import declaration with a renamed import
    //     let mut import_decl = ImportDecl {
    //         span: DUMMY_SP,
    //         src: Box::new(Str {
    //             span: DUMMY_SP,
    //             value: "#features/some".into(),
    //             raw: None,
    //         }),
    //         type_only: false,
    //         with: None,
    //         specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
    //             span: DUMMY_SP,
    //             local: Ident {
    //                 span: DUMMY_SP,
    //                 sym: "MyButton".into(),
    //                 optional: false,
    //             },
    //             imported: Some(ModuleExportName::Ident(Ident {
    //                 span: DUMMY_SP,
    //                 sym: "Button".into(),
    //                 optional: false,
    //             })),
    //             is_type_only: false,
    //         })],
    //     };

    //     // Visit the import declaration
    //     visitor.visit_mut_import_decl(&mut import_decl);

    //     // Since we can't easily mock process_import, we'll just check that the code doesn't panic
    //     // In a real test, we would verify that the import declaration was correctly transformed
    // }

    // #[test]
    // fn test_visitor_with_type_only_import() {
    //     // Create a visitor with test rules
    //     let rule = Rule {
    //         pattern: "#features/*".to_string(),
    //         paths: vec!["src/features/*/index.ts".to_string()],
    //     };

    //     let config = Config {
    //         rules: Some(vec![rule]),
    //         cache_duration_ms: Some(1000),
    //     };

    //     let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

    //     // Create a test import declaration with a type-only import
    //     let mut import_decl = ImportDecl {
    //         span: DUMMY_SP,
    //         src: Box::new(Str {
    //             span: DUMMY_SP,
    //             value: "#features/some".into(),
    //             raw: None,
    //         }),
    //         type_only: true,
    //         with: None,
    //         specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
    //             span: DUMMY_SP,
    //             local: Ident {
    //                 span: DUMMY_SP,
    //                 sym: "ButtonProps".into(),
    //                 optional: false,
    //             },
    //             imported: None,
    //             is_type_only: true,
    //         })],
    //     };

    //     // Visit the import declaration
    //     visitor.visit_mut_import_decl(&mut import_decl);

    //     // Since we can't easily mock process_import, we'll just check that the code doesn't panic
    //     // In a real test, we would verify that the import declaration was correctly transformed
    // }

    // #[test]
    // fn test_visitor_with_multiple_rules() {
    //     // Create a visitor with multiple rules
    //     let rule1 = Rule {
    //         pattern: "#features/*".to_string(),
    //         paths: vec!["src/features/*/index.ts".to_string()],
    //     };

    //     let rule2 = Rule {
    //         pattern: "#entities/*".to_string(),
    //         paths: vec!["src/entities/*/index.ts".to_string()],
    //     };

    //     let config = Config {
    //         rules: Some(vec![rule1, rule2]),
    //         cache_duration_ms: Some(1000),
    //     };

    //     let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

    //     // Create a test import declaration
    //     let mut import_decl = ImportDecl {
    //         span: DUMMY_SP,
    //         src: Box::new(Str {
    //             span: DUMMY_SP,
    //             value: "#entities/user".into(),
    //             raw: None,
    //         }),
    //         type_only: false,
    //         with: None,
    //         specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
    //             span: DUMMY_SP,
    //             local: Ident {
    //                 span: DUMMY_SP,
    //                 sym: "User".into(),
    //                 optional: false,
    //             },
    //             imported: None,
    //             is_type_only: false,
    //         })],
    //     };

    //     // Visit the import declaration
    //     visitor.visit_mut_import_decl(&mut import_decl);

    //     // Since we can't easily mock process_import, we'll just check that the code doesn't panic
    //     // In a real test, we would verify that the import declaration was correctly transformed
    // }

    // #[test]
    // fn test_visitor_with_specific_and_wildcard_rules() {
    //     // Create a visitor with specific and wildcard rules
    //     let rule1 = Rule {
    //         pattern: "#features/user".to_string(),
    //         paths: vec!["src/specific-features/user/index.ts".to_string()],
    //     };

    //     let rule2 = Rule {
    //         pattern: "#features/*".to_string(),
    //         paths: vec!["src/features/*/index.ts".to_string()],
    //     };

    //     let config = Config {
    //         rules: Some(vec![rule1, rule2]),
    //         cache_duration_ms: Some(1000),
    //     };

    //     let mut visitor = BarrelTransformVisitor::new(config, "/".to_string(), "".to_string());

    //     // Create a test import declaration that matches the specific rule
    //     let mut import_decl = ImportDecl {
    //         span: DUMMY_SP,
    //         src: Box::new(Str {
    //             span: DUMMY_SP,
    //             value: "#features/user".into(),
    //             raw: None,
    //         }),
    //         type_only: false,
    //         with: None,
    //         specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
    //             span: DUMMY_SP,
    //             local: Ident {
    //                 span: DUMMY_SP,
    //                 sym: "User".into(),
    //                 optional: false,
    //             },
    //             imported: None,
    //             is_type_only: false,
    //         })],
    //     };

    //     // Visit the import declaration
    //     visitor.visit_mut_import_decl(&mut import_decl);

    //     // Since we can't easily mock process_import, we'll just check that the code doesn't panic
    //     // In a real test, we would verify that the import declaration was correctly transformed
    // }
}
