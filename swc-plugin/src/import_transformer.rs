use crate::cache::FileCache;
use crate::re_export::{analyze_barrel_file, ReExport};
use crate::resolver::{
    dirname, path_join, resolve_barrel_file, resolve_relative_path, resolve_to_virtual_path,
};
use std::collections::HashMap;
use std::path::Path;
use swc_core::common::sync::Lrc;
use swc_core::common::DUMMY_SP;
use swc_core::common::{
    errors::{ColorConfig, Handler},
    SourceMap,
};
use swc_core::ecma::ast::Module;
use swc_core::ecma::ast::{
    ImportDecl, ImportDefaultSpecifier, ImportNamedSpecifier, ImportSpecifier, ModuleExportName,
    Str,
};
use swc_core::ecma::parser::{parse_file_as_module, Syntax, TsConfig};

/// Transforms an import declaration by replacing barrel imports with direct imports
///
/// # Arguments
///
/// * `source_dir` - The directory containing the current source file
/// * `import_decl` - The import declaration to transform
/// * `barrel_file` - The path to the barrel file
/// * `re_exports` - The re-exports from the barrel file
///
/// # Returns
///
/// A vector of new import declarations that directly import from the original source files
fn transform_import(
    source_dir: &str,
    import_decl: &ImportDecl,
    barrel_file: &str,
    re_exports: &[ReExport],
) -> Result<Vec<ImportDecl>, String> {
    let mut new_imports = HashMap::new();
    let mut missing_exports = Vec::new();

    let barrel_file_dir = dirname(barrel_file);

    for specifier in &import_decl.specifiers {
        match specifier {
            ImportSpecifier::Named(named) => {
                let imported_name = named
                    .imported
                    .as_ref()
                    .map(|name| match name {
                        ModuleExportName::Ident(ident) => ident.sym.to_string(),
                        ModuleExportName::Str(str) => str.value.to_string(),
                    })
                    .unwrap_or_else(|| named.local.sym.to_string());

                if let Some(re_export) =
                    re_exports.iter().find(|e| e.exported_name == imported_name)
                {
                    let target_path = path_join(&barrel_file_dir, &re_export.source_path);
                    let import_path = resolve_relative_path(source_dir, &target_path).unwrap();

                    println!("Resolve START --------");
                    println!("source_dir: {}", source_dir);
                    println!("barrel_file_dir: {}", barrel_file_dir);
                    println!("export: {}", re_export.source_path);
                    println!("target_path: {}", target_path);
                    println!("import_path: {}", import_path);
                    println!("Resolve END --------");

                    if re_export.is_default {
                        let default_specifier = ImportSpecifier::Default(ImportDefaultSpecifier {
                            span: named.span,
                            local: named.local.clone(),
                        });

                        new_imports
                            .entry(import_path.clone())
                            .or_insert_with(Vec::new)
                            .push(default_specifier);
                    } else {
                        let new_specifier = ImportSpecifier::Named(ImportNamedSpecifier {
                            span: named.span,
                            local: named.local.clone(),
                            imported: if named.imported.is_some() {
                                named.imported.clone()
                            } else if re_export.original_name != imported_name
                                && !re_export.is_default
                            {
                                Some(ModuleExportName::Ident(swc_core::ecma::ast::Ident {
                                    span: DUMMY_SP,
                                    sym: re_export.original_name.clone().into(),
                                    optional: false,
                                }))
                            } else if re_export.is_default {
                                None
                            } else {
                                None
                            },
                            is_type_only: named.is_type_only,
                        });

                        new_imports
                            .entry(import_path.clone())
                            .or_insert_with(Vec::new)
                            .push(new_specifier);
                    }
                } else {
                    missing_exports.push(imported_name.clone());
                }
            }
            ImportSpecifier::Default(default) => {
                // Look for a re-export of the default export
                if let Some(re_export) = re_exports.iter().find(|e| e.is_default) {
                    let target_path = path_join(&barrel_file_dir, &re_export.source_path);
                    let import_path = resolve_relative_path(source_dir, &target_path).unwrap();

                    let new_specifier = ImportSpecifier::Default(ImportDefaultSpecifier {
                        span: default.span,
                        local: default.local.clone(),
                    });

                    new_imports
                        .entry(import_path.clone())
                        .or_insert_with(Vec::new)
                        .push(new_specifier);
                } else {
                    // The default export was not found in the barrel file
                    missing_exports.push("default".to_string());
                }
            }
            ImportSpecifier::Namespace(_) => {
                return Err(
                    "Namespace imports are not supported for barrel file optimization".to_string(),
                );
            }
        }
    }

    // Check if any imports were not found in the barrel file
    if !missing_exports.is_empty() {
        return Err(format!(
            "The following exports were not found in the barrel file {}: {}",
            barrel_file,
            missing_exports.join(", ")
        ));
    }

    // Create new import declarations for each source path
    let mut result = Vec::new();
    for (source_path, specifiers) in new_imports {
        let new_import = ImportDecl {
            span: import_decl.span,
            specifiers,
            src: Box::new(Str {
                span: DUMMY_SP,
                value: source_path.into(),
                raw: None,
            }),
            type_only: import_decl.type_only,
            with: import_decl.with.clone(),
        };

        result.push(new_import);
    }

    Ok(result)
}

