use crate::config::{Config, InvalidBarrelMode, UnsupportedImportMode};
use crate::paths::{dirname, path_join, resolve_relative_path};
use crate::re_export::{analyze_barrel_file, ReExport};
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{LazyLock, Mutex};
use swc_core::common::sync::Lrc;
use swc_core::common::SourceMap;
use swc_core::common::DUMMY_SP;
use swc_core::ecma::ast::Module;
use swc_core::ecma::ast::{
    ImportDecl, ImportDefaultSpecifier, ImportNamedSpecifier, ImportSpecifier, ModuleExportName,
    Str,
};
use swc_core::ecma::parser::{parse_file_as_module, Syntax, TsSyntax};

/// Cache for parsed barrel files to avoid re-parsing the same file
static BARREL_CACHE: LazyLock<Mutex<HashMap<String, Option<Vec<ReExport>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

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

/// Creates a named import specifier for a non-default re-export
fn create_named_specifier(
    span: swc_core::common::Span,
    local_name: &swc_core::ecma::ast::Ident,
    re_export: &ReExport,
    is_type_only: bool,
) -> ImportSpecifier {
    ImportSpecifier::Named(ImportNamedSpecifier {
        span,
        local: local_name.clone(),
        // The `as` clause is needed when either the barrel or the consumer renamed the export
        imported: (local_name.sym != re_export.original_name).then(|| {
            ModuleExportName::Ident(swc_core::ecma::ast::Ident {
                span: DUMMY_SP,
                sym: re_export.original_name.clone().into(),
                optional: false,
                ctxt: Default::default(),
            })
        }),
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
            ModuleExportName::Str(str) => str.value.to_atom_lossy().to_string(),
        })
        .unwrap_or_else(|| named.local.sym.to_string())
}

/// A local binding requested from the barrel file
struct RequestedBinding<'a> {
    span: swc_core::common::Span,
    local: &'a swc_core::ecma::ast::Ident,
    is_type_only: bool,
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
/// New import declarations that directly import from the original source files,
/// or `None` if the import should stay unchanged
pub fn transform_import(
    source_dir: &str,
    import_decl: &ImportDecl,
    barrel_file: &str,
    config: &Config,
) -> Result<Option<Vec<ImportDecl>>, String> {
    if import_decl.specifiers.is_empty() {
        return Ok(None);
    }

    let Some(re_exports) = parse_barrel_file_exports(barrel_file, config)? else {
        return Ok(None);
    };

    // Bindings grouped by the barrel export they read, in declaration order
    let mut requested: IndexMap<String, Vec<RequestedBinding>> = IndexMap::new();

    for specifier in &import_decl.specifiers {
        let (exported_name, binding) = match specifier {
            ImportSpecifier::Named(named) => (
                extract_imported_name(named),
                RequestedBinding {
                    span: named.span,
                    local: &named.local,
                    is_type_only: named.is_type_only,
                },
            ),
            ImportSpecifier::Default(default) => (
                "default".to_string(),
                RequestedBinding {
                    span: default.span,
                    local: &default.local,
                    is_type_only: false,
                },
            ),
            // A namespace binding needs the whole barrel, so the declaration is left as is
            ImportSpecifier::Namespace(_) => {
                return match config.unsupported_import_mode {
                    UnsupportedImportMode::Error => Err(
                        "E_NO_NAMESPACE_IMPORTS: Namespace imports are not supported for barrel file optimization".to_string(),
                    ),
                    UnsupportedImportMode::Warn => {
                        eprintln!("Warning: Namespace imports are not supported for barrel file optimization. Import from {} will be skipped.", import_decl.src.value.to_atom_lossy());
                        Ok(None)
                    }
                    UnsupportedImportMode::Off => Ok(None),
                };
            }
        };

        requested.entry(exported_name).or_default().push(binding);
    }

    let exported_names: HashSet<&str> = re_exports
        .iter()
        .map(|re_export| re_export.exported_name.as_str())
        .collect();

    let missing_exports: Vec<&str> = requested
        .keys()
        .map(String::as_str)
        .filter(|name| !exported_names.contains(name))
        .collect();

    if !missing_exports.is_empty() {
        return Err(format!(
            "E_UNRESOLVED_EXPORTS: The following exports were not found in the barrel file {}: {}",
            barrel_file,
            missing_exports.join(", ")
        ));
    }

    let barrel_file_dir = dirname(barrel_file);
    let mut new_imports: IndexMap<String, Vec<ImportSpecifier>> = IndexMap::new();

    // Iterate through re_exports in barrel file order to preserve order
    for re_export in &re_exports {
        let Some(bindings) = requested.get(&re_export.exported_name) else {
            continue;
        };

        let import_path = resolve_import_path(&barrel_file_dir, source_dir, re_export);
        let specifiers = new_imports.entry(import_path).or_default();

        for binding in bindings {
            specifiers.push(if re_export.is_default {
                create_default_specifier(binding.span, binding.local)
            } else {
                create_named_specifier(binding.span, binding.local, re_export, binding.is_type_only)
            });
        }
    }

    if new_imports.is_empty() {
        return Ok(None);
    }

    let result = new_imports
        .into_iter()
        .map(|(source_path, specifiers)| ImportDecl {
            span: import_decl.span,
            specifiers,
            src: Box::new(Str {
                span: DUMMY_SP,
                value: source_path.into(),
                raw: None,
            }),
            type_only: import_decl.type_only,
            with: import_decl.with.clone(),
            phase: Default::default(),
        })
        .collect();

    Ok(Some(result))
}

/// Parses a file into an AST
fn parse_file(file_path: &str) -> Result<Module, String> {
    let cm: Lrc<SourceMap> = Default::default();

    let fm = match cm.load_file(Path::new(file_path)) {
        Ok(fm) => fm,
        Err(e) => return Err(format!("E_FILE_READ: Failed to load file: {}", e)),
    };

    let syntax = Syntax::Typescript(TsSyntax {
        tsx: file_path.ends_with(".tsx"),
        ..Default::default()
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

    match analyze_barrel_file(&ast) {
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
