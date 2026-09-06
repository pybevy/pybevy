use std::{collections::HashSet, path::Path};

use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StubSymbolKind {
    Class,
    Function,
    Alias,
    Constant,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubSymbolDef {
    pub module_path: String,
    pub name: String,
    pub kind: StubSymbolKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StubReexport {
    pub name: String,
    pub module: String,
    pub symbol: String,
}

pub fn parse_symbol_directory(path: &Path) -> Result<Vec<StubSymbolDef>> {
    let mut symbols = Vec::new();
    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "pyi")
        })
    {
        let file_path = entry.path();
        let Some(module_path) = super::extract_module_path(file_path, path) else {
            continue;
        };
        let source = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read {}", file_path.display()))?;
        symbols.extend(parse_file_symbols(&source, &module_path)?);
    }
    symbols.sort_by(|left, right| {
        (&left.module_path, &left.name).cmp(&(&right.module_path, &right.name))
    });
    symbols.dedup_by(|left, right| {
        left.module_path == right.module_path && left.name == right.name && left.kind == right.kind
    });
    Ok(symbols)
}

pub fn parse_reexports(path: &Path) -> Result<Vec<StubReexport>> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let exports = super::explicit_exports(&source).unwrap_or_default();
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_python::LANGUAGE.into())?;
    let tree = parser
        .parse(&source, None)
        .context("Failed to parse Python re-export module")?;
    let bytes = source.as_bytes();
    let mut bindings = std::collections::HashMap::<String, (String, String)>::new();
    let mut cursor = tree.root_node().walk();
    for node in tree.root_node().children(&mut cursor) {
        if node.kind() == "import_from_statement" {
            let text = node.utf8_text(bytes).unwrap_or_default();
            let Some(rest) = text.trim().strip_prefix("from ") else {
                continue;
            };
            let Some((module, _)) = rest.split_once(" import") else {
                continue;
            };
            let module = if let Some(relative) = module.trim().strip_prefix('.') {
                format!("pybevy.{relative}")
            } else {
                module.trim().to_string()
            };
            let mut after_import = false;
            let mut child_cursor = node.walk();
            for child in node.children(&mut child_cursor) {
                if child.utf8_text(bytes).unwrap_or_default() == "import" {
                    after_import = true;
                    continue;
                }
                if !after_import {
                    continue;
                }
                match child.kind() {
                    "identifier" | "dotted_name" => {
                        let name = child.utf8_text(bytes).unwrap_or_default();
                        bindings.insert(name.to_string(), (module.clone(), name.to_string()));
                    }
                    "aliased_import" => {
                        let original = child
                            .child_by_field_name("name")
                            .and_then(|name| name.utf8_text(bytes).ok())
                            .unwrap_or_default();
                        let alias = child
                            .child_by_field_name("alias")
                            .and_then(|name| name.utf8_text(bytes).ok())
                            .unwrap_or(original);
                        bindings.insert(alias.to_string(), (module.clone(), original.to_string()));
                    }
                    _ => {}
                }
            }
        } else if node.kind() == "expression_statement"
            && let Some(assignment) = node.named_child(0)
            && assignment.kind() == "assignment"
            && let (Some(left), Some(right)) = (
                assignment.child_by_field_name("left"),
                assignment.child_by_field_name("right"),
            )
            && left.kind() == "identifier"
            && right.kind() == "attribute"
            && let (Some(object), Some(attribute)) = (
                right.child_by_field_name("object"),
                right.child_by_field_name("attribute"),
            )
            && object.kind() == "identifier"
        {
            let object = object.utf8_text(bytes).unwrap_or_default();
            if let Some((module, _)) = bindings.get(object).cloned() {
                let name = left.utf8_text(bytes).unwrap_or_default();
                let symbol = attribute.utf8_text(bytes).unwrap_or_default();
                bindings.insert(name.to_string(), (module, symbol.to_string()));
            }
        }
    }

    let mut reexports = bindings
        .into_iter()
        .filter(|(name, _)| exports.contains(name))
        .map(|(name, (module, symbol))| StubReexport {
            name,
            module,
            symbol,
        })
        .collect::<Vec<_>>();
    reexports.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(reexports)
}

fn parse_file_symbols(source: &str, module_path: &str) -> Result<Vec<StubSymbolDef>> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_python::LANGUAGE.into())?;
    let tree = parser
        .parse(source, None)
        .context("Failed to parse Python stub")?;
    let exports = super::explicit_exports(source);
    let mut symbols = Vec::new();
    let mut cursor = tree.root_node().walk();
    for node in tree.root_node().children(&mut cursor) {
        collect_top_level_symbol(
            node,
            source.as_bytes(),
            module_path,
            exports.as_ref(),
            &mut symbols,
        );
    }
    Ok(symbols)
}