/// Parses a file into an AST
fn parse_file(file_path: &str) -> Result<Module, String> {
    let cm: Lrc<SourceMap> = Default::default();
    let _handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));

    let fm = match cm.load_file(Path::new(file_path)) {
        Ok(fm) => fm,
        Err(e) => return Err(format!("Failed to load file: {}", e)),
    };

    let syntax = Syntax::Typescript(TsConfig {
        tsx: false,
        decorators: false,
        dts: false,
        no_early_errors: false,
        disallow_ambiguous_jsx_like: false,
    });

    match parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![]) {
        Ok(module) => Ok(module),
        Err(e) => Err(format!("Failed to parse file: {:?}", e)),
    }
}

/// Analyzes a barrel file and extracts re-export information
///
/// # Arguments
///
/// * `file_path` - The path to the barrel file
///
/// # Returns
///
/// A list of re-exports if the file is a valid barrel file, `Err` otherwise
fn parse_barrel_file_exports(file_path: &str) -> Result<Vec<ReExport>, String> {
    // Parse the file into an AST
    let ast = parse_file(file_path)?;

    // Analyze the barrel file to extract re-exports
    match analyze_barrel_file(&ast, file_path) {
        Ok(re_exports) => {
            // Check if any re-exports were found
            if re_exports.is_empty() {
                return Err(format!("No re-exports found in barrel file: {}", file_path));
            }
            Ok(re_exports)
        }
        Err(e) => Err(format!("Invalid barrel file {}: {}", file_path, e)),
    }
}

