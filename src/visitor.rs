use std::collections::HashMap;
use std::path::Path;
use swc_core::ecma::ast::{ImportDecl, Module, ModuleItem};
use swc_core::ecma::visit::{VisitMut, VisitMutWith};

use crate::alias_resolver::AliasResolver;
use crate::config::Config;
use crate::import_transformer::transform_import;
use crate::paths::{dirname, path_join, to_virtual_path};
use crate::pattern_matcher::path_matches_pattern;

/// Visitor for transforming barrel file imports
pub struct BarrelTransformVisitor {
    /// Compilation working directory
    cwd: String,

    /// Virtual path to the directory for the current file
    source_dir: String,

    /// Map of import declarations to their replacements
    /// The key is the span of the original import, and the value is a vector of replacement imports
    import_replacements: HashMap<u32, Vec<ImportDecl>>,

    /// Resolver for import aliases
    alias_resolver: AliasResolver,

    /// Patterns for barrel files
    patterns: Vec<String>,
}

impl BarrelTransformVisitor {
    /// Creates a new visitor with the specified configuration
    pub fn new(config: &Config, cwd: String, filename: String) -> Result<Option<Self>, String> {
        // Transform patterns to virtual paths
        let mut patterns = Vec::new();
        for pattern in &config.patterns {
            let joined_path = path_join(&cwd, pattern);
            let virtual_path = to_virtual_path(&cwd, &joined_path)?;
            patterns.push(virtual_path);
        }

        // Normalize absolute path to the source file
        // swc/loader and swc/jest pass full `filepath`
        // swc/cli pass relative `filepath`
        let source_file_path = path_join(&cwd, &filename);

        // Cannot process files outside cwd due to WASM restrictions
        if !source_file_path.starts_with(&cwd) {
            return Ok(None);
        }

        let source_file_virtual_path = to_virtual_path(&cwd, &source_file_path)?;
        let source_dir = dirname(&source_file_virtual_path);

        let alias_resolver = AliasResolver::new(config, &cwd, &source_file_virtual_path)?;

        Ok(Some(BarrelTransformVisitor {
            cwd,
            source_dir,
            import_replacements: HashMap::new(),
            alias_resolver,
            patterns,
        }))
    }

    fn process_import(&self, import_decl: &ImportDecl) -> Result<Option<Vec<ImportDecl>>, String> {
        let import_path = import_decl.src.value.to_string();
        let barrel_file = if import_path.starts_with(".") {
            path_join(&self.source_dir, &import_path)
        } else if Path::new(&import_path).is_absolute() {
            match to_virtual_path(&self.cwd, &import_path) {
                Ok(resolved_path) => resolved_path,
                Err(_) => return Ok(None),
            }
        } else {
            match self.alias_resolver.resolve(&import_path)? {
                Some(resolved_path) => resolved_path,
                None => {
                    return Ok(None);
                }
            }
        };

        if !self.match_pattern(&barrel_file) {
            return Ok(None);
        }

        let new_imports = transform_import(&self.source_dir, import_decl, &barrel_file)?;

        Ok(Some(new_imports))
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
    fn match_pattern(&self, import_path: &str) -> bool {
        self.patterns
            .iter()
            .any(|pattern| path_matches_pattern(import_path, pattern))
    }
}

impl VisitMut for BarrelTransformVisitor {
    fn visit_mut_module(&mut self, module: &mut Module) {
        module.visit_mut_children_with(self);
    }

    fn visit_mut_import_decl(&mut self, import_decl: &mut ImportDecl) {
        match self.process_import(import_decl) {
            Ok(Some(new_imports)) => {
                if !new_imports.is_empty() {
                    // Store the span of the original import as a key
                    // We'll use this to identify the import in visit_mut_module_items
                    let span_lo = import_decl.span.lo.0;

                    self.import_replacements
                        .insert(span_lo, new_imports.clone());
                }
            }
            Ok(None) => {}
            Err(err) => {
                let handler = &swc_core::plugin::errors::HANDLER;
                handler.with(|handler| {
                    handler
                        .struct_span_err(
                            import_decl.span,
                            &format!("Error processing barrel import: {}", err),
                        )
                        .emit()
                });
            }
        }

        import_decl.visit_mut_children_with(self);
    }

    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        // First, visit all items to collect replacements
        for item in items.iter_mut() {
            item.visit_mut_with(self);
        }

        // Collect all the changes we need to make
        let mut changes = Vec::new();

        for (i, item) in items.iter().enumerate() {
            if let ModuleItem::ModuleDecl(swc_core::ecma::ast::ModuleDecl::Import(import)) = item {
                if let Some(mut replacements) =
                    self.import_replacements.get(&import.span.lo.0).cloned()
                {
                    replacements
                        .sort_by(|a, b| a.src.value.to_string().cmp(&b.src.value.to_string()));
                    changes.push((i, replacements));
                }
            }
        }

        // Apply all changes, starting from the end to avoid invalidating indices
        for (index, replacements) in changes.into_iter().rev() {
            // Remove the original import
            items.remove(index);

            // Insert all replacements at the position of the removed import
            let mut insert_pos = index;

            for import in replacements.iter() {
                items.insert(
                    insert_pos,
                    ModuleItem::ModuleDecl(swc_core::ecma::ast::ModuleDecl::Import(import.clone())),
                );
                insert_pos += 1;
            }
        }
    }
}
