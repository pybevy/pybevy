pub mod class_def;
mod method_def;
pub mod module_symbols;
pub mod types;

use std::{collections::HashSet, path::Path};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use crate::model::PyClassDef;

/// Parse all Python stub files in a directory
pub fn parse_directory(path: &Path) -> Result<Vec<PyClassDef>> {
    let mut classes = Vec::new();

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "pyi"))
    {
        let file_path = entry.path();
        let module_path = extract_module_path(file_path, path);
        let mut file_classes = parse_file(file_path)
            .with_context(|| format!("Failed to parse {}", file_path.display()))?;

        // Set module path for all classes
        for class in &mut file_classes {
            class.module_path = module_path.clone();
        }

        classes.extend(file_classes);
    }

    Ok(classes)
}

/// Extract module path from file path relative to base
/// e.g., pybevy/camera.pyi -> "camera"
/// e.g., pybevy/input/keyboard.pyi -> "input.keyboard"
pub(super) fn extract_module_path(file_path: &Path, base_path: &Path) -> Option<String> {
    let relative = file_path.strip_prefix(base_path).ok()?;

    // Get the file stem (without .pyi extension)
    let stem = relative.file_stem()?.to_str()?;

    // Get parent directory components
    let parent = relative.parent()?;
    let parent_parts: Vec<_> = parent
        .components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect();

    if parent_parts.is_empty() {
        // Top-level file like camera.pyi -> "camera"
        Some(stem.to_string())
    } else {
        // Nested file like input/keyboard.pyi -> "input.keyboard"
        let mut parts = parent_parts;
        parts.push(stem);
        Some(parts.join("."))
    }
}

/// Parse a single Python stub file
pub fn parse_file(path: &Path) -> Result<Vec<PyClassDef>> {
    let content = std::fs::read_to_string(path)?;
    let mut classes = class_def::parse_stub_file(&content, path)?;
    if let Some(exports) = explicit_exports(&content) {
        classes.retain(|class| exports.contains(&class.python_name));
    }
    Ok(classes)
}

pub(super) fn explicit_exports(source: &str) -> Option<HashSet<String>> {
    let assignment = source.find("__all__")?;
    let list_start = source[assignment..].find('[')? + assignment;
    let mut depth = 0_usize;
    let mut list_end = None;
    for (offset, character) in source[list_start..].char_indices() {
        match character {
            '[' => depth += 1,
            ']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    list_end = Some(list_start + offset + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let list = &source[list_start..list_end?];
    let quoted_name = regex::Regex::new(r#"[\"']([A-Za-z_][A-Za-z0-9_]*)[\"']"#).ok()?;
    Some(
        quoted_name
            .captures_iter(list)
            .filter_map(|capture| capture.get(1).map(|name| name.as_str().to_string()))
            .collect(),
    )
}
