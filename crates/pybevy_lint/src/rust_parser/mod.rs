mod conversions;
mod macros;
mod pyclass;
mod pymethods;
mod signature;
pub mod types;

use std::path::Path;

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::model::PyClassDef;

fn is_project_artifact_directory(entry: &walkdir::DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    if entry.file_name().to_str() == Some("target") {
        return true;
    }
    // No PyBevy source lives in a dot directory, and several hold Rust that is
    // not ours: a repo-local CARGO_HOME carries the whole registry under
    // .cache/registry/src, and parsing third-party crates there fails the run.
    if entry.depth() > 0
        && entry
            .file_name()
            .to_str()
            .is_some_and(|n| n.starts_with('.'))
    {
        return true;
    }
    entry.depth() > 0 && entry.path().join(".git").exists()
}

/// Parse all Rust files in a directory and extract PyClass definitions
pub fn parse_directory(path: &Path) -> Result<Vec<PyClassDef>> {
    let mut classes = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_entry(|entry| !is_project_artifact_directory(entry))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let file_path = entry.path();
        let module_path = extract_module_path(file_path, path);
        let mut file_classes = parse_file(file_path)
            .with_context(|| format!("Failed to parse {}", file_path.display()))?;

        for class in &mut file_classes {
            class.module_path = module_path.clone();
        }

        classes.extend(file_classes);
    }

    Ok(classes)
}

/// Extract module path from file path relative to base
/// e.g., src/camera/scaling_mode.rs -> "camera"
/// e.g., src/input/keyboard/mod.rs -> "input.keyboard"
/// When base_path is a subdirectory (e.g., src/camera/), uses the base directory name
fn extract_module_path(file_path: &Path, base_path: &Path) -> Option<String> {
    let relative = match file_path.strip_prefix(base_path) {
        Ok(relative) => relative,
        Err(_) if base_path == Path::new(".") => file_path,
        Err(_) => return None,
    };
    let components: Vec<_> = relative
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    if components.is_empty() {
        return None;
    }

    // Repository-wide validation starts at the workspace root. Normalize the
    // two workspace layouts to the Python module before considering internal
    // Rust source directories, which are not part of the public module path.
    if let ["crates", crate_name, "src", ..] = components.as_slice()
        && let Some(module) = crate_name.strip_prefix("pybevy_")
    {
        return Some(module.to_string());
    }
    if let ["src", module, ..] = components.as_slice() {
        return Some((*module).to_string());
    }

    // Remove the file name and extension, keep directory structure
    // src/camera/scaling_mode.rs -> ["camera", "scaling_mode.rs"] -> "camera"
    // src/input/keyboard/mod.rs -> ["input", "keyboard", "mod.rs"] -> "input.keyboard"
    let path_parts: Vec<&str> = components[..components.len() - 1].to_vec();

    if path_parts.is_empty() {
        // File is directly in base_path (e.g., src/camera/scaling_mode.rs with base src/camera/)
        // Use the last component of base_path as the module
        // e.g., src/camera/ -> "camera"
        base_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    } else {
        Some(path_parts.join("."))
    }
}

/// Parse a single Rust file and extract PyClass definitions.
pub fn parse_file(path: &Path) -> Result<Vec<PyClassDef>> {
    let content = std::fs::read_to_string(path)?;
    let syntax = syn::parse_file(&content)?;

    let mut classes = Vec::new();

    // First pass: collect all pyclass definitions (structs and enums)
    for item in &syntax.items {
        match item {
            syn::Item::Struct(item_struct) => {
                if let Some(class) = pyclass::parse_struct(item_struct, path) {
                    classes.push(class);
                }
            }
            syn::Item::Enum(item_enum) => {
                if let Some(class) = pyclass::parse_enum(item_enum, path) {
                    classes.push(class);
                }
            }
            _ => {}
        }
    }

    // Second pass: collect pymethods for each class
    for item in &syntax.items {
        if let syn::Item::Impl(item_impl) = item {
            pymethods::process_impl_block(item_impl, &mut classes, path);
            conversions::process_conversion_impl(item_impl, &mut classes, path);
        }
    }

    Ok(classes)
}
