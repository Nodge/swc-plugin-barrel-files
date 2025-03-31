//! Re-export analyzer module for the barrel files plugin
//!
//! This module provides functionality for analyzing barrel files and extracting re-export information.

use std::path::Path;
use swc_core::ecma::ast::{
    Decl, ExportSpecifier, Module, ModuleDecl, ModuleExportName, ModuleItem,
};

/// Represents a re-export from a barrel file
#[derive(Debug, Clone, PartialEq)]
pub struct ReExport {
    /// The name under which the export is exposed (may be renamed)
    pub exported_name: String,

    /// The path from which the export is imported
    pub source_path: String,

    /// The original name of the export in the source file
    pub original_name: String,

    /// Whether this is a default export
    pub is_default: bool,
}

/// Error type for barrel file analysis
#[derive(Debug, Clone, PartialEq)]
pub enum BarrelError {
    /// The barrel file contains non-export code
    NonExportCode(String),

    /// The barrel file contains wildcard exports
    WildcardExport(String),

    /// The barrel file contains namespace exports
    NamespaceExport(String),

    /// The barrel file contains an export declaration without a source
    MissingSource(String),
}

impl std::fmt::Display for BarrelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BarrelError::NonExportCode(msg) => {
                write!(f, "Barrel file contains non-export code: {}", msg)
            }
            BarrelError::WildcardExport(msg) => write!(
                f,
                "Wildcard exports are not supported in barrel files: {}",
                msg
            ),
            BarrelError::NamespaceExport(msg) => write!(
                f,
                "Namespace exports are not supported in barrel files: {}",
                msg
            ),
            BarrelError::MissingSource(msg) => {
                write!(f, "Export declaration without source: {}", msg)
            }
        }
    }
}

impl std::error::Error for BarrelError {}

/// Validates that a file only contains re-exports
///
/// # Arguments
///
/// * `ast` - The AST of the barrel file
///
/// # Returns
///
/// `Ok(())` if the file only contains re-exports, `Err` otherwise
fn validate_barrel_file(ast: &Module) -> Result<(), BarrelError> {
    // Check that the file only contains export declarations
    for item in &ast.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(_)) => {
                // Named exports are allowed
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export_decl)) => {
                // Check that the export declaration only contains simple declarations
                match &export_decl.decl {
                    Decl::Var(_) => {
                        return Err(BarrelError::NonExportCode(
                            "Variable declarations are not allowed in barrel files".into(),
                        ));
                    }
                    Decl::Class(_) => {
                        return Err(BarrelError::NonExportCode(
                            "Class declarations are not allowed in barrel files".into(),
                        ));
                    }
                    Decl::Fn(_) => {
                        return Err(BarrelError::NonExportCode(
                            "Function declarations are not allowed in barrel files".into(),
                        ));
                    }
                    Decl::TsInterface(_) => {
                        return Err(BarrelError::NonExportCode(
                            "Interface declarations are not allowed in barrel files".into(),
                        ));
                    }
                    Decl::TsTypeAlias(_) => {
                        return Err(BarrelError::NonExportCode(
                            "Type alias declarations are not allowed in barrel files".into(),
                        ));
                    }
                    Decl::TsEnum(_) => {
                        return Err(BarrelError::NonExportCode(
                            "Enum declarations are not allowed in barrel files".into(),
                        ));
                    }
                    Decl::TsModule(_) => {
                        return Err(BarrelError::NonExportCode(
                            "Module declarations are not allowed in barrel files".into(),
                        ));
                    }
                    _ => {
                        return Err(BarrelError::NonExportCode(
                            "Unknown declaration type in barrel file".into(),
                        ));
                    }
                }
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportAll(_)) => {
                return Err(BarrelError::WildcardExport(
                    "Wildcard exports are not allowed in barrel files".into(),
                ));
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(_)) => {
                return Err(BarrelError::NonExportCode(
                    "Default export declarations are not allowed in barrel files".into(),
                ));
            }
            ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultExpr(_)) => {
                return Err(BarrelError::NonExportCode(
                    "Default export expressions are not allowed in barrel files".into(),
                ));
            }
            ModuleItem::ModuleDecl(ModuleDecl::Import(_)) => {
                return Err(BarrelError::NonExportCode(
                    "Import declarations are not allowed in barrel files".into(),
                ));
            }
            ModuleItem::ModuleDecl(ModuleDecl::TsImportEquals(_)) => {
                return Err(BarrelError::NonExportCode(
                    "TypeScript import equals declarations are not allowed in barrel files".into(),
                ));
            }
            ModuleItem::ModuleDecl(ModuleDecl::TsExportAssignment(_)) => {
                return Err(BarrelError::NonExportCode(
                    "TypeScript export assignments are not allowed in barrel files".into(),
                ));
            }
            ModuleItem::ModuleDecl(ModuleDecl::TsNamespaceExport(_)) => {
                return Err(BarrelError::NonExportCode(
                    "TypeScript namespace exports are not allowed in barrel files".into(),
                ));
            }
            ModuleItem::Stmt(_) => {
                return Err(BarrelError::NonExportCode(
                    "Statements are not allowed in barrel files".into(),
                ));
            }
        }
    }

    Ok(())
}