/// Processes an import declaration based on the matched rule
///
/// # Arguments
///
/// * `cwd` - Compilation working directory
/// * `file` - Current file
/// * `import_decl` - The import declaration to process
/// * `pattern` - The pattern that matched the import
/// * `paths` - The possible paths to resolve to
/// * `_file_cache` - The file cache (not used as per requirements)
///
/// # Returns
///
/// A vector of new import declarations that directly import from the original source files,
/// or an error if the barrel file could not be resolved or analyzed
pub fn process_import(
    cwd: &str,
    filename: &str,
    import_decl: &ImportDecl,
    pattern: &str,
    paths: &[String],
    _file_cache: &mut FileCache,
) -> Result<Vec<ImportDecl>, String> {
    // Resolve the barrel file
    let import_source = import_decl.src.value.to_string();
    let barrel_file = match resolve_barrel_file(cwd, &import_source, pattern, paths) {
        Ok(res) => match res {
            Some(path) => path,
            None => {
                return Err(format!(
                    "Could not resolve barrel file for import from {}",
                    import_source,
                ));
            }
        },
        Err(e) => {
            return Err(format!(
                "Could not resolve barrel file for {}: {}",
                import_source, e
            ));
        }
    };

    // Parse the barrel file directly without using cache
    let re_exports = match parse_barrel_file_exports(&barrel_file) {
        Ok(exports) => exports,
        Err(e) => {
            return Err(format!(
                "Error analyzing barrel file {}: {}",
                barrel_file, e
            ));
        }
    };

    let source_file = path_join(cwd, filename);
    let source_file = match resolve_to_virtual_path(cwd, &source_file) {
        Ok(path) => path,
        Err(e) => {
            return Err(format!(
                "Error resolving source file {}: {}",
                source_file, e
            ))
        }
    };
    let source_dir = dirname(&source_file);

    // Transform the import declaration
    match transform_import(&source_dir, import_decl, &barrel_file, &re_exports) {
        Ok(new_imports) => Ok(new_imports),
        Err(e) => Err(format!(
            "Error transforming import from {}: {}",
            import_source, e
        )),
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use swc_core::ecma::ast::Ident;

//     #[test]
//     fn test_transform_import() {
//         // Create a test import declaration
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: None,
//             specifiers: vec![
//                 ImportSpecifier::Named(ImportNamedSpecifier {
//                     span: DUMMY_SP,
//                     local: Ident {
//                         span: DUMMY_SP,
//                         sym: "User".into(),
//                         optional: false,
//                     },
//                     imported: None,
//                     is_type_only: false,
//                 }),
//                 ImportSpecifier::Named(ImportNamedSpecifier {
//                     span: DUMMY_SP,
//                     local: Ident {
//                         span: DUMMY_SP,
//                         sym: "createUser".into(),
//                         optional: false,
//                     },
//                     imported: None,
//                     is_type_only: false,
//                 }),
//             ],
//         };

//         // Create test re-exports
//         let re_exports = vec![
//             ReExport {
//                 exported_name: "User".to_string(),
//                 source_path: "./models".to_string(),
//                 original_name: "User".to_string(),
//                 is_default: false,
//             },
//             ReExport {
//                 exported_name: "createUser".to_string(),
//                 source_path: "./api".to_string(),
//                 original_name: "createUser".to_string(),
//                 is_default: false,
//             },
//         ];

//         // Transform the import declaration
//         let new_imports =
//             transform_import("/source/file/path.ts", &import_decl, "/path/to/entities/user/index.ts", &re_exports).unwrap();

//         // Check the result
//         assert_eq!(new_imports.len(), 2);

//         // Check that the imports are correctly transformed
//         let mut has_user_import = false;
//         let mut has_create_user_import = false;

//         for import in &new_imports {
//             if import.src.value.contains("models") {
//                 has_user_import = true;
//                 assert_eq!(import.specifiers.len(), 1);
//                 if let ImportSpecifier::Named(named) = &import.specifiers[0] {
//                     assert_eq!(named.local.sym.to_string(), "User");
//                 } else {
//                     panic!("Expected named import specifier");
//                 }
//             } else if import.src.value.contains("api") {
//                 has_create_user_import = true;
//                 assert_eq!(import.specifiers.len(), 1);
//                 if let ImportSpecifier::Named(named) = &import.specifiers[0] {
//                     assert_eq!(named.local.sym.to_string(), "createUser");
//                 } else {
//                     panic!("Expected named import specifier");
//                 }
//             }
//         }

//         assert!(has_user_import);
//         assert!(has_create_user_import);
//     }

//     #[test]
//     fn test_transform_import_with_default() {
//         // Create a test import declaration with a default import
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: None,
//             specifiers: vec![ImportSpecifier::Default(ImportDefaultSpecifier {
//                 span: DUMMY_SP,
//                 local: Ident {
//                     span: DUMMY_SP,
//                     sym: "User".into(),
//                     optional: false,
//                 },
//             })],
//         };

//         // Create test re-exports with a default export
//         let re_exports = vec![ReExport {
//             exported_name: "default".to_string(),
//             source_path: "./models".to_string(),
//             original_name: "default".to_string(),
//             is_default: true,
//         }];

//         // Transform the import declaration
//         let new_imports =
//             transform_import(&import_decl, "/path/to/entities/user/index.ts", &re_exports).unwrap();

//         // Check the result
//         assert_eq!(new_imports.len(), 1);

//         // Check that the import is correctly transformed
//         let import = &new_imports[0];
//         assert!(import.src.value.contains("models"));
//         assert_eq!(import.specifiers.len(), 1);

//         if let ImportSpecifier::Default(default) = &import.specifiers[0] {
//             assert_eq!(default.local.sym.to_string(), "User");
//         } else {
//             panic!("Expected default import specifier");
//         }
//     }

//     #[test]
//     fn test_transform_import_with_renamed_export() {
//         // Create a test import declaration
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: None,
//             specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
//                 span: DUMMY_SP,
//                 local: Ident {
//                     span: DUMMY_SP,
//                     sym: "CustomButton".into(),
//                     optional: false,
//                 },
//                 imported: None,
//                 is_type_only: false,
//             })],
//         };

//         // Create test re-exports with a renamed export
//         let re_exports = vec![ReExport {
//             exported_name: "CustomButton".to_string(),
//             source_path: "./components".to_string(),
//             original_name: "Button".to_string(),
//             is_default: false,
//         }];

//         // Transform the import declaration
//         let new_imports =
//             transform_import(&import_decl, "/path/to/entities/user/index.ts", &re_exports).unwrap();

//         // Check the result
//         assert_eq!(new_imports.len(), 1);

//         // Check that the import is correctly transformed
//         let import = &new_imports[0];
//         assert!(import.src.value.contains("components"));
//         assert_eq!(import.specifiers.len(), 1);

//         if let ImportSpecifier::Named(named) = &import.specifiers[0] {
//             assert_eq!(named.local.sym.to_string(), "CustomButton");
//             assert!(named.imported.is_some());
//             if let Some(ModuleExportName::Ident(ident)) = &named.imported {
//                 assert_eq!(ident.sym.to_string(), "Button");
//             } else {
//                 panic!("Expected imported name to be an identifier");
//             }
//         } else {
//             panic!("Expected named import specifier");
//         }
//     }

//     #[test]
//     fn test_transform_import_with_missing_export() {
//         // Create a test import declaration
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: None,
//             specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
//                 span: DUMMY_SP,
//                 local: Ident {
//                     span: DUMMY_SP,
//                     sym: "MissingExport".into(),
//                     optional: false,
//                 },
//                 imported: None,
//                 is_type_only: false,
//             })],
//         };

//         // Create test re-exports without the requested export
//         let re_exports = vec![ReExport {
//             exported_name: "User".to_string(),
//             source_path: "./models".to_string(),
//             original_name: "User".to_string(),
//             is_default: false,
//         }];

//         // Transform the import declaration
//         let result = transform_import(&import_decl, "/path/to/entities/user/index.ts", &re_exports);

//         // Check that the transformation fails with an appropriate error message
//         assert!(result.is_err());
//         let error = result.unwrap_err();
//         assert!(error.contains("MissingExport"));
//     }

//     #[test]
//     fn test_transform_import_with_namespace() {
//         // Create a test import declaration with a namespace import
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: None,
//             specifiers: vec![ImportSpecifier::Namespace(
//                 swc_core::ecma::ast::ImportStarAsSpecifier {
//                     span: DUMMY_SP,
//                     local: Ident {
//                         span: DUMMY_SP,
//                         sym: "UserModule".into(),
//                         optional: false,
//                     },
//                 },
//             )],
//         };

//         // Create test re-exports
//         let re_exports = vec![ReExport {
//             exported_name: "User".to_string(),
//             source_path: "./models".to_string(),
//             original_name: "User".to_string(),
//             is_default: false,
//         }];

//         // Transform the import declaration - should fail for namespace imports
//         let result = transform_import(&import_decl, "/path/to/entities/user/index.ts", &re_exports);

//         // Check that the transformation fails with the expected error message
//         assert!(result.is_err());
//         let error = result.unwrap_err();
//         assert!(error.contains("Namespace imports are not supported"));
//     }

//     #[test]
//     fn test_process_import_unresolvable() {
//         // Create a test import declaration
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: None,
//             specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
//                 span: DUMMY_SP,
//                 local: Ident {
//                     span: DUMMY_SP,
//                     sym: "User".into(),
//                     optional: false,
//                 },
//                 imported: None,
//                 is_type_only: false,
//             })],
//         };

//         // Create a pattern and paths that won't resolve
//         let pattern = "#entities/*";
//         let paths = vec!["./nonexistent/*/index.ts".to_string()];

//         // Process the import
//         let mut file_cache = FileCache::new(1000);
//         let result = process_import("/", &import_decl, pattern, &paths, &mut file_cache);

//         // Check that an error is returned when the barrel file can't be resolved
//         assert!(result.is_err());
//         let error = result.unwrap_err();
//         assert!(error.contains("Could not resolve barrel file"));
//     }

//     #[test]
//     fn test_parse_barrel_file_exports() {
//         // This test would normally require a real file, but we can mock the parse_file function
//         // using a test-specific implementation that returns a predefined AST

//         // For now, we'll test the error case when the file can't be parsed
//         let result = parse_barrel_file_exports("/nonexistent/file.ts");
//         assert!(result.is_err());
//         let error = result.unwrap_err();
//         assert!(error.contains("Failed to load file"));
//     }

//     // Additional test for parse_barrel_file_exports with empty exports
//     #[test]
//     fn test_parse_barrel_file_exports_empty() {
//         // This would require mocking the analyze_barrel_file function to return empty exports
//         // In a real implementation, we would use a test double or dependency injection
//         // For now, we'll just document what this test would verify

//         // The test would:
//         // 1. Mock parse_file to return a valid AST
//         // 2. Mock analyze_barrel_file to return empty exports
//         // 3. Call parse_barrel_file_exports
//         // 4. Verify that it returns an error with "No re-exports found" message
//     }

//     #[test]
//     fn test_parse_file() {
//         // Test the error case for parse_file
//         let result = parse_file("/nonexistent/file.ts");
//         assert!(result.is_err());
//         let error = result.unwrap_err();
//         assert!(error.contains("Failed to load file"));
//     }

//     // Test for transform_import with both named and default specifiers
//     #[test]
//     fn test_transform_import_with_mixed_specifiers() {
//         // Create a test import declaration with both default and named imports
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: None,
//             specifiers: vec![
//                 ImportSpecifier::Default(ImportDefaultSpecifier {
//                     span: DUMMY_SP,
//                     local: Ident {
//                         span: DUMMY_SP,
//                         sym: "User".into(),
//                         optional: false,
//                     },
//                 }),
//                 ImportSpecifier::Named(ImportNamedSpecifier {
//                     span: DUMMY_SP,
//                     local: Ident {
//                         span: DUMMY_SP,
//                         sym: "createUser".into(),
//                         optional: false,
//                     },
//                     imported: None,
//                     is_type_only: false,
//                 }),
//             ],
//         };

//         // Create test re-exports with both default and named exports
//         let re_exports = vec![
//             ReExport {
//                 exported_name: "default".to_string(),
//                 source_path: "./models".to_string(),
//                 original_name: "default".to_string(),
//                 is_default: true,
//             },
//             ReExport {
//                 exported_name: "createUser".to_string(),
//                 source_path: "./api".to_string(),
//                 original_name: "createUser".to_string(),
//                 is_default: false,
//             },
//         ];

//         // Transform the import declaration
//         let new_imports =
//             transform_import(&import_decl, "/path/to/entities/user/index.ts", &re_exports).unwrap();

//         // Check the result
//         assert_eq!(new_imports.len(), 2);

//         // Check that the imports are correctly transformed
//         let mut has_user_import = false;
//         let mut has_create_user_import = false;

//         for import in &new_imports {
//             if import.src.value.contains("models") {
//                 has_user_import = true;
//                 assert_eq!(import.specifiers.len(), 1);
//                 if let ImportSpecifier::Default(default) = &import.specifiers[0] {
//                     assert_eq!(default.local.sym.to_string(), "User");
//                 } else {
//                     panic!("Expected default import specifier");
//                 }
//             } else if import.src.value.contains("api") {
//                 has_create_user_import = true;
//                 assert_eq!(import.specifiers.len(), 1);
//                 if let ImportSpecifier::Named(named) = &import.specifiers[0] {
//                     assert_eq!(named.local.sym.to_string(), "createUser");
//                 } else {
//                     panic!("Expected named import specifier");
//                 }
//             }
//         }

//         assert!(has_user_import);
//         assert!(has_create_user_import);
//     }

//     // Test for type-only imports
//     #[test]
//     fn test_transform_import_with_type_only() {
//         // Create a test import declaration with type-only import
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: true,
//             with: None,
//             specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
//                 span: DUMMY_SP,
//                 local: Ident {
//                     span: DUMMY_SP,
//                     sym: "UserType".into(),
//                     optional: false,
//                 },
//                 imported: None,
//                 is_type_only: false,
//             })],
//         };

//         // Create test re-exports
//         let re_exports = vec![ReExport {
//             exported_name: "UserType".to_string(),
//             source_path: "./types".to_string(),
//             original_name: "UserType".to_string(),
//             is_default: false,
//         }];

//         // Transform the import declaration
//         let new_imports =
//             transform_import(&import_decl, "/path/to/entities/user/index.ts", &re_exports).unwrap();

//         // Check the result
//         assert_eq!(new_imports.len(), 1);

//         // Check that the type-only flag is preserved
//         assert!(new_imports[0].type_only);

//         // Check that the import is correctly transformed
//         assert!(new_imports[0].src.value.contains("types"));
//         assert_eq!(new_imports[0].specifiers.len(), 1);

//         if let ImportSpecifier::Named(named) = &new_imports[0].specifiers[0] {
//             assert_eq!(named.local.sym.to_string(), "UserType");
//         } else {
//             panic!("Expected named import specifier");
//         }
//     }

//     // Test for imported specifier with a different local name
//     #[test]
//     fn test_transform_import_with_renamed_import() {
//         // Create a test import declaration with a renamed import
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: None,
//             specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
//                 span: DUMMY_SP,
//                 local: Ident {
//                     span: DUMMY_SP,
//                     sym: "LocalUser".into(),
//                     optional: false,
//                 },
//                 imported: Some(ModuleExportName::Ident(Ident {
//                     span: DUMMY_SP,
//                     sym: "User".into(),
//                     optional: false,
//                 })),
//                 is_type_only: false,
//             })],
//         };

//         // Create test re-exports
//         let re_exports = vec![ReExport {
//             exported_name: "User".to_string(),
//             source_path: "./models".to_string(),
//             original_name: "User".to_string(),
//             is_default: false,
//         }];

//         // Transform the import declaration
//         let new_imports =
//             transform_import(&import_decl, "/path/to/entities/user/index.ts", &re_exports).unwrap();

//         // Check the result
//         assert_eq!(new_imports.len(), 1);

//         // Check that the import is correctly transformed
//         assert!(new_imports[0].src.value.contains("models"));
//         assert_eq!(new_imports[0].specifiers.len(), 1);

//         if let ImportSpecifier::Named(named) = &new_imports[0].specifiers[0] {
//             assert_eq!(named.local.sym.to_string(), "LocalUser");
//             // The imported name should be preserved
//             assert!(named.imported.is_some());
//             if let Some(ModuleExportName::Ident(ident)) = &named.imported {
//                 assert_eq!(ident.sym.to_string(), "User");
//             } else {
//                 panic!("Expected imported name to be an identifier");
//             }
//         } else {
//             panic!("Expected named import specifier");
//         }
//     }

//     // Test for process_import with a mock resolver and parser
//     #[test]
//     fn test_process_import_mock() {
//         // Create a test import declaration
//         let _import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: None,
//             specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
//                 span: DUMMY_SP,
//                 local: Ident {
//                     span: DUMMY_SP,
//                     sym: "User".into(),
//                     optional: false,
//                 },
//                 imported: None,
//                 is_type_only: false,
//             })],
//         };

//         // Create a pattern and paths
//         let _pattern = "#entities/*";
//         let _paths = vec!["./src/entities/*/index.ts".to_string()];

//         // Create a file cache
//         let _file_cache = FileCache::new(1000);

//         // In a real test with mocking capabilities, we would:
//         // 1. Mock resolve_barrel_file to return a specific path
//         // 2. Mock parse_barrel_file_exports to return specific re-exports
//         // 3. Call process_import and verify the result

//         // For now, we'll just document what this test would verify
//         // and rely on the existing test_process_import_unresolvable test
//         // for actual coverage of the process_import function
//     }

//     // Test for handling imports with 'with' attribute (import assertions)
//     #[test]
//     fn test_transform_import_with_assertions() {
//         // Create a test import declaration with import assertions
//         let import_decl = ImportDecl {
//             span: DUMMY_SP,
//             src: Box::new(Str {
//                 span: DUMMY_SP,
//                 value: "#entities/user".into(),
//                 raw: None,
//             }),
//             type_only: false,
//             with: Some(Box::new(swc_core::ecma::ast::ObjectLit {
//                 span: DUMMY_SP,
//                 props: vec![swc_core::ecma::ast::PropOrSpread::Prop(Box::new(
//                     swc_core::ecma::ast::Prop::KeyValue(swc_core::ecma::ast::KeyValueProp {
//                         key: swc_core::ecma::ast::PropName::Ident(Ident {
//                             span: DUMMY_SP,
//                             sym: "type".into(),
//                             optional: false,
//                         }),
//                         value: Box::new(swc_core::ecma::ast::Expr::Lit(
//                             swc_core::ecma::ast::Lit::Str(Str {
//                                 span: DUMMY_SP,
//                                 value: "json".into(),
//                                 raw: None,
//                             }),
//                         )),
//                     }),
//                 ))],
//             })),
//             specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
//                 span: DUMMY_SP,
//                 local: Ident {
//                     span: DUMMY_SP,
//                     sym: "User".into(),
//                     optional: false,
//                 },
//                 imported: None,
//                 is_type_only: false,
//             })],
//         };

//         // Create test re-exports
//         let re_exports = vec![ReExport {
//             exported_name: "User".to_string(),
//             source_path: "./models".to_string(),
//             original_name: "User".to_string(),
//             is_default: false,
//         }];

//         // Transform the import declaration
//         let new_imports =
//             transform_import(&import_decl, "/path/to/entities/user/index.ts", &re_exports).unwrap();

//         // Check the result
//         assert_eq!(new_imports.len(), 1);

//         // Check that the import assertions are preserved
//         assert!(new_imports[0].with.is_some());

//         // Check that the import is correctly transformed
//         assert!(new_imports[0].src.value.contains("models"));
//         assert_eq!(new_imports[0].specifiers.len(), 1);

//         if let ImportSpecifier::Named(named) = &new_imports[0].specifiers[0] {
//             assert_eq!(named.local.sym.to_string(), "User");
//         } else {
//             panic!("Expected named import specifier");
//         }
//     }
// }
