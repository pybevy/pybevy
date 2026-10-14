//! W030: pybevy.prelude must mirror the pinned Bevy prelude.

use std::{collections::HashSet, fs, mem, path::Path};

use crate::{
    model::PyClassDef,
    output::{Diagnostic, DiagnosticCode, Suggestion},
};

/// Compare the curated `pybevy/prelude.py` export list against the pinned
/// Bevy prelude (`bevy::prelude` = `bevy_internal::prelude`, the union of
/// every crate's `pub mod prelude` block).
///
/// A name is only required when PyBevy actually implements it (unimplemented
/// Bevy-prelude names are coverage debt, not prelude debt) and only
/// class-like (uppercase) names are compared, so Bevy prelude functions
/// (`vec2`, ...) and module re-exports stay out of the check. PyBevy-specific
/// prelude additions (implemented uppercase names absent from every Bevy
/// prelude) are reported for review instead of being silently allowed.
pub fn check_prelude_parity(
    rust_classes: &[PyClassDef],
    bevy_path: Option<&Path>,
    prelude_py: &Path,
) -> Vec<Diagnostic> {
    let Some(bevy_path) = bevy_path else {
        return Vec::new();
    };
    let Some(pybevy_prelude) = read_export_names(prelude_py) else {
        return Vec::new();
    };
    let Some(bevy_prelude) = bevy_prelude_names(bevy_path) else {
        return Vec::new();
    };

    let implemented: HashSet<&str> = rust_classes
        .iter()
        .map(|class| class.python_name.as_str())
        .collect();

    let missing: Vec<&String> = bevy_prelude
        .iter()
        .filter(|name| implemented.contains(name.as_str()) && !pybevy_prelude.contains(*name))
        .collect();
    let extra: Vec<&String> = pybevy_prelude
        .iter()
        .filter(|name| {
            is_type_name(name)
                && implemented.contains(name.as_str())
                && !bevy_prelude.contains(*name)
        })
        .collect();

    let mut diagnostics = Vec::new();
    if !missing.is_empty() {
        let mut names: Vec<&str> = missing.iter().map(|name| name.as_str()).collect();
        names.sort();
        let shown: Vec<&str> = names.iter().take(10).copied().collect();
        let message = format!(
            "pybevy.prelude is missing {} implemented Bevy-prelude name(s): {}",
            names.len(),
            shown.join(", ")
        );
        diagnostics.push(
            Diagnostic::warning(DiagnosticCode::W030, message)
                .with_note(
                    "the Bevy prelude is the canonical import surface; every implemented prelude name should be reachable from pybevy.prelude",
                )
                .with_suggestion(Suggestion::new(
                    "add the names to pybevy/prelude.py or record a validation exception with the reason",
                )),
        );
    }
    if !extra.is_empty() {
        let mut names: Vec<&str> = extra.iter().map(|name| name.as_str()).collect();
        names.sort();
        let shown: Vec<&str> = names.iter().take(10).copied().collect();
        diagnostics.push(
            Diagnostic::warning(
                DiagnosticCode::W030,
                format!(
                    "pybevy.prelude exports {} implemented name(s) absent from the Bevy prelude: {}",
                    names.len(),
                    shown.join(", ")
                ),
            )
            .with_note(
                "PyBevy-specific prelude additions are allowed but must be reviewed and excepted with a reason",
            )
            .with_suggestion(Suggestion::new(
                "record a validation exception per name, or drop the export if it is not part of the intended surface",
            )),
        );
    }
    diagnostics
}

/// Upper-case first letter: class-like names only.
fn is_type_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
}

