use crate::config::{Config, InvalidBarrelMode, UnsupportedImportMode};
use crate::paths::{dirname, path_join, resolve_relative_path};
use crate::re_export::{analyze_barrel_file, ReExport};
use indexmap::IndexMap;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;
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
use swc_core::ecma::parser::{parse_file_as_module, Syntax};

/// Cache for parsed barrel files to avoid re-parsing the same file
static BARREL_CACHE: Lazy<Mutex<HashMap<String, Option<Vec<ReExport>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Resolves the import path from the barrel file directory and re-export source path
fn resolve_import_path(barrel_file_dir: &str, source_dir: &str, re_export: &ReExport) -> String {
    if !re_export.source_path.starts_with('.') {
        return re_export.source_path.clone();
    }

    let target_path = path_join(barrel_file_dir, &re_export.source_path);
    resolve_relative_path(source_dir, &target_path).unwrap()
}

/// Creates a default import specifier
fn create_default_specifier(
    span: swc_core::common::Span,
    local_name: &swc_core::ecma::ast::Ident,
) -> ImportSpecifier {
    ImportSpecifier::Default(ImportDefaultSpecifier {
        span,
        local: local_name.clone(),
    })
}

/// Creates a named import specifier
fn create_named_specifier(
    span: swc_core::common::Span,
    local_name: &swc_core::ecma::ast::Ident,
    re_export: &ReExport,
    is_type_only: bool,
) -> ImportSpecifier {
    ImportSpecifier::Named(ImportNamedSpecifier {
        span,
        local: local_name.clone(),
        imported: if !re_export.is_default {
            // For named exports, check if we need to add the 'as' clause
            if local_name.sym != re_export.original_name {
                // Only add the 'as' clause when the original name is different from the local name
                // This handles both cases:
                // 1. When the export was renamed in the barrel file (setVisible as toggle)
                // 2. When the import is renamed in the consumer file (toggle as switcher)
                Some(ModuleExportName::Ident(swc_core::ecma::ast::Ident {
                    span: DUMMY_SP,
                    sym: re_export.original_name.clone().into(),
                    optional: false,
                }))
            } else {
                // If the original name is the same as the local name, don't add the 'as' clause
                None
            }
        } else {
            // For default exports, we don't need to specify the imported name
            None
        },
        is_type_only,
    })
}

/// Extracts the imported name from a named import specifier
fn extract_imported_name(named: &ImportNamedSpecifier) -> String {
    named
        .imported
        .as_ref()
        .map(|name| match name {
            ModuleExportName::Ident(ident) => ident.sym.to_string(),
            ModuleExportName::Str(str) => str.value.to_string(),
        })
        .unwrap_or_else(|| named.local.sym.to_string())
}

