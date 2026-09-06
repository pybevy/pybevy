use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use tree_sitter::{Node, Parser};
use walkdir::WalkDir;

use super::model::{ApiCatalog, ApiPath, MemberRef, SymbolUseKind, TestUsage};
use crate::python_parser::module_symbols::StubSymbolKind;

#[derive(Debug)]
struct ImportBindings<'a> {
    catalog: &'a ApiCatalog,
    classes: HashMap<String, ApiPath>,
    symbols: HashMap<String, (ApiPath, StubSymbolKind)>,
    modules: HashMap<String, String>,
    unresolved_classes: HashMap<String, String>,
}

impl<'a> ImportBindings<'a> {
    fn new(catalog: &'a ApiCatalog) -> Self {
        Self {
            catalog,
            classes: HashMap::new(),
            symbols: HashMap::new(),
            modules: HashMap::new(),
            unresolved_classes: HashMap::new(),
        }
    }

    fn get(&self, name: &str) -> Option<&ApiPath> {
        self.classes.get(name)
    }

    fn insert_class(&mut self, name: String, path: ApiPath) {
        self.classes.insert(name, path);
    }

    fn insert_symbol(&mut self, name: String, path: ApiPath, kind: StubSymbolKind) {
        if kind == StubSymbolKind::Class {
            self.insert_class(name, path);
        } else {
            self.symbols.insert(name, (path, kind));
        }
    }

    fn resolve_expression(&self, node: Node<'_>, source: &[u8]) -> Option<ApiPath> {
        if node.kind() == "identifier" {
            return node
                .utf8_text(source)
                .ok()
                .and_then(|name| self.classes.get(name))
                .cloned();
        }
        if node.kind() != "attribute" {
            return None;
        }

        let expression = node.utf8_text(source).ok()?;
        for (alias, module) in &self.modules {
            let Some(suffix) = expression.strip_prefix(alias) else {
                continue;
            };
            let Some(suffix) = suffix.strip_prefix('.') else {
                continue;
            };
            let (module_suffix, symbol) = suffix
                .rsplit_once('.')
                .map_or(("", suffix), |(prefix, symbol)| (prefix, symbol));
            let resolved_module = if module_suffix.is_empty() {
                module.clone()
            } else {
                format!("{module}.{module_suffix}")
            };
            if let Some((path, kind)) = self.catalog.resolve_exact_symbol(&resolved_module, symbol)
            {
                return (kind == StubSymbolKind::Class).then_some(path);
            }
            if let Some(path) = self.catalog.resolve_import(&resolved_module, symbol) {
                return Some(path);
            }
        }
        None
    }

    fn is_pybevy_qualified(&self, node: Node<'_>, source: &[u8]) -> bool {
        let Ok(expression) = node.utf8_text(source) else {
            return false;
        };
        self.modules.keys().any(|alias| {
            expression == alias
                || expression
                    .strip_prefix(alias)
                    .is_some_and(|suffix| suffix.starts_with('.'))
        })
    }

    fn resolve_symbol_expression(
        &self,
        node: Node<'_>,
        source: &[u8],
    ) -> Option<(ApiPath, StubSymbolKind)> {
        if node.kind() == "identifier" {
            return node
                .utf8_text(source)
                .ok()
                .and_then(|name| self.symbols.get(name))
                .cloned();
        }
        if node.kind() != "attribute" {
            return None;
        }
        let expression = node.utf8_text(source).ok()?;
        for (alias, module) in &self.modules {
            let Some(suffix) = expression
                .strip_prefix(alias)
                .and_then(|suffix| suffix.strip_prefix('.'))
            else {
                continue;
            };
            let (module_suffix, symbol) = suffix
                .rsplit_once('.')
                .map_or(("", suffix), |(prefix, symbol)| (prefix, symbol));
            let resolved_module = if module_suffix.is_empty() {
                module.clone()
            } else {
                format!("{module}.{module_suffix}")
            };
            if let Some((path, kind)) = self.catalog.resolve_exact_symbol(&resolved_module, symbol)
            {
                return (kind != StubSymbolKind::Class).then_some((path, kind));
            }
        }
        None
    }
}

/// Type annotation wrappers that are not classes themselves, stripped during
/// Query/Res type parsing but should not be tracked as class references.
const TYPE_WRAPPERS: &[&str] = &[
    "Mut",
    "With",
    "Without",
    "Added",
    "Changed",
    "Removed",
    "Or",
    "Has",
    "AnyOf",
    "Local",
    "Res",
    "ResMut",
    "Query",
    "View",
    "EventReader",
    "EventWriter",
    "Observer",
    "Trigger",
    "Add",
    "Remove",
    "Insert",
    "Discard",
    "Despawn",
    "Single",
];

/// Parse all test files in the given directory
pub fn parse_test_files(test_path: &Path, catalog: &ApiCatalog) -> Result<TestUsage> {
    let mut usage = TestUsage::default();

    let mut parser = Parser::new();
    let language = tree_sitter_python::LANGUAGE;
    parser
        .set_language(&language.into())
        .context("Failed to set tree-sitter-python language")?;

    for entry in WalkDir::new(test_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().extension().is_some_and(|ext| ext == "py")
                && e.file_type().is_file()
                && e.path()
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("test_"))
        })
    {
        let path = entry.path().to_path_buf();
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read test file: {}", path.display()))?;

        parse_single_file(&mut parser, &path, &source, catalog, &mut usage)?;
    }

    Ok(usage)
}