/// Collect imported names from `from ... import (...)` blocks and single-line
/// imports in a Python file (PyBevy prelude style; `as` aliases bind the alias).
fn read_export_names(path: &Path) -> Option<HashSet<String>> {
    let source = fs::read_to_string(path).ok()?;
    let mut names = HashSet::new();
    let mut in_block = false;
    for raw_line in source.lines() {
        let line = strip_comment(raw_line);
        let trimmed = line.trim();
        if in_block {
            if trimmed.ends_with(')') {
                in_block = false;
                let inner = trimmed.trim_end_matches(')').trim();
                if !inner.starts_with("import") {
                    collect_identifiers(inner, &mut names);
                }
            } else if trimmed.contains(')') {
                in_block = false;
                let inner = trimmed.split(')').next().unwrap_or("");
                if !inner.starts_with("import") {
                    collect_identifiers(inner, &mut names);
                }
            } else {
                collect_identifiers(trimmed, &mut names);
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("from ") {
            let rest = rest.trim();
            if let Some(after_import) = rest.split(" import ").nth(1) {
                let after_import = after_import.trim();
                if after_import.starts_with('(') {
                    in_block = true;
                    let inner = after_import.strip_prefix('(').unwrap_or("");
                    if inner.contains(')') {
                        in_block = false;
                        let inner = inner.split(')').next().unwrap_or("");
                        if !inner.trim().is_empty() {
                            collect_identifiers(inner, &mut names);
                        }
                    } else if !inner.trim().is_empty() {
                        collect_identifiers(inner, &mut names);
                    }
                } else if !after_import.is_empty() {
                    collect_identifiers(after_import, &mut names);
                }
            }
        }
    }
    Some(names)
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn collect_identifiers(text: &str, names: &mut HashSet<String>) {
    for entry in text.split(',') {
        let entry = entry.trim();
        if entry.is_empty() || entry == "*" {
            continue;
        }
        let entry = exported_name(entry);
        // Only bare imported names; `a.b` submodule re-exports are skipped.
        if entry.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && entry
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            names.insert(entry.to_string());
        }
    }
}

/// The pinned Bevy prelude: `bevy_internal::prelude` (the module `bevy`
/// re-exports) plus every crate prelude block it pulls in. Wildcard pulls
/// (`crate::primitives::*`) are resolved through the target module's own
/// `pub use` surface.
fn bevy_prelude_names(bevy_path: &Path) -> Option<HashSet<String>> {
    let internal =
        fs::read_to_string(bevy_path.join("crates/bevy_internal/src/prelude.rs")).ok()?;
    let mut names = HashSet::new();
    let mut modules = HashSet::new();

    for statement in use_statements(&internal) {
        for entry in brace_entries(&statement) {
            let (module, terminal) = split_terminal(&entry);
            if terminal == Some("*") {
                if let Some(short) = module_pull(module) {
                    modules.insert(short.to_string());
                } else if let Some(module) = module
                    && let Some(short) = module.strip_prefix("crate::")
                {
                    names.extend(wildcard_names(
                        bevy_path.join("crates/bevy_internal/src").as_path(),
                        short,
                        0,
                    ));
                }
            } else if terminal.is_some_and(is_type_name) {
                names.insert(terminal.unwrap().to_string());
            }
        }
    }

    for module in modules {
        let lib = bevy_path.join(format!("crates/bevy_{module}/src/lib.rs"));
        let Ok(source) = fs::read_to_string(&lib) else {
            continue;
        };
        let Some(block) = prelude_block(&source, lib.parent().unwrap_or(Path::new(""))) else {
            continue;
        };
        for statement in use_statements(&block) {
            for entry in brace_entries(&statement) {
                let (entry_module, terminal) = split_terminal(&entry);
                if terminal == Some("*") {
                    if let Some(wild_module) = entry_module
                        && let Some(short) = wild_module.strip_prefix("crate::")
                    {
                        names.extend(wildcard_names(
                            bevy_path
                                .join(format!("crates/bevy_{module}/src"))
                                .as_path(),
                            short,
                            0,
                        ));
                    }
                } else if terminal.is_some_and(is_type_name) {
                    names.insert(terminal.unwrap().to_string());
                }
            }
        }
    }

    Some(names)
}

/// A `crate::<module>::prelude` parent means "pull that crate's prelude";
/// returns the crate's short name (`crate::a11y::prelude` -> `a11y`).
fn module_pull(module: Option<&str>) -> Option<&str> {
    let without = module?.strip_suffix("::prelude")?;
    Some(without.rsplit("::").next().unwrap_or(without))
}

/// Names made public by a module wildcard: the target module's `pub use`
/// terminals plus its own top-level `pub` items, recursing into nested
/// `pub use X::*;` entries (capped depth).
fn wildcard_names(src_dir: &Path, module_path: &str, depth: u32) -> Vec<String> {
    let relative = module_path.replace("::", "/");
    let file = src_dir.join(format!("{relative}.rs"));
    let file = if file.exists() {
        file
    } else {
        src_dir.join(format!("{relative}/mod.rs"))
    };
    let Ok(source) = fs::read_to_string(&file) else {
        return Vec::new();
    };
    let mut names = pub_item_names(&source);
    for statement in use_statements(&source) {
        for entry in brace_entries(&statement) {
            let (module, terminal) = split_terminal(&entry);
            if terminal == Some("*") {
                if let Some(module) = module
                    && depth < 3
                {
                    let next = if let Some(short) = module.strip_prefix("crate::") {
                        short.to_string()
                    } else {
                        format!("{module_path}::{module}")
                    };
                    names.extend(wildcard_names(src_dir, &next, depth + 1));
                }
            } else if let Some(name) = terminal.filter(|t| is_type_name(t)) {
                names.push(name.to_string());
            }
        }
    }
    names.dedup();
    names
}

/// Top-level `pub struct/enum/trait/type/const/static/fn` definitions in a
/// source file (what a `mod::*` wildcard re-exports from definitions).
fn pub_item_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for raw_line in source.lines() {
        // Only column-0 items: nested (indented) pub methods are not exported
        // by a module-level wildcard.
        if !raw_line.starts_with("pub ") {
            continue;
        }
        let line = strip_comment(raw_line);
        if !line.starts_with("pub ") {
            continue;
        }
        let rest = line[4..].trim_start();
        if rest.starts_with('[') || rest.starts_with('(') {
            // `pub(crate)`, `pub(super)`: not public.
            continue;
        }
        for keyword in ["struct", "enum", "trait", "type", "const", "static", "fn"] {
            if let Some(after) = rest.strip_prefix(keyword) {
                let after = after.trim_start();
                let name: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if is_type_name(&name) || keyword == "const" || keyword == "static" {
                    names.push(name);
                }
                break;
            }
        }
    }
    names
}

/// Split a use path into (parent module, terminal segment), resolving nested
/// brace groups to their individual entries first.
fn split_terminal(entry: &str) -> (Option<&str>, Option<&str>) {
    let entry = entry.trim();
    let last_sep = entry.rfind("::").map(|i| (i, entry.len()));
    match last_sep {
        Some((start, end)) if entry[start + 2..end].starts_with('{') => {
            // Nested group: caller already expanded it; treat as no terminal.
            (None, None)
        }
        Some((start, _)) => (
            Some(&entry[..start]),
            Some(exported_name(&entry[start + 2..])),
        ),
        None => (None, Some(exported_name(entry))),
    }
}

/// The name a `use` entry binds: the alias of `X as Y` (`_` for anonymous
/// trait imports, which bind no name), otherwise the entry itself.
fn exported_name(terminal: &str) -> &str {
    match terminal.split_once(" as ") {
        Some((_, alias)) => alias.trim(),
        None => terminal.trim(),
    }
}

/// Expand a use statement's path into individual entries, recursing into
/// brace groups (`a::{B, C}` -> `a::B`, `a::C`).
fn brace_entries(statement: &str) -> Vec<String> {
    let Some(path) = statement
        .strip_prefix("pub use ")
        .and_then(|rest| rest.strip_suffix(';'))
    else {
        return Vec::new();
    };
    let path = path.trim();
    let mut out = Vec::new();
    expand_path(path, &mut out);
    out
}

fn expand_path(path: &str, out: &mut Vec<String>) {
    let path = path.trim();
    if let Some(open) = path.find('{') {
        let Some(close) = path.rfind('}') else {
            return;
        };
        let base = path[..open].trim().trim_end_matches("::");
        for entry in split_top_level(&path[open + 1..close]) {
            let entry = entry.trim();
            if entry.is_empty() || entry == "*" {
                continue;
            }
            let joined = if entry.starts_with("crate::") || base.is_empty() {
                entry.to_string()
            } else {
                format!("{base}::{entry}")
            };
            if entry.contains('{') {
                expand_path(&joined, out);
            } else {
                out.push(joined);
            }
        }
    } else {
        out.push(path.to_string());
    }
}

/// Split on commas that are not nested inside braces.
fn split_top_level(inner: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (i, ch) in inner.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&inner[start..]);
    out
}

