use std::path::Path;
use swc_core::ecma::ast::{ImportDecl, ImportSpecifier, ModuleDecl, ModuleItem};
use swc_core::ecma::visit::{noop_visit_mut_type, VisitMut, VisitMutWith};

use crate::alias_resolver::AliasResolver;
use crate::config::Config;
use crate::import_transformer::transform_import;
use crate::path_resolver::PathResolver;
use crate::paths::{dirname, path_join};
use crate::pattern_matcher::CompiledPattern;

/// Visitor for transforming barrel file imports
pub struct BarrelTransformVisitor {
    /// Virtual path to the directory for the current file
    source_dir: String,

    /// Resolver for import aliases
    alias_resolver: AliasResolver,

    /// Resolver for file paths
    path_resolver: PathResolver,

    /// Pre-compiled patterns for barrel files
    compiled_patterns: Vec<CompiledPattern>,

    /// Enable debug logging
    debug: bool,

    /// Plugin configuration
    config: Config,
}

fn log(message: String) {
    println!("[swc-plugin-barrel-files] {}", message);
}

impl BarrelTransformVisitor {
    /// Creates a new visitor with the specified configuration
    pub fn new(config: &Config, cwd: String, filename: String) -> Result<Option<Self>, String> {
        let path_resolver = PathResolver::new(&cwd);

        let compiled_patterns = Self::compile_patterns(&cwd, config, &path_resolver)?;

        // Normalize absolute path to the source file
        // swc/loader and swc/jest pass full `filepath`
        // swc/cli pass relative `filepath`
        let source_file_path = path_join(&cwd, &filename);

        // Cannot process files outside cwd due to WASM restrictions
        if !Path::new(&source_file_path).starts_with(&cwd) {
            if config.debug.unwrap_or_default() {
                log(format!(
                    "Skipping {} (reason: outside cwd)",
                    source_file_path
                ));
            }
            return Ok(None);
        }

        let source_file_virtual_path = path_resolver.to_virtual_path(&source_file_path)?;
        let source_dir = dirname(&source_file_virtual_path);

        let alias_resolver = AliasResolver::new(
            &config.aliases,
            &path_resolver,
            &cwd,
            &source_file_virtual_path,
        )?;

        let visitor = Self {
            source_dir,
            alias_resolver,
            path_resolver,
            compiled_patterns,
            debug: config.debug.unwrap_or_default(),
            config: config.to_owned(),
        };

        visitor.log(format!("Parsing {}", source_file_virtual_path));

        Ok(Some(visitor))
    }

    fn compile_patterns(
        cwd: &str,
        config: &Config,
        path_resolver: &PathResolver,
    ) -> Result<Vec<CompiledPattern>, String> {
        let mut compiled_patterns = Vec::new();

        for pattern in &config.patterns {
            let joined_path = path_join(cwd, pattern);
            let virtual_path = path_resolver.to_virtual_path(&joined_path)?;

            let compiled_pattern = CompiledPattern::new(&virtual_path)
                .map_err(|e| format!("Failed to compile pattern '{}': {}", virtual_path, e))?;

            compiled_patterns.push(compiled_pattern);
        }

        Ok(compiled_patterns)
    }

    fn process_import(&self, import_decl: &ImportDecl) -> Result<Option<Vec<ImportDecl>>, String> {
        let import_path_atom = import_decl.src.value.to_atom_lossy();
        let import_path = import_path_atom.as_str();

        let barrel_file = if !import_path.starts_with('.') && !Path::new(import_path).is_absolute()
        {
            self.resolve_aliased_import(import_path)?
        } else {
            self.resolve_local_import(import_path)?
        };

        if let Some(barrel_file) = barrel_file {
            self.transform_import(import_decl, &barrel_file)
        } else {
            Ok(None)
        }
    }