/// Transforms an import declaration by replacing barrel imports with direct imports
///
/// # Arguments
///
/// * `source_dir` - The directory containing the current source file
/// * `import_decl` - The import declaration to transform
/// * `barrel_file` - The path to the barrel file
/// * `config` - The plugin configuration
///
/// # Returns
///
/// A vector of new import declarations that directly import from the original source files
pub fn transform_import(
    source_dir: &str,
    import_decl: &ImportDecl,
    barrel_file: &str,
    config: &Config,
) -> Result<Option<Vec<ImportDecl>>, String> {
    let mut new_imports: IndexMap<String, Vec<ImportSpecifier>> = IndexMap::new();
    let mut missing_exports = Vec::new();

    let barrel_file_dir = dirname(barrel_file);

    let re_exports = parse_barrel_file_exports(barrel_file, config)?;

    if let Some(re_exports) = re_exports {
        let mut import_specifiers_map = HashMap::new();

        for specifier in &import_decl.specifiers {
            match specifier {
                ImportSpecifier::Named(named) => {
                    let imported_name = extract_imported_name(named);
                    import_specifiers_map.insert(imported_name, specifier);
                }
                ImportSpecifier::Default(_) => {
                    import_specifiers_map.insert("default".to_string(), specifier);
                }
                ImportSpecifier::Namespace(_) => match config.unsupported_import_mode {
                    UnsupportedImportMode::Error => {
                        return Err(
                            "E_NO_NAMESPACE_IMPORTS: Namespace imports are not supported for barrel file optimization".to_string(),
                        );
                    }
                    UnsupportedImportMode::Warn => {
                        eprintln!("Warning: Namespace imports are not supported for barrel file optimization. Import from {} will be skipped.", import_decl.src.value);
                    }
                    UnsupportedImportMode::Off => {}
                },
            }
        }

        let mut found_exports = HashSet::new();
        let mut has_default_export = false;

        // Iterate through re_exports in barrel file order to preserve order
        for re_export in &re_exports {
            found_exports.insert(&re_export.exported_name);
            if re_export.is_default {
                has_default_export = true;
            }

            if let Some(specifier) = import_specifiers_map.get(&re_export.exported_name) {
                let import_path = resolve_import_path(&barrel_file_dir, source_dir, re_export);

                let new_specifier = match specifier {
                    ImportSpecifier::Named(named) => {
                        if re_export.is_default {
                            create_default_specifier(named.span, &named.local)
                        } else {
                            create_named_specifier(
                                named.span,
                                &named.local,
                                re_export,
                                named.is_type_only,
                            )
                        }
                    }
                    ImportSpecifier::Default(default) => {
                        create_default_specifier(default.span, &default.local)
                    }
                    ImportSpecifier::Namespace(_) => {
                        continue;
                    }
                };

                new_imports
                    .entry(import_path)
                    .or_default()
                    .push(new_specifier);
            }
        }

        // Check for any imported specifiers that weren't found in the barrel file
        for imported_name in import_specifiers_map.keys() {
            if imported_name == "default" {
                if !has_default_export {
                    missing_exports.push("default".to_string());
                }
            } else if !found_exports.contains(imported_name) {
                missing_exports.push(imported_name.clone());
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

        Ok(Some(result))
    } else {
        Ok(None)
    }
}

/// Parses a file into an AST
fn parse_file(file_path: &str) -> Result<Module, String> {
    let cm: Lrc<SourceMap> = Default::default();
    let _handler = Handler::with_tty_emitter(ColorConfig::Auto, true, false, Some(cm.clone()));

    let fm = match cm.load_file(Path::new(file_path)) {
        Ok(fm) => fm,
        Err(e) => return Err(format!("E_FILE_READ: Failed to load file: {}", e)),
    };

    let syntax = Syntax::Typescript(Default::default());

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
/// * `config` - The plugin configuration
///
/// # Returns
///
/// A list of re-exports if the file is a valid barrel file, `Err` otherwise
fn parse_barrel_file_exports(
    file_path: &str,
    config: &Config,
) -> Result<Option<Vec<ReExport>>, String> {
    if let Ok(cache) = BARREL_CACHE.lock() {
        if let Some(cached_exports) = cache.get(file_path) {
            return Ok(cached_exports.clone());
        }
    }

    let ast = parse_file(file_path)?;

    match analyze_barrel_file(&ast, file_path) {
        Ok(re_exports) => {
            if re_exports.is_empty() {
                return Err(format!(
                    "E_UNRESOLVED_EXPORTS: No re-exports found in barrel file: {}",
                    file_path
                ));
            }

            if let Ok(mut cache) = BARREL_CACHE.lock() {
                cache.insert(file_path.to_string(), Some(re_exports.clone()));
            }

            Ok(Some(re_exports))
        }
        Err(e) => {
            let error_msg = format!(
                "E_INVALID_BARREL_FILE: Invalid barrel file {}: {}",
                file_path, e
            );

            match config.invalid_barrel_mode {
                InvalidBarrelMode::Error => Err(error_msg),
                InvalidBarrelMode::Warn => {
                    eprintln!("Warning: {}", error_msg);
                    if let Ok(mut cache) = BARREL_CACHE.lock() {
                        cache.insert(file_path.to_string(), None);
                    }
                    Ok(None)
                }
                InvalidBarrelMode::Off => {
                    if let Ok(mut cache) = BARREL_CACHE.lock() {
                        cache.insert(file_path.to_string(), None);
                    }
                    Ok(None)
                }
            }
        }
    }
}
