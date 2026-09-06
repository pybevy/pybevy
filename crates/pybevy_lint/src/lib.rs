pub mod bevy_audit;
pub mod bevy_parser;
pub mod comparison;
pub mod config;
pub mod config_audit;
pub mod mappings;
pub mod model;
pub mod output;
pub mod python_parser;
pub mod rust_parser;
pub mod test_coverage;
pub mod validation;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use anyhow::Result;
pub use bevy_parser::{BevyCrate, merge_reexported_types};
pub use comparison::{ComparisonResult, CoverageReport, compare_apis, pybevy_bevy_name};
pub use config::Config;
use model::PyClassDef;
use output::Diagnostic;

/// Parse re-export source crates (per `bevy.crate_type_sources`) and merge the
/// types PyBevy wraps into the parsed Bevy crates so the comparison sees them.
pub fn merge_reexported_bevy_types(
    bevy_crates: &mut HashMap<String, BevyCrate>,
    bevy_path: &Path,
    config: &crate::config::BevyConfig,
    pybevy_classes: &[PyClassDef],
    use_cache: bool,
) {
    let wanted: HashSet<String> = pybevy_classes
        .iter()
        .map(|c| pybevy_bevy_name(c, config))
        .collect();
    merge_reexported_types(
        bevy_crates,
        bevy_path,
        &config.crate_type_sources,
        &wanted,
        use_cache,
    );
}

/// Parse all Rust files in the given directory and extract PyClass definitions
pub fn parse_rust_files(path: &Path) -> Result<Vec<PyClassDef>> {
    rust_parser::parse_directory(path)
}

/// Parse all Python stub files in the given directory
pub fn parse_python_stubs(path: &Path) -> Result<Vec<PyClassDef>> {
    python_parser::parse_directory(path)
}

/// Parse every public module-level symbol declared by the Python stubs.
pub fn parse_python_stub_symbols(
    path: &Path,
) -> Result<Vec<python_parser::module_symbols::StubSymbolDef>> {
    python_parser::module_symbols::parse_symbol_directory(path)
}

/// Parse the reviewed wildcard export map from a Python prelude module.
pub fn parse_python_reexports(
    path: &Path,
) -> Result<Vec<python_parser::module_symbols::StubReexport>> {
    python_parser::module_symbols::parse_reexports(path)
}

/// Validate Rust definitions against Python stubs
pub fn validate(rust_classes: &[PyClassDef], python_classes: &[PyClassDef]) -> Vec<Diagnostic> {
    validation::validate_all(rust_classes, python_classes, None)
}

/// Validate Rust definitions against Python stubs with config
pub fn validate_with_config(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: &Config,
) -> Vec<Diagnostic> {
    validation::validate_all(rust_classes, python_classes, Some(config))
}

/// Validate a caller-filtered subset without auditing unrelated exceptions.
pub fn validate_scoped_with_config(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: &Config,
) -> Vec<Diagnostic> {
    validation::validate_scoped(rust_classes, python_classes, Some(config))
}

/// Parse Bevy crates from the given Bevy source path (with caching enabled by default)
pub fn parse_bevy_crates(
    bevy_path: &Path,
    crate_names: &[&str],
) -> Result<HashMap<String, BevyCrate>> {
    bevy_parser::parse_bevy_crates(bevy_path, crate_names)
}

/// Parse Bevy crates with optional caching
pub fn parse_bevy_crates_with_cache(
    bevy_path: &Path,
    crate_names: &[&str],
    use_cache: bool,
) -> Result<HashMap<String, BevyCrate>> {
    bevy_parser::parse_bevy_crates_with_cache(bevy_path, crate_names, use_cache)
}

/// Compare PyBevy API against Bevy API
pub fn compare_with_bevy(
    pybevy_classes: &[PyClassDef],
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &config::Config,
) -> ComparisonResult {
    comparison::compare_apis(pybevy_classes, bevy_crates, &config.bevy)
}