/// Analyzes a barrel file and extracts re-export information
///
/// # Arguments
///
/// * `ast` - The AST of the barrel file
/// * `file_path` - The path of the barrel file
///
/// # Returns
///
/// A list of re-exports if the file is a valid barrel file, `Err` otherwise
pub fn analyze_barrel_file(ast: &Module, file_path: &str) -> Result<Vec<ReExport>, BarrelError> {
    validate_barrel_file(ast)?;

    let mut re_exports = Vec::new();
    let _barrel_dir = Path::new(file_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));

    for item in &ast.body {
        if let ModuleItem::ModuleDecl(ModuleDecl::ExportNamed(export)) = item {
            // Handle named exports
            for specifier in &export.specifiers {
                match specifier {
                    ExportSpecifier::Named(named) => {
                        let exported_name = match &named.exported {
                            Some(ModuleExportName::Ident(ident)) => ident.sym.to_string(),
                            Some(ModuleExportName::Str(str)) => str.value.to_string(),
                            None => match &named.orig {
                                ModuleExportName::Ident(ident) => ident.sym.to_string(),
                                ModuleExportName::Str(str) => str.value.to_string(),
                            },
                        };

                        let original_name = match &named.orig {
                            ModuleExportName::Ident(ident) => ident.sym.to_string(),
                            ModuleExportName::Str(str) => str.value.to_string(),
                        };

                        if let Some(src) = &export.src {
                            let source_path = src.value.to_string();

                            re_exports.push(ReExport {
                                exported_name,
                                source_path,
                                original_name: original_name.clone(),
                                is_default: original_name == "default",
                            });
                        } else {
                            return Err(BarrelError::MissingSource(format!(
                                "Export '{}' does not have a source",
                                exported_name
                            )));
                        }
                    }
                    ExportSpecifier::Default(default) => {
                        if let Some(src) = &export.src {
                            let source_path = src.value.to_string();

                            re_exports.push(ReExport {
                                exported_name: default.exported.sym.to_string(),
                                source_path,
                                original_name: "default".to_string(),
                                is_default: true,
                            });
                        } else {
                            return Err(BarrelError::MissingSource(
                                "Default export does not have a source".to_string(),
                            ));
                        }
                    }
                    ExportSpecifier::Namespace(ns) => {
                        let exported_name = match &ns.name {
                            ModuleExportName::Ident(ident) => ident.sym.to_string(),
                            ModuleExportName::Str(str) => str.value.to_string(),
                        };

                        if let Some(src) = &export.src {
                            let source_path = src.value.to_string();

                            return Err(BarrelError::NamespaceExport(format!(
                                "export * as {} from '{}'",
                                exported_name, source_path
                            )));
                        } else {
                            return Err(BarrelError::MissingSource(format!(
                                "Namespace export '{}' does not have a source",
                                exported_name
                            )));
                        }
                    }
                }
            }
        } else if let ModuleItem::ModuleDecl(ModuleDecl::ExportAll(_)) = item {
            return Err(BarrelError::WildcardExport(
                "Wildcard exports are not allowed in barrel files".to_string(),
            ));
        }
    }

    Ok(re_exports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use swc_core::common::DUMMY_SP;
    use swc_core::ecma::ast::{
        BlockStmt, DefaultDecl, EmptyStmt, ExportAll, ExportNamedSpecifier, FnExpr, Ident,
        ImportDecl, ImportNamedSpecifier, ImportSpecifier, NamedExport, Stmt, Str,
    };

    #[test]
    fn test_validate_barrel_file() {
        // Create a valid barrel file AST with named exports
        let mut module = Module {
            span: DUMMY_SP,
            body: vec![],
            shebang: None,
        };

        // Add a named export
        let named_export = ModuleDecl::ExportNamed(NamedExport {
            span: DUMMY_SP,
            specifiers: vec![ExportSpecifier::Named(ExportNamedSpecifier {
                span: DUMMY_SP,
                orig: ModuleExportName::Ident(Ident {
                    span: DUMMY_SP,
                    sym: "Button".into(),
                    optional: false,
                    ctxt: Default::default(),
                }),
                exported: None,
                is_type_only: false,
            })],
            src: Some(Box::new(Str {
                span: DUMMY_SP,
                value: "./components/Button".into(),
                raw: None,
            })),
            type_only: false,
            with: None,
        });

        module.body.push(ModuleItem::ModuleDecl(named_export));

        // Validate the barrel file
        let result = validate_barrel_file(&module);
        assert!(result.is_ok());

        // Create an invalid barrel file AST with a import declaration
        let mut module = Module {
            span: DUMMY_SP,
            body: vec![],
            shebang: None,
        };

        // Add an import declaration
        let import_decl = ModuleDecl::Import(ImportDecl {
            span: DUMMY_SP,
            specifiers: vec![ImportSpecifier::Named(ImportNamedSpecifier {
                span: DUMMY_SP,
                local: Ident {
                    span: DUMMY_SP,
                    sym: "Button".into(),
                    optional: false,
                    ctxt: Default::default(),
                },
                imported: None,
                is_type_only: false,
            })],
            src: Box::new(Str {
                span: DUMMY_SP,
                value: "./components/Button".into(),
                raw: None,
            }),
            type_only: false,
            with: None,
            phase: Default::default(),
        });

        module.body.push(ModuleItem::ModuleDecl(import_decl));

        // Validate the barrel file
        let result = validate_barrel_file(&module);
        assert!(result.is_err());
        match result {
            Err(BarrelError::NonExportCode(_)) => {}
            _ => panic!("Expected NonExportCode error"),
        }

        // Create an invalid barrel file AST with a wildcard export
        let mut module = Module {
            span: DUMMY_SP,
            body: vec![],
            shebang: None,
        };

        // Add a wildcard export
        let wildcard_export = ModuleDecl::ExportAll(ExportAll {
            span: DUMMY_SP,
            src: Box::new(Str {
                span: DUMMY_SP,
                value: "./components".into(),
                raw: None,
            }),
            with: None,
            type_only: false,
        });

        module.body.push(ModuleItem::ModuleDecl(wildcard_export));

        // Validate the barrel file
        let result = validate_barrel_file(&module);
        assert!(result.is_err());
        match result {
            Err(BarrelError::WildcardExport(_)) => {}
            _ => panic!("Expected WildcardExport error"),
        }

        // Create an invalid barrel file AST with a non-export statement
        let mut module = Module {
            span: DUMMY_SP,
            body: vec![],
            shebang: None,
        };

        // Add a non-export statement
        module
            .body
            .push(ModuleItem::Stmt(Stmt::Empty(EmptyStmt { span: DUMMY_SP })));

        // Validate the barrel file
        let result = validate_barrel_file(&module);
        assert!(result.is_err());
        match result {
            Err(BarrelError::NonExportCode(_)) => {}
            _ => panic!("Expected NonExportCode error"),
        }

        // Create an invalid barrel file AST with a default export declaration
        let mut module = Module {
            span: DUMMY_SP,
            body: vec![],
            shebang: None,
        };

        // Add a default export declaration
        let default_export =
            ModuleDecl::ExportDefaultDecl(swc_core::ecma::ast::ExportDefaultDecl {
                span: DUMMY_SP,
                decl: DefaultDecl::Fn(FnExpr {
                    ident: None,
                    function: Box::new(swc_core::ecma::ast::Function {
                        params: vec![],
                        decorators: vec![],
                        span: DUMMY_SP,
                        body: Some(BlockStmt {
                            span: DUMMY_SP,
                            stmts: vec![],
                            ctxt: Default::default(),
                        }),
                        is_generator: false,
                        is_async: false,
                        type_params: None,
                        return_type: None,
                        ctxt: Default::default(),
                    }),
                }),
            });

        module.body.push(ModuleItem::ModuleDecl(default_export));

        // Validate the barrel file
        let result = validate_barrel_file(&module);
        assert!(result.is_err());
        match result {
            Err(BarrelError::NonExportCode(_)) => {}
            _ => panic!("Expected NonExportCode error"),
        }
    }

    #[test]
    fn test_analyze_barrel_file() {
        // Create a valid barrel file AST with named exports
        let mut module = Module {
            span: DUMMY_SP,
            body: vec![],
            shebang: None,
        };

        // Add a named export
        let named_export = ModuleDecl::ExportNamed(NamedExport {
            span: DUMMY_SP,
            specifiers: vec![ExportSpecifier::Named(ExportNamedSpecifier {
                span: DUMMY_SP,
                orig: ModuleExportName::Ident(Ident {
                    span: DUMMY_SP,
                    sym: "Button".into(),
                    optional: false,
                    ctxt: Default::default(),
                }),
                exported: None,
                is_type_only: false,
            })],
            src: Some(Box::new(Str {
                span: DUMMY_SP,
                value: "./components/Button".into(),
                raw: None,
            })),
            type_only: false,
            with: None,
        });

        module.body.push(ModuleItem::ModuleDecl(named_export));

        // Analyze the barrel file
        let result = analyze_barrel_file(&module, "/path/to/barrel/index.ts");
        assert!(result.is_ok());

        let re_exports = result.unwrap();
        assert_eq!(re_exports.len(), 1);
        assert_eq!(re_exports[0].exported_name, "Button");
        assert_eq!(re_exports[0].source_path, "./components/Button");
        assert_eq!(re_exports[0].original_name, "Button");
        assert!(!re_exports[0].is_default);

        // Create a barrel file AST with renamed exports
        let mut module = Module {
            span: DUMMY_SP,
            body: vec![],
            shebang: None,
        };

        // Add a renamed export
        let renamed_export = ModuleDecl::ExportNamed(NamedExport {
            span: DUMMY_SP,
            specifiers: vec![ExportSpecifier::Named(ExportNamedSpecifier {
                span: DUMMY_SP,
                orig: ModuleExportName::Ident(Ident {
                    span: DUMMY_SP,
                    sym: "Button".into(),
                    optional: false,
                    ctxt: Default::default(),
                }),
                exported: Some(ModuleExportName::Ident(Ident {
                    span: DUMMY_SP,
                    sym: "CustomButton".into(),
                    optional: false,
                    ctxt: Default::default(),
                })),
                is_type_only: false,
            })],
            src: Some(Box::new(Str {
                span: DUMMY_SP,
                value: "./components/Button".into(),
                raw: None,
            })),
            type_only: false,
            with: None,
        });

        module.body.push(ModuleItem::ModuleDecl(renamed_export));

        // Analyze the barrel file
        let result = analyze_barrel_file(&module, "/path/to/barrel/index.ts");
        assert!(result.is_ok());

        let re_exports = result.unwrap();
        assert_eq!(re_exports.len(), 1);
        assert_eq!(re_exports[0].exported_name, "CustomButton");
        assert_eq!(re_exports[0].source_path, "./components/Button");
        assert_eq!(re_exports[0].original_name, "Button");
        assert!(!re_exports[0].is_default);

        // Create a barrel file AST with a default export
        let mut module = Module {
            span: DUMMY_SP,
            body: vec![],
            shebang: None,
        };

        // Add a default export
        let default_export = ModuleDecl::ExportNamed(NamedExport {
            span: DUMMY_SP,
            specifiers: vec![ExportSpecifier::Default(
                swc_core::ecma::ast::ExportDefaultSpecifier {
                    exported: Ident {
                        span: DUMMY_SP,
                        sym: "Button".into(),
                        optional: false,
                        ctxt: Default::default(),
                    },
                },
            )],
            src: Some(Box::new(Str {
                span: DUMMY_SP,
                value: "./components/Button".into(),
                raw: None,
            })),
            type_only: false,
            with: None,
        });

        module.body.push(ModuleItem::ModuleDecl(default_export));

        // Analyze the barrel file
        let result = analyze_barrel_file(&module, "/path/to/barrel/index.ts");
        assert!(result.is_ok());

        let re_exports = result.unwrap();
        assert_eq!(re_exports.len(), 1);
        assert_eq!(re_exports[0].exported_name, "Button");
        assert_eq!(re_exports[0].source_path, "./components/Button");
        assert_eq!(re_exports[0].original_name, "default");
        assert!(re_exports[0].is_default);
    }
}
