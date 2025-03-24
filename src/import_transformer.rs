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
                    "E_NO_NAMESPACE_IMPORTS: Namespace imports are not supported for barrel file optimization".to_string(),
                );
            }
        }
    }

    // Check if any imports were not found in the barrel file
    if !missing_exports.is_empty() {
        return Err(format!(
            "E_UNRESOLVED_EXPORTS: The following exports were not found in the barrel file {}: {}",
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
        Err(e) => return Err(format!("E_FILE_READ: Failed to load file: {}", e)),
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
        Err(e) => Err(format!("E_FILE_PARSE: Failed to parse file: {:?}", e)),
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
    let ast = parse_file(file_path)?;

    match analyze_barrel_file(&ast, file_path) {
        Ok(re_exports) => {
            if re_exports.is_empty() {
                return Err(format!(
                    "E_UNRESOLVED_EXPORTS: No re-exports found in barrel file: {}",
                    file_path
                ));
            }
            Ok(re_exports)
        }
        Err(e) => Err(format!(
            "E_INVALID_BARREL_FILE: Invalid barrel file {}: {}",
            file_path, e
        )),
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
) -> Result<Vec<ImportDecl>, String> {
    let import_source = import_decl.src.value.to_string();
    let barrel_file = match resolve_barrel_file(cwd, &import_source, pattern, paths) {
        Ok(res) => match res {
            Some(path) => path,
            None => {
                return Err(format!(
                    "E_BARREL_FILE_NOT_FOUND: Could not resolve barrel file for import from {}",
                    import_source,
                ));
            }
        },
        Err(e) => {
            return Err(format!(
                "E_BARREL_FILE_NOT_FOUND: Could not resolve barrel file for {}: {}",
                import_source, e
            ));
        }
    };

    let re_exports = match parse_barrel_file_exports(&barrel_file) {
        Ok(exports) => exports,
        Err(e) => {
            return Err(format!(
                "E_INVALID_BARREL_FILE: Error analyzing barrel file {}: {}",
                barrel_file, e
            ));
        }
    };

    let source_file = path_join(cwd, filename);
    let source_file = match resolve_to_virtual_path(cwd, &source_file) {
        Ok(path) => path,
        Err(e) => {
            return Err(format!(
                "E_SOURCE_FILE_NOT_FOUND: Error resolving source file {}: {}",
                source_file, e
            ))
        }
    };
    let source_dir = dirname(&source_file);

    match transform_import(&source_dir, import_decl, &barrel_file, &re_exports) {
        Ok(new_imports) => Ok(new_imports),
        Err(e) => Err(format!(
            "E_TRANSFORM_FAILED: Error transforming import from {}: {}",
            import_source, e
        )),
    }
}