fn collect_top_level_symbol(
    node: Node<'_>,
    source: &[u8],
    module_path: &str,
    exports: Option<&HashSet<String>>,
    symbols: &mut Vec<StubSymbolDef>,
) {
    match node.kind() {
        "class_definition" => push_named(
            node.child_by_field_name("name"),
            StubSymbolKind::Class,
            source,
            module_path,
            exports,
            symbols,
        ),
        "function_definition" => push_named(
            node.child_by_field_name("name"),
            StubSymbolKind::Function,
            source,
            module_path,
            exports,
            symbols,
        ),
        "decorated_definition" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if matches!(child.kind(), "class_definition" | "function_definition") {
                    collect_top_level_symbol(child, source, module_path, exports, symbols);
                }
            }
        }
        "expression_statement" => {
            let Some(child) = node.named_child(0) else {
                return;
            };
            collect_assignment(child, source, module_path, exports, symbols);
        }
        "type_alias_statement" => {
            if let Some(name) = type_alias_name(node, source) {
                push_name(name, StubSymbolKind::Alias, module_path, exports, symbols);
            }
        }
        "import_from_statement" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() != "aliased_import" {
                    continue;
                }
                let original = child
                    .child_by_field_name("name")
                    .and_then(|name| name.utf8_text(source).ok());
                let alias = child
                    .child_by_field_name("alias")
                    .and_then(|name| name.utf8_text(source).ok());
                if original == alias {
                    push_name(
                        alias.unwrap_or_default(),
                        StubSymbolKind::Alias,
                        module_path,
                        exports,
                        symbols,
                    );
                }
            }
        }
        _ => {}
    }
}

fn type_alias_name<'a>(node: Node<'a>, source: &'a [u8]) -> Option<&'a str> {
    let left = node.child_by_field_name("left")?;
    let declaration = left.utf8_text(source).ok()?;
    let (name, parameters) = declaration.split_once('[')?;
    let parameters = parameters.strip_suffix(']')?;
    let right = node
        .child_by_field_name("right")?
        .utf8_text(source)
        .ok()?
        .trim();
    let is_transparent = parameters.split(',').any(|parameter| {
        parameter
            .trim()
            .trim_start_matches('*')
            .split([':', '='])
            .next()
            .is_some_and(|parameter_name| parameter_name.trim() == right)
    });
    let name = name.trim();
    (is_transparent
        && !name.is_empty()
        && name
            .chars()
            .all(|character| character == '_' || character.is_alphanumeric()))
    .then_some(name)
}

fn collect_assignment(
    node: Node<'_>,
    source: &[u8],
    module_path: &str,
    exports: Option<&HashSet<String>>,
    symbols: &mut Vec<StubSymbolDef>,
) {
    if node.kind() != "assignment" {
        return;
    }
    let Some(left) = node.child_by_field_name("left") else {
        return;
    };
    let name = left.utf8_text(source).unwrap_or_default();
    if name == "__all__" {
        return;
    }
    let kind = if node.child_by_field_name("type").is_some() {
        StubSymbolKind::Constant
    } else {
        let right = node.child_by_field_name("right");
        match right.map(|value| value.kind()) {
            Some("identifier" | "binary_operator" | "subscript") => StubSymbolKind::Alias,
            Some("call") if right.is_some_and(|right| is_type_checking_call(right, source)) => {
                StubSymbolKind::Alias
            }
            _ => StubSymbolKind::Constant,
        }
    };
    push_name(name, kind, module_path, exports, symbols);
}

fn is_type_checking_call(node: Node<'_>, source: &[u8]) -> bool {
    let Some(function) = node.child_by_field_name("function") else {
        return false;
    };
    function
        .utf8_text(source)
        .ok()
        .is_some_and(|name| matches!(name, "NewType" | "TypeVar" | "TypeVarTuple"))
}

fn push_named(
    name: Option<Node<'_>>,
    kind: StubSymbolKind,
    source: &[u8],
    module_path: &str,
    exports: Option<&HashSet<String>>,
    symbols: &mut Vec<StubSymbolDef>,
) {
    if let Some(name) = name.and_then(|name| name.utf8_text(source).ok()) {
        push_name(name, kind, module_path, exports, symbols);
    }
}

fn push_name(
    name: &str,
    kind: StubSymbolKind,
    module_path: &str,
    exports: Option<&HashSet<String>>,
    symbols: &mut Vec<StubSymbolDef>,
) {
    if name.starts_with('_') || exports.is_some_and(|exports| !exports.contains(name)) {
        return;
    }
    symbols.push(StubSymbolDef {
        module_path: module_path.to_string(),
        name: name.to_string(),
        kind,
    });
}
