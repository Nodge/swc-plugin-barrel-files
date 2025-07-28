//! SWC Plugin for Barrel Files
//!
//! This plugin transforms imports from barrel files (index.ts) into direct imports
//! from the source files. This helps to avoid circular dependencies and improves tree-shaking.

mod alias_resolver;
mod config;
mod import_transformer;
mod path_resolver;
mod paths;
mod pattern_matcher;
mod re_export;
mod visitor;

use swc_core::ecma::ast::Program;
use swc_core::ecma::visit::{as_folder, FoldWith};
use swc_core::plugin::metadata::TransformPluginMetadataContextKind;
use swc_core::plugin::{plugin_transform, proxies::TransformPluginProgramMetadata};

use config::Config;
use visitor::BarrelTransformVisitor;

/// SWC plugin transform entry point
///
/// This function is called by SWC to transform the AST.
#[plugin_transform]
pub fn process_transform(program: Program, metadata: TransformPluginProgramMetadata) -> Program {
    let cwd = metadata
        .get_context(&TransformPluginMetadataContextKind::Cwd)
        .expect("E_INVALID_ENV: Current working directory is not available");

    let filename = metadata
        .get_context(&TransformPluginMetadataContextKind::Filename)
        .expect("E_INVALID_ENV: Current filename is not available");

    let config: Config = serde_json::from_str(
        &metadata
            .get_transform_plugin_config()
            .unwrap_or_else(|| "{}".to_string()),
    )
    .expect("E_INVALID_CONFIG: Error parsing barrel plugin configuration");

    let visitor =
        BarrelTransformVisitor::new(&config, cwd, filename).expect("Error creating visitor");

    match visitor {
        Some(visitor) => program.fold_with(&mut as_folder(visitor)),
        None => program,
    }
}