/// Parse a single Python test file and extract class/member references
fn parse_single_file(
    parser: &mut Parser,
    file_path: &Path,
    source: &str,
    catalog: &ApiCatalog,
    usage: &mut TestUsage,
) -> Result<()> {
    let tree = parser
        .parse(source, None)
        .context("Failed to parse Python file")?;
    let root = tree.root_node();
    let source_bytes = source.as_bytes();

    let has_wildcard_pybevy_import = has_wildcard_import(root, source_bytes);

    let imports = extract_imports(root, source_bytes, catalog, has_wildcard_pybevy_import);
    extract_explicit_markers(root, source, file_path, catalog, usage);

    let mut var_types: HashMap<String, Vec<ApiPath>> = HashMap::new();

    extract_references(
        root,
        source_bytes,
        file_path,
        &imports,
        has_wildcard_pybevy_import,
        &mut var_types,
        usage,
    );

    Ok(())
}

fn extract_explicit_markers(
    root: Node<'_>,
    source: &str,
    file_path: &Path,
    catalog: &ApiCatalog,
    usage: &mut TestUsage,
) {
    let marker = regex::Regex::new(
        r"(?m)^\s*#\s*pybevy-exercise:\s+(pybevy(?:\.[A-Za-z_][A-Za-z0-9_]*)+)\s+([a-z_]+)\s*$",
    )
    .expect("exercise marker regex is valid");
    let mut cursor = root.walk();
    for node in root.children(&mut cursor) {
        let function = if node.kind() == "function_definition" {
            Some(node)
        } else if node.kind() == "decorated_definition" {
            let mut inner = node.walk();
            node.children(&mut inner)
                .find(|child| child.kind() == "function_definition")
        } else {
            None
        };
        let Some(function) = function else {
            continue;
        };
        let is_test = function
            .child_by_field_name("name")
            .and_then(|name| name.utf8_text(source.as_bytes()).ok())
            .is_some_and(|name| name.starts_with("test_"));
        if !is_test {
            continue;
        }
        let function_source = &source[function.byte_range()];
        for capture in marker.captures_iter(function_source) {
            let path = capture.get(1).unwrap().as_str();
            let kind = capture.get(2).unwrap().as_str();
            let line = function.start_position().row
                + function_source[..capture.get(0).unwrap().start()]
                    .bytes()
                    .filter(|byte| *byte == b'\n')
                    .count()
                + 1;
            if !record_explicit_marker(catalog, usage, path, kind, file_path, line) {
                usage.record_unresolved(
                    format!("{path} {kind}"),
                    "invalid explicit exercise marker",
                    file_path,
                    line,
                );
            }
        }
    }
}

fn record_explicit_marker(
    catalog: &ApiCatalog,
    usage: &mut TestUsage,
    operation_path: &str,
    kind: &str,
    file_path: &Path,
    line: usize,
) -> bool {
    if matches!(kind, "module_function_call" | "module_symbol_read") {
        let Some((path, symbol_kind)) = catalog.symbol_path(operation_path) else {
            return false;
        };
        let valid = match kind {
            "module_function_call" => symbol_kind == StubSymbolKind::Function,
            "module_symbol_read" => matches!(
                symbol_kind,
                StubSymbolKind::Alias | StubSymbolKind::Constant
            ),
            _ => false,
        };
        if valid {
            usage.record_symbol_site(
                &path,
                if kind == "module_function_call" {
                    SymbolUseKind::Call
                } else {
                    SymbolUseKind::Read
                },
                file_path,
                line,
                false,
            );
        }
        return valid;
    }

    let (class_path_text, member) = if kind == "constructor_call" {
        (operation_path, MemberRef::Constructor)
    } else {
        let Some((class_path, name)) = operation_path.rsplit_once('.') else {
            return false;
        };
        let member = match kind {
            "property_read" => MemberRef::Property(name.to_string()),
            "property_write" => MemberRef::PropertySetter(name.to_string()),
            "method_call" => MemberRef::Method(name.to_string()),
            "static_method_call" => MemberRef::StaticMethod(name.to_string()),
            "class_attribute_read" => MemberRef::ClassAttr(name.to_string()),
            "enum_variant_read" => MemberRef::EnumVariant(name.to_string()),
            "nested_type_reference" => MemberRef::NestedType(name.to_string()),
            _ => return false,
        };
        (class_path, member)
    };
    let Some(class_path) = catalog.class_path(class_path_text) else {
        return false;
    };
    if !catalog.has_member(&class_path, &member) {
        return false;
    }
    usage.record_member_site(&class_path, member, file_path, line, false);
    true
}

/// Check if there's a `from pybevy.prelude import *` or `from pybevy import *`
fn has_wildcard_import(root: Node, source: &[u8]) -> bool {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "import_from_statement" {
            let text = child.utf8_text(source).unwrap_or("");
            if text.contains("pybevy") && text.contains("import *") {
                return true;
            }
        }
    }
    false
}

/// Extract import mappings: local name -> canonical public class path.
fn extract_imports<'a>(
    root: Node<'_>,
    source: &[u8],
    catalog: &'a ApiCatalog,
    has_wildcard: bool,
) -> ImportBindings<'a> {
    let mut imports = ImportBindings::new(catalog);
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "import_from_statement" => {
                let text = child.utf8_text(source).unwrap_or("");
                if !text.contains("pybevy") {
                    continue;
                }
                let Some(module) = import_from_module(child, source) else {
                    continue;
                };
                extract_import_names(child, source, &module, catalog, &mut imports);
            }
            "import_statement" => extract_module_imports(child, source, &mut imports),
            _ => {}
        }
    }

    if has_wildcard {
        for (name, path, kind) in catalog.wildcard_symbols() {
            if !is_type_wrapper(name) {
                if kind == StubSymbolKind::Class {
                    imports
                        .classes
                        .entry(name.to_string())
                        .or_insert_with(|| path.clone());
                } else {
                    imports
                        .symbols
                        .entry(name.to_string())
                        .or_insert_with(|| (path.clone(), kind));
                }
            }
        }
    }

    imports
}