    fn resolve_aliased_import(&self, import_path: &str) -> Result<Option<String>, String> {
        match self.alias_resolver.resolve(import_path)? {
            Some(resolved_path) => {
                self.log(format!(
                    "    alias \"{}\" resolved to {}",
                    import_path, resolved_path
                ));

                if !self.match_pattern(&resolved_path) {
                    self.log(format!("    not matched by patterns: {}", resolved_path));
                    return Ok(None);
                }

                Ok(Some(resolved_path))
            }
            None => {
                self.log(format!("    import \"{}\" was not resolved", import_path));

                Ok(None)
            }
        }
    }

    fn resolve_local_import(&self, import_path: &str) -> Result<Option<String>, String> {
        let import_path = if import_path.starts_with(".") {
            path_join(&self.source_dir, import_path)
        } else {
            import_path.into()
        };

        let barrel_file = match self.path_resolver.to_virtual_path(&import_path) {
            Ok(resolved_path) => resolved_path,
            Err(_) => {
                self.log(format!(
                    "    file cannot be processed: {} (reason: outside cwd)",
                    import_path
                ));
                return Ok(None);
            }
        };

        if !self.match_pattern(&barrel_file) {
            self.log(format!("    not matched by patterns: {}", barrel_file));
            return Ok(None);
        }

        Ok(Some(barrel_file))
    }

    fn transform_import(
        &self,
        import_decl: &ImportDecl,
        barrel_file: &str,
    ) -> Result<Option<Vec<ImportDecl>>, String> {
        self.log(format!("    found barrel file: {}", barrel_file));

        let new_imports =
            transform_import(&self.source_dir, import_decl, barrel_file, &self.config)?;

        if let Some(new_imports) = new_imports {
            if self.debug {
                self.log("    replacing with:".into());

                for new_import in new_imports.iter() {
                    let source = &new_import.src.value;
                    for specifier in &new_import.specifiers {
                        let specifier_name = match specifier {
                            ImportSpecifier::Named(named) => &named.local.sym,
                            ImportSpecifier::Default(default) => &default.local.sym,
                            ImportSpecifier::Namespace(namespace) => &namespace.local.sym,
                        };
                        self.log(format!(
                            "        import {{ {} }} from \"{}\"",
                            specifier_name,
                            source.to_atom_lossy()
                        ));
                    }
                }
            }

            Ok(Some(new_imports))
        } else {
            Ok(None)
        }
    }

    /// Matches an import path against the configured patterns using pre-compiled patterns
    ///
    /// # Arguments
    ///
    /// * `import_path` - The import path to match
    ///
    /// # Returns
    ///
    /// `true` if any pattern matches, `false` otherwise
    fn match_pattern(&self, import_path: &str) -> bool {
        self.compiled_patterns
            .iter()
            .any(|compiled_pattern| compiled_pattern.matches(import_path))
    }

    fn log(&self, message: String) {
        if self.debug {
            log(message);
        }
    }
}

impl VisitMut for BarrelTransformVisitor {
    // A comprehensive list of possible visitor methods can be found here:
    // https://rustdoc.swc.rs/swc_ecma_visit/trait.VisitMut.html
    noop_visit_mut_type!();

    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        items.visit_mut_children_with(self);

        let mut result = Vec::with_capacity(items.len());

        for item in items.drain(..) {
            if let ModuleItem::ModuleDecl(ModuleDecl::Import(import_decl)) = &item {
                match self.process_import(import_decl) {
                    Ok(Some(new_imports)) => {
                        result.extend(
                            new_imports
                                .into_iter()
                                .map(|import| ModuleItem::ModuleDecl(ModuleDecl::Import(import))),
                        );
                        continue;
                    }
                    Ok(None) => {}
                    Err(err) => {
                        swc_core::plugin::errors::HANDLER.with(|handler| {
                            handler
                                .struct_span_err(
                                    import_decl.span,
                                    &format!("Error processing barrel import: {}", err),
                                )
                                .emit()
                        });
                    }
                }
            }

            result.push(item);
        }

        *items = result;
    }
}