/// All `pub use ...;` statements in a source file (multi-line aware).
fn use_statements(source: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    for line in source.lines() {
        let line = strip_comment(line);
        let trimmed = line.trim();
        if trimmed.starts_with("pub use ") {
            current = trimmed.to_string();
            if current.ends_with(';') {
                statements.push(mem::take(&mut current));
            }
        } else if !current.is_empty() {
            current.push(' ');
            current.push_str(trimmed);
            if current.ends_with(';') {
                let statement = mem::take(&mut current).trim().to_string();
                statements.push(statement);
            }
        }
    }
    statements
}

/// Extract the body of the first `pub mod prelude { ... }` block by brace
/// matching, falling back to a file-based `pub mod prelude;` declaration.
fn prelude_block(source: &str, lib_dir: &Path) -> Option<String> {
    if let Some(start) = source.find("pub mod prelude {") {
        let body_start = start + "pub mod prelude {".len();
        let mut depth = 1;
        for (offset, ch) in source[body_start..].char_indices() {
            if ch == '{' {
                depth += 1;
            } else if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    return Some(source[body_start..body_start + offset].to_string());
                }
            }
        }
        return None;
    }
    if source.contains("pub mod prelude;") {
        return fs::read_to_string(lib_dir.join("prelude.rs")).ok();
    }
    None
}