/// Extract individual import names from an import_from_statement
fn import_from_module(node: Node, source: &[u8]) -> Option<String> {
    let text = node.utf8_text(source).ok()?.trim();
    let rest = text.strip_prefix("from ")?;
    let import = regex::Regex::new(r"(?s)^([.A-Za-z0-9_]+)\s+import\b").ok()?;
    import
        .captures(rest)
        .and_then(|capture| capture.get(1))
        .map(|module| module.as_str().to_string())
}

fn extract_module_imports(node: Node, source: &[u8], imports: &mut ImportBindings<'_>) {
    let text = node.utf8_text(source).unwrap_or("");
    let Some(imports_text) = text.trim().strip_prefix("import ") else {
        return;
    };
    for import in imports_text.split(',').map(str::trim) {
        let (module, alias) = import
            .split_once(" as ")
            .map_or((import, None), |(module, alias)| {
                (module.trim(), Some(alias.trim()))
            });
        if module != "pybevy" && !module.starts_with("pybevy.") {
            continue;
        }
        let local_name = alias.unwrap_or_else(|| module.split('.').next().unwrap_or(module));
        let bound_module = if alias.is_some() { module } else { local_name };
        imports
            .modules
            .insert(local_name.to_string(), bound_module.to_string());
    }
}

fn extract_import_names(
    node: Node,
    source: &[u8],
    module: &str,
    catalog: &ApiCatalog,
    imports: &mut ImportBindings<'_>,
) {
    let child_count = node.child_count();
    let mut i = 0;

    while i < child_count {
        let child = node.child(i).unwrap();
        if child.utf8_text(source).unwrap_or("") == "import" {
            i += 1;
            break;
        }
        i += 1;
    }

    while i < child_count {
        let child = node.child(i).unwrap();
        match child.kind() {
            "dotted_name" | "identifier" => {
                let name = child.utf8_text(source).unwrap_or("").to_string();
                if !name.is_empty() && !is_type_wrapper(&name) {
                    // Check for alias (next might be "as" keyword)
                    if i + 2 < child_count {
                        let next = node.child(i + 1).unwrap();
                        if next.utf8_text(source).unwrap_or("") == "as" {
                            let alias = node.child(i + 2).unwrap();
                            let alias_name = alias.utf8_text(source).unwrap_or("").to_string();
                            if !alias_name.is_empty() {
                                record_import(imports, catalog, module, &name, alias_name);
                                i += 3;
                                continue;
                            }
                        }
                    }
                    record_import(imports, catalog, module, &name, name.clone());
                }
            }
            "aliased_import" => {
                // Handle `Name as Alias` pattern
                let original = child
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source).ok())
                    .unwrap_or("")
                    .to_string();
                let alias = child
                    .child_by_field_name("alias")
                    .and_then(|n| n.utf8_text(source).ok())
                    .unwrap_or("")
                    .to_string();

                if !original.is_empty() && !is_type_wrapper(&original) {
                    if !alias.is_empty() {
                        record_import(imports, catalog, module, &original, alias);
                    } else {
                        record_import(imports, catalog, module, &original, original.clone());
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
}

fn record_import(
    imports: &mut ImportBindings<'_>,
    catalog: &ApiCatalog,
    module: &str,
    symbol: &str,
    local_name: String,
) {
    if let Some((path, kind)) = catalog.resolve_symbol(module, symbol) {
        imports.insert_symbol(local_name, path, kind);
    } else if catalog.has_module(module) && looks_like_class_name(symbol) {
        imports
            .unresolved_classes
            .insert(local_name, format!("{module}.{symbol}"));
    }
}

fn is_type_wrapper(name: &str) -> bool {
    TYPE_WRAPPERS.contains(&name)
}

/// Extract references from AST nodes
fn extract_references(
    node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    match node.kind() {
        "assignment" => {
            process_assignment(
                node,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
        }
        "augmented_assignment" => {
            // e.g., `var.prop += value`: record the property access
            if let Some(left) = node.child_by_field_name("left")
                && left.kind() == "attribute"
            {
                process_attribute_access(
                    left,
                    source,
                    file_path,
                    imports,
                    has_wildcard,
                    var_types,
                    usage,
                );
            }
        }
        "assert_statement" => {
            let mut asserted = TestUsage::default();
            let mut asserted_types = var_types.clone();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                process_expression(
                    child,
                    source,
                    file_path,
                    imports,
                    has_wildcard,
                    &mut asserted_types,
                    &mut asserted,
                );
            }
            if has_uncertain_control_flow(node) {
                usage.merge_referenced(asserted);
            } else {
                usage.merge_as_asserted(asserted);
            }
            return;
        }
        "with_statement" if is_pytest_raises(node, source) => {
            let mut expected_error = TestUsage::default();
            let mut expected_error_types = var_types.clone();
            if let Some(body) = node.child_by_field_name("body") {
                extract_references_children(
                    body,
                    source,
                    file_path,
                    imports,
                    has_wildcard,
                    &mut expected_error_types,
                    &mut expected_error,
                );
            }
            if expected_error.operation_count() == 1 {
                usage.merge_as_asserted(expected_error);
            } else {
                usage.merge_referenced(expected_error);
            }
            return;
        }
        "expression_statement" | "return_statement" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                process_expression(
                    child,
                    source,
                    file_path,
                    imports,
                    has_wildcard,
                    var_types,
                    usage,
                );
            }
        }
        "function_definition" => {
            let mut local_types = var_types.clone();
            process_function_params(node, source, imports, &mut local_types);

            if let Some(body) = node.child_by_field_name("body") {
                extract_references_children(
                    body,
                    source,
                    file_path,
                    imports,
                    has_wildcard,
                    &mut local_types,
                    usage,
                );
            }
            return; // Don't recurse again below
        }
        "decorator" => {
            process_decorator(
                node,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
            return;
        }
        "for_statement" => {
            process_for_loop(
                node,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
            return;
        }
        _ => {}
    }

    extract_references_children(
        node,
        source,
        file_path,
        imports,
        has_wildcard,
        var_types,
        usage,
    );
}

fn process_decorator(
    node: Node<'_>,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    let Some(expression) = node.named_child(0) else {
        return;
    };
    if expression.kind() == "call" {
        process_expression(
            expression,
            source,
            file_path,
            imports,
            has_wildcard,
            var_types,
            usage,
        );
    } else if let Some((path, _kind)) = imports.resolve_symbol_expression(expression, source) {
        record_symbol_node(usage, &path, SymbolUseKind::Call, file_path, expression);
    } else if let Some(class_path) = imports.resolve_expression(expression, source) {
        record_member_node(
            usage,
            &class_path,
            MemberRef::Constructor,
            file_path,
            expression,
        );
    } else {
        process_expression(
            expression,
            source,
            file_path,
            imports,
            has_wildcard,
            var_types,
            usage,
        );
    }
}

fn is_pytest_raises(node: Node<'_>, source: &[u8]) -> bool {
    node.child_by_field_name("body")
        .and_then(|body| source.get(node.start_byte()..body.start_byte()))
        .is_some_and(|prefix| {
            prefix
                .windows(b"pytest.raises(".len())
                .any(|window| window == b"pytest.raises(")
        })
}

fn has_uncertain_control_flow(node: Node<'_>) -> bool {
    if matches!(node.kind(), "boolean_operator" | "conditional_expression") {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(has_uncertain_control_flow)
}

fn execution_is_conclusive(node: Node<'_>) -> bool {
    let mut current = Some(node);
    while let Some(node) = current {
        if matches!(node.kind(), "boolean_operator" | "conditional_expression") {
            return false;
        }
        if matches!(
            node.kind(),
            "expression_statement"
                | "assignment"
                | "augmented_assignment"
                | "assert_statement"
                | "return_statement"
                | "with_statement"
                | "if_statement"
                | "while_statement"
        ) {
            break;
        }
        current = node.parent();
    }
    true
}

fn record_member_node(
    usage: &mut TestUsage,
    class_path: &ApiPath,
    member: MemberRef,
    file_path: &Path,
    node: Node<'_>,
) {
    usage.record_member_site(
        class_path,
        member,
        file_path,
        node.start_position().row + 1,
        execution_is_conclusive(node),
    );
}

fn record_symbol_node(
    usage: &mut TestUsage,
    path: &ApiPath,
    kind: SymbolUseKind,
    file_path: &Path,
    node: Node<'_>,
) {
    usage.record_symbol_site(
        path,
        kind,
        file_path,
        node.start_position().row + 1,
        execution_is_conclusive(node),
    );
}

fn extract_references_children(
    node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_references(
            child,
            source,
            file_path,
            imports,
            has_wildcard,
            var_types,
            usage,
        );
    }
}

/// Process an assignment statement to track variable types and references
fn process_assignment(
    node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    let left = match node.child_by_field_name("left") {
        Some(n) => n,
        None => return,
    };
    let right = match node.child_by_field_name("right") {
        Some(n) => n,
        None => return,
    };

    if left.kind() == "attribute" {
        process_attribute_setter(
            left,
            source,
            file_path,
            imports,
            has_wildcard,
            var_types,
            usage,
        );
    }

    if left.kind() == "identifier" {
        let variable = left.utf8_text(source).unwrap_or_default();
        if !variable.is_empty() {
            if let Some(class_path) = resolve_value_type(right, source, imports, var_types) {
                var_types
                    .entry(variable.to_string())
                    .or_default()
                    .push(class_path);
            }
            if let Some(class_path) =
                resolve_collection_element_type(right, source, imports, var_types)
            {
                var_types
                    .entry(format!("__container_type:{variable}"))
                    .or_default()
                    .push(class_path);
            }
        }
    }

    process_expression(
        right,
        source,
        file_path,
        imports,
        has_wildcard,
        var_types,
        usage,
    );
}

/// Process a property setter: `var.prop = value`
fn process_attribute_setter(
    attr_node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    let object = match attr_node.child_by_field_name("object") {
        Some(n) => n,
        None => return,
    };
    let attribute = match attr_node.child_by_field_name("attribute") {
        Some(n) => n,
        None => return,
    };

    let prop_name = attribute.utf8_text(source).unwrap_or("").to_string();
    if prop_name.is_empty() {
        return;
    }

    if let Some(class_name) = resolve_object_type(object, source, imports, has_wildcard, var_types)
    {
        record_member_node(
            usage,
            &class_name,
            MemberRef::PropertySetter(prop_name),
            file_path,
            attr_node,
        );
    }
}

/// Process an attribute access (read): `var.prop`
fn process_attribute_access(
    attr_node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    let object = match attr_node.child_by_field_name("object") {
        Some(n) => n,
        None => return,
    };
    let attribute = match attr_node.child_by_field_name("attribute") {
        Some(n) => n,
        None => return,
    };

    let prop_name = attribute.utf8_text(source).unwrap_or("").to_string();
    if prop_name.is_empty() {
        return;
    }

    process_expression(
        object,
        source,
        file_path,
        imports,
        has_wildcard,
        var_types,
        usage,
    );

    if let Some((path, _kind)) = imports.resolve_symbol_expression(attr_node, source) {
        record_symbol_node(usage, &path, SymbolUseKind::Read, file_path, attr_node);
        return;
    }

    if let Some(class_path) = imports.resolve_expression(attr_node, source) {
        usage.record_class(&class_path, file_path);
        return;
    }

    if let Some(class_path) = imports.resolve_expression(object, source) {
        let member = imports.catalog.class_read_member(&class_path, &prop_name);
        record_member_node(usage, &class_path, member, file_path, attr_node);
        return;
    }

    if imports.is_pybevy_qualified(attr_node, source)
        && attr_node
            .utf8_text(source)
            .ok()
            .is_some_and(looks_like_class_expression)
    {
        record_unresolved_node(
            usage,
            attr_node,
            source,
            file_path,
            "qualified reference is not a unique public stub class",
        );
        return;
    }

    if let Some(class_name) = resolve_object_type(object, source, imports, has_wildcard, var_types)
    {
        record_member_node(
            usage,
            &class_name,
            MemberRef::Property(prop_name),
            file_path,
            attr_node,
        );
    }
}

/// Process an expression node for class references
fn process_expression(
    node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    match node.kind() {
        "call" => {
            process_call_expression(
                node,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
        }
        "attribute" => {
            // Standalone attribute access (not a call)
            process_attribute_access(
                node,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
        }
        "identifier" => {
            let name = node.utf8_text(source).unwrap_or_default();
            if let Some((path, _kind)) = imports.symbols.get(name) {
                record_symbol_node(usage, path, SymbolUseKind::Read, file_path, node);
            }
        }
        "binary_operator" => {
            process_binary_operator(
                node,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
        }
        "unary_operator" => {
            process_unary_operator(
                node,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
        }
        "comparison" | "comparison_operator" => {
            process_comparison_operator(
                node,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                process_expression(
                    child,
                    source,
                    file_path,
                    imports,
                    has_wildcard,
                    var_types,
                    usage,
                );
            }
        }
    }
}

/// Process a call expression: `ClassName(...)`, `ClassName.method(...)`, or `var.method(...)`
fn process_call_expression(
    node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    let func = match node.child_by_field_name("function") {
        Some(n) => n,
        None => return,
    };

    match func.kind() {
        "identifier" => {
            // Direct call: `ClassName(...)` -> constructor
            let name = func.utf8_text(source).unwrap_or("");
            if let Some(class_name) = resolve_class_name(name, imports, has_wildcard) {
                record_member_node(usage, &class_name, MemberRef::Constructor, file_path, node);
            } else if let Some((path, _kind)) = imports.symbols.get(name) {
                record_symbol_node(usage, path, SymbolUseKind::Call, file_path, node);
            } else if let Some(contract_path) = imports.unresolved_classes.get(name) {
                record_unresolved_node(
                    usage,
                    func,
                    source,
                    file_path,
                    &format!("`{contract_path}` is not a unique public stub class"),
                );
            } else if has_wildcard && looks_like_class_name(name) {
                record_unresolved_node(
                    usage,
                    func,
                    source,
                    file_path,
                    "wildcard-imported name is not a unique public stub class",
                );
            }
        }
        "attribute" => {
            if let Some((path, _kind)) = imports.resolve_symbol_expression(func, source) {
                record_symbol_node(usage, &path, SymbolUseKind::Call, file_path, node);
            } else if let Some(class_path) = imports.resolve_expression(func, source) {
                record_member_node(usage, &class_path, MemberRef::Constructor, file_path, node);
            } else if let (Some(object), Some(attr)) = (
                func.child_by_field_name("object"),
                func.child_by_field_name("attribute"),
            ) {
                let method_name = attr.utf8_text(source).unwrap_or("").to_string();
                if let Some(class_name) = imports.resolve_expression(object, source) {
                    record_member_node(
                        usage,
                        &class_name,
                        MemberRef::StaticMethod(method_name),
                        file_path,
                        node,
                    );
                } else if let Some(class_name) =
                    resolve_object_type(object, source, imports, has_wildcard, var_types)
                {
                    record_member_node(
                        usage,
                        &class_name,
                        MemberRef::Method(method_name),
                        file_path,
                        node,
                    );
                } else if imports.is_pybevy_qualified(object, source)
                    && object
                        .utf8_text(source)
                        .ok()
                        .is_some_and(looks_like_class_expression)
                {
                    record_unresolved_node(
                        usage,
                        object,
                        source,
                        file_path,
                        "qualified name is not a unique public stub class",
                    );
                } else if imports.is_pybevy_qualified(func, source)
                    && looks_like_class_name(&method_name)
                {
                    record_unresolved_node(
                        usage,
                        func,
                        source,
                        file_path,
                        "qualified constructor is not a unique public stub class",
                    );
                }
            }
        }
        _ => {}
    }

    if let Some(args) = node.child_by_field_name("arguments") {
        let mut cursor = args.walk();
        for child in args.children(&mut cursor) {
            process_expression(
                child,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
        }
    }
}

fn looks_like_class_name(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

fn looks_like_class_expression(expression: &str) -> bool {
    expression
        .rsplit('.')
        .next()
        .is_some_and(looks_like_class_name)
}

fn record_unresolved_node(
    usage: &mut TestUsage,
    node: Node<'_>,
    source: &[u8],
    file_path: &Path,
    reason: &str,
) {
    usage.record_unresolved(
        node.utf8_text(source).unwrap_or("<invalid expression>"),
        reason,
        file_path,
        node.start_position().row + 1,
    );
}

/// Map binary operator token to dunder method name
fn binary_op_to_dunder(op: &str) -> Option<&'static str> {
    match op {
        "+" => Some("__add__"),
        "-" => Some("__sub__"),
        "*" => Some("__mul__"),
        "/" => Some("__truediv__"),
        "//" => Some("__floordiv__"),
        "%" => Some("__mod__"),
        "**" => Some("__pow__"),
        "|" => Some("__or__"),
        "&" => Some("__and__"),
        "^" => Some("__xor__"),
        "<<" => Some("__lshift__"),
        ">>" => Some("__rshift__"),
        "@" => Some("__matmul__"),
        _ => None,
    }
}

/// Map comparison operator to dunder method name
fn comparison_op_to_dunder(op: &str) -> Option<&'static str> {
    match op {
        "==" => Some("__richcmp__"),
        "!=" => Some("__richcmp__"),
        "<" => Some("__richcmp__"),
        ">" => Some("__richcmp__"),
        "<=" => Some("__richcmp__"),
        ">=" => Some("__richcmp__"),
        _ => None,
    }
}

/// Process binary operator: `a + b` → record __add__ on left operand's type
fn process_binary_operator(
    node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");
    let operator = node.child_by_field_name("operator");

    if let (Some(left), Some(op_node)) = (left, operator) {
        let op_text = op_node.utf8_text(source).unwrap_or("");
        if let Some(dunder) = binary_op_to_dunder(op_text) {
            if let Some(class_name) =
                resolve_object_type(left, source, imports, has_wildcard, var_types)
            {
                record_member_node(
                    usage,
                    &class_name,
                    MemberRef::Method(dunder.to_string()),
                    file_path,
                    node,
                );
            }
            // Also check for __rmul__ etc. on right operand
            if let Some(right) = right
                && let Some(class_name) =
                    resolve_object_type(right, source, imports, has_wildcard, var_types)
            {
                let rmul = format!("__r{}__", &dunder[2..dunder.len() - 2]);
                record_member_node(usage, &class_name, MemberRef::Method(rmul), file_path, node);
            }
        }
    }

    if let Some(left) = node.child_by_field_name("left") {
        process_expression(
            left,
            source,
            file_path,
            imports,
            has_wildcard,
            var_types,
            usage,
        );
    }
    if let Some(right) = node.child_by_field_name("right") {
        process_expression(
            right,
            source,
            file_path,
            imports,
            has_wildcard,
            var_types,
            usage,
        );
    }
}

/// Process unary operator: `-a` → record __neg__ on operand's type
fn process_unary_operator(
    node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    let operator = node.child_by_field_name("operator");
    let argument = node.child_by_field_name("argument");

    if let (Some(op_node), Some(arg)) = (operator, argument) {
        let op_text = op_node.utf8_text(source).unwrap_or("");
        let dunder = match op_text {
            "-" => Some("__neg__"),
            "~" => Some("__invert__"),
            _ => None,
        };
        if let Some(dunder) = dunder
            && let Some(class_name) =
                resolve_object_type(arg, source, imports, has_wildcard, var_types)
        {
            record_member_node(
                usage,
                &class_name,
                MemberRef::Method(dunder.to_string()),
                file_path,
                node,
            );
        }
    }

    if let Some(arg) = node.child_by_field_name("argument") {
        process_expression(
            arg,
            source,
            file_path,
            imports,
            has_wildcard,
            var_types,
            usage,
        );
    }
}

/// Process comparison: `a == b` → record __richcmp__ (or __eq__) on left operand's type
fn process_comparison_operator(
    node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    // comparison node children: expr, operator, expr [, operator, expr ...]
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    let mut expr_idx = 0;
    for child in &children {
        if child.kind() == "!="
            || child.kind() == "=="
            || child.kind() == "<"
            || child.kind() == ">"
            || child.kind() == "<="
            || child.kind() == ">="
            || child.kind() == "is"
            || child.kind() == "not"
            || child.kind() == "in"
        {
            let op_text = child.utf8_text(source).unwrap_or("");
            if let Some(dunder) = comparison_op_to_dunder(op_text) {
                if expr_idx > 0 {
                    let left = &children[expr_idx - 1];
                    if let Some(class_name) =
                        resolve_object_type(*left, source, imports, has_wildcard, var_types)
                    {
                        record_member_node(
                            usage,
                            &class_name,
                            MemberRef::Method(dunder.to_string()),
                            file_path,
                            node,
                        );
                    }
                }
            }
        } else {
            expr_idx += 1;
        }
    }

    for child in &children {
        if child.kind() != "!="
            && child.kind() != "=="
            && child.kind() != "<"
            && child.kind() != ">"
            && child.kind() != "<="
            && child.kind() != ">="
            && child.kind() != "is"
            && child.kind() != "not"
            && child.kind() != "in"
        {
            process_expression(
                *child,
                source,
                file_path,
                imports,
                has_wildcard,
                var_types,
                usage,
            );
        }
    }
}

/// Resolve a name to a class name using imports
fn resolve_class_name(
    name: &str,
    imports: &ImportBindings<'_>,
    _has_wildcard: bool,
) -> Option<ApiPath> {
    imports.get(name).cloned()
}

/// Resolve the type of an object expression
fn resolve_object_type(
    node: Node,
    source: &[u8],
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &HashMap<String, Vec<ApiPath>>,
) -> Option<ApiPath> {
    match node.kind() {
        "identifier" => {
            let name = node.utf8_text(source).unwrap_or("");
            let _ = (imports, has_wildcard);
            var_types.get(name).and_then(|types| types.last().cloned())
        }
        "attribute" => resolve_attribute_value_type(node, source, imports, var_types),
        "call" => resolve_call_result_type(node, source, imports, var_types),
        _ => None,
    }
}

fn resolve_attribute_value_type(
    node: Node<'_>,
    source: &[u8],
    imports: &ImportBindings<'_>,
    var_types: &HashMap<String, Vec<ApiPath>>,
) -> Option<ApiPath> {
    let object = node.child_by_field_name("object")?;
    let attribute = node
        .child_by_field_name("attribute")?
        .utf8_text(source)
        .ok()?;
    if let Some(owner) = imports.resolve_expression(object, source) {
        imports.catalog.resolve_class_attr_type(&owner, attribute)
    } else {
        let owner = resolve_object_type(object, source, imports, false, var_types)?;
        imports.catalog.resolve_property_type(&owner, attribute)
    }
}

fn resolve_value_type(
    node: Node<'_>,
    source: &[u8],
    imports: &ImportBindings<'_>,
    var_types: &HashMap<String, Vec<ApiPath>>,
) -> Option<ApiPath> {
    match node.kind() {
        "call" => resolve_call_result_type(node, source, imports, var_types),
        "identifier" => resolve_object_type(node, source, imports, false, var_types),
        "attribute" => resolve_attribute_value_type(node, source, imports, var_types),
        "subscript" => {
            let value = node
                .child_by_field_name("value")
                .or_else(|| node.named_child(0))?;
            let variable = value.utf8_text(source).ok()?;
            var_types
                .get(&format!("__container_type:{variable}"))
                .and_then(|types| types.last())
                .cloned()
        }
        _ => None,
    }
}

fn resolve_call_result_type(
    call_node: Node,
    source: &[u8],
    imports: &ImportBindings<'_>,
    var_types: &HashMap<String, Vec<ApiPath>>,
) -> Option<ApiPath> {
    let func = call_node.child_by_field_name("function")?;

    if let Some(class_path) = imports.resolve_expression(func, source) {
        return Some(class_path);
    }

    match func.kind() {
        "identifier" => {
            let name = func.utf8_text(source).unwrap_or("");
            resolve_class_name(name, imports, false)
        }
        "attribute" => {
            let object = func.child_by_field_name("object")?;
            let method = func
                .child_by_field_name("attribute")?
                .utf8_text(source)
                .ok()?;
            if let Some(owner) = imports.resolve_expression(object, source) {
                imports.catalog.resolve_method_return(&owner, method, true)
            } else {
                let owner = resolve_object_type(object, source, imports, false, var_types)?;
                imports.catalog.resolve_method_return(&owner, method, false)
            }
        }
        _ => None,
    }
}

fn resolve_collection_element_type(
    node: Node<'_>,
    source: &[u8],
    imports: &ImportBindings<'_>,
    var_types: &HashMap<String, Vec<ApiPath>>,
) -> Option<ApiPath> {
    if !matches!(node.kind(), "list" | "tuple" | "set") {
        return None;
    }
    let mut element_type = None;
    let mut cursor = node.walk();
    for element in node.named_children(&mut cursor) {
        let current = resolve_value_type(element, source, imports, var_types)?;
        if element_type.as_ref().is_some_and(|known| known != &current) {
            return None;
        }
        element_type = Some(current);
    }
    element_type
}

/// Process function parameters with type annotations (Query/Res patterns)
fn process_function_params(
    func_node: Node,
    source: &[u8],
    imports: &ImportBindings<'_>,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
) {
    let params = match func_node.child_by_field_name("parameters") {
        Some(n) => n,
        None => return,
    };

    let mut cursor = params.walk();
    for param in params.children(&mut cursor) {
        if param.kind() != "typed_parameter" && param.kind() != "typed_default_parameter" {
            continue;
        }

        // tree-sitter-python uses positional children, not named fields for typed_parameter
        let mut inner_cursor = param.walk();
        let children: Vec<_> = param.children(&mut inner_cursor).collect();

        let param_name = children
            .iter()
            .find(|c| c.kind() == "identifier")
            .and_then(|n| n.utf8_text(source).ok())
            .unwrap_or("");

        let type_text = children
            .iter()
            .find(|c| c.kind() == "type")
            .and_then(|n| n.utf8_text(source).ok())
            .map(|s| s.trim().to_string());

        if param_name.is_empty() || type_text.is_none() {
            continue;
        }

        let type_text = type_text.unwrap();

        // Parse Res[Time] -> track param as Time
        // Parse ResMut[Assets[StandardMaterial]] -> track param as Assets
        // Parse Query[Transform] -> track variable as Transform (for for-loop)
        // Parse Query[Mut[Transform]] -> track variable as Transform
        // Parse Query[tuple[Transform, Velocity]] -> track as [Transform, Velocity]
        if type_text.starts_with("Res[") || type_text.starts_with("ResMut[") {
            let inner = extract_bracket_content(&type_text);
            if let Some(inner) = inner {
                let class = strip_wrapper(&inner);
                if !class.is_empty()
                    && !is_type_wrapper(&class)
                    && let Some(resolved) = imports.get(&class).cloned()
                {
                    var_types
                        .entry(param_name.to_string())
                        .or_default()
                        .push(resolved);
                }
            }
        } else if type_text.starts_with("Query[") || type_text.starts_with("View[") {
            let inner = extract_bracket_content(&type_text);
            if let Some(inner) = inner {
                // Extract the component types (first part before filter)
                // Use bracket-aware split to separate components from filters
                let parts = split_type_list(&inner);
                let component_part = parts.first().map(|s| s.trim()).unwrap_or(&inner);
                let types = extract_component_types(component_part, imports);
                if !types.is_empty() {
                    // Store as __query_type:{param_name} for for-loop resolution
                    var_types
                        .entry(format!("__query_type:{}", param_name))
                        .or_default()
                        .extend(types);
                }
            }
        } else if !type_text.contains('[') && !is_type_wrapper(&type_text) {
            // Bare type annotation: `commands: Commands`, `time: Time`
            if let Some(resolved) = imports.get(&type_text).cloned() {
                var_types
                    .entry(param_name.to_string())
                    .or_default()
                    .push(resolved);
            }
        }
    }
}

/// Extract content inside brackets: `Foo[bar]` -> `bar`
fn extract_bracket_content(s: &str) -> Option<String> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    if end > start {
        Some(s[start + 1..end].trim().to_string())
    } else {
        None
    }
}

/// Strip wrapper types like Mut[T] -> T
fn strip_wrapper(s: &str) -> String {
    let s = s.trim();
    if s.starts_with("Mut[") && s.ends_with(']') {
        s[4..s.len() - 1].trim().to_string()
    } else {
        s.to_string()
    }
}

/// Extract component types from a query component specification
fn extract_component_types(spec: &str, imports: &ImportBindings<'_>) -> Vec<ApiPath> {
    let spec = spec.trim();

    // Handle tuple[A, B, C]
    if spec.starts_with("tuple[") && spec.ends_with(']') {
        let inner = &spec[6..spec.len() - 1];
        return split_type_list(inner)
            .into_iter()
            .map(|t| strip_wrapper(t.trim()))
            .filter(|t| !t.is_empty() && !is_type_wrapper(t))
            .filter_map(|name| imports.get(&name).cloned())
            .collect();
    }

    // Single type, possibly wrapped in Mut[]
    let t = strip_wrapper(spec);
    if !t.is_empty() && !is_type_wrapper(&t) {
        imports.get(&t).cloned().into_iter().collect()
    } else {
        vec![]
    }
}

/// Split a comma-separated type list, respecting bracket nesting
fn split_type_list(s: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (i, c) in s.char_indices() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth -= 1,
            ',' if depth == 0 => {
                result.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = s[start..].trim();
    if !last.is_empty() {
        result.push(last);
    }
    result
}

/// Process a for loop to track iteration variable types
fn process_for_loop(
    node: Node,
    source: &[u8],
    file_path: &Path,
    imports: &ImportBindings<'_>,
    has_wildcard: bool,
    var_types: &mut HashMap<String, Vec<ApiPath>>,
    usage: &mut TestUsage,
) {
    let left = node.child_by_field_name("left");
    let right = node.child_by_field_name("right");

    if let (Some(left), Some(right)) = (left, right) {
        // Resolve the iterable type
        if right.kind() == "identifier" {
            let iter_name = right.utf8_text(source).unwrap_or("");
            let query_key = format!("__query_type:{}", iter_name);

            if let Some(types) = var_types.get(&query_key).cloned() {
                match left.kind() {
                    "identifier" => {
                        // `for t in query:` -> t gets first type
                        let var_name = left.utf8_text(source).unwrap_or("").to_string();
                        if !var_name.is_empty()
                            && let Some(first_type) = types.first()
                        {
                            var_types
                                .entry(var_name)
                                .or_default()
                                .push(first_type.clone());
                        }
                    }
                    "pattern_list" | "tuple_pattern" => {
                        // `for a, b in query:` -> destructure
                        let mut cursor = left.walk();
                        let mut idx = 0;
                        for child in left.children(&mut cursor) {
                            if child.kind() == "identifier" {
                                let var_name = child.utf8_text(source).unwrap_or("").to_string();
                                if !var_name.is_empty() && idx < types.len() {
                                    var_types
                                        .entry(var_name)
                                        .or_default()
                                        .push(types[idx].clone());
                                }
                                idx += 1;
                            }
                        }
                    }
                    _ => {}
                }
            } else if left.kind() == "identifier"
                && let Some(element_type) = var_types
                    .get(&format!("__container_type:{iter_name}"))
                    .and_then(|types| types.last())
                    .cloned()
            {
                let variable = left.utf8_text(source).unwrap_or_default();
                if !variable.is_empty() {
                    var_types
                        .entry(variable.to_string())
                        .or_default()
                        .push(element_type);
                }
            }
        }
    }

    if let Some(body) = node.child_by_field_name("body") {
        extract_references_children(
            body,
            source,
            file_path,
            imports,
            has_wildcard,
            var_types,
            usage,
        );
    }
}
