use std::path::Path;

use tree_sitter::Node;

use crate::model::{MethodDef, ParameterDef, ParameterKind, SelfMutability, SourceLocation};

/// Parse a function definition node
pub fn parse_function_def(
    node: &Node,
    source: &str,
    path: &Path,
    decorators: &[String],
) -> Option<MethodDef> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(&name_node, source).to_string();

    let params_node = node.child_by_field_name("parameters")?;
    let (parameters, has_varargs, has_kwargs) = parse_parameters_with_varargs(&params_node, source);

    let return_type = node
        .child_by_field_name("return_type")
        .map(|n| node_text(&n, source).to_string());

    let is_static = decorators
        .iter()
        .any(|d| d.contains("@staticmethod") || d.contains("@classmethod"));

    let self_mutability = if parameters.is_empty() || is_static {
        SelfMutability::None
    } else if parameters
        .first()
        .is_some_and(|p| p.name == "self" || p.name == "cls")
    {
        SelfMutability::Ref // Python doesn't distinguish &self vs &mut self
    } else {
        SelfMutability::None
    };

    let parameters: Vec<_> = parameters
        .into_iter()
        .filter(|p| p.name != "self" && p.name != "cls")
        .collect();

    Some(MethodDef {
        name: name.clone(),
        rust_name: name,
        parameters,
        return_type,
        is_static,
        is_class_method: decorators.iter().any(|d| d.contains("@classmethod")),
        self_mutability,
        location: Some(SourceLocation {
            file: path.to_path_buf(),
            line: node.start_position().row + 1,
            column: node.start_position().column,
        }),
        signature_str: None,
        has_varargs,
        has_kwargs,
        result_classification: Default::default(),
        result_derived_from_self: false,
    })
}

/// Parse function parameters and detect *args/**kwargs
fn parse_parameters_with_varargs(node: &Node, source: &str) -> (Vec<ParameterDef>, bool, bool) {
    let mut params = Vec::new();
    let mut has_varargs = false;
    let mut has_kwargs = false;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        let kind = child.kind();
        match kind {
            "identifier" => {
                // Simple parameter without type annotation (e.g., "self")
                let name = node_text(&child, source).to_string();
                params.push(ParameterDef {
                    name,
                    param_type: None,
                    default_value: None,
                    is_optional: false,
                    kind: ParameterKind::PositionalOrKeyword,
                });
            }
            "typed_parameter" => {
                if let Some((param, is_var, is_kw)) = parse_typed_parameter(&child, source) {
                    // Typed varargs (*args: T) or kwargs (**kwargs: T) detected
                    if is_var {
                        has_varargs = true;
                    } else if is_kw {
                        has_kwargs = true;
                    } else {
                        params.push(param);
                    }
                }
            }
            "default_parameter" => {
                if let Some(param) = parse_default_parameter(&child, source) {
                    params.push(param);
                }
            }
            "typed_default_parameter" => {
                if let Some(param) = parse_typed_default_parameter(&child, source) {
                    params.push(param);
                }
            }
            "list_splat_pattern" => {
                // *args pattern
                has_varargs = true;
            }
            "dictionary_splat_pattern" => {
                // **kwargs pattern
                has_kwargs = true;
            }
            "(" | ")" | "," | "*" | "**" => {}
            _ => {
                // Debug: print unhandled node types
            }
        }
    }

    (params, has_varargs, has_kwargs)
}

/// Check if a typed parameter contains a varargs pattern (*args: type or **kwargs: type)
fn typed_param_splat_kind(node: &Node) -> Option<&'static str> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "list_splat_pattern" => return Some("varargs"),
            "dictionary_splat_pattern" => return Some("kwargs"),
            _ => {}
        }
    }
    None
}

/// Parse a typed parameter (name: type)
/// Returns (ParameterDef, is_varargs, is_kwargs)
fn parse_typed_parameter(node: &Node, source: &str) -> Option<(ParameterDef, bool, bool)> {
    // tree-sitter-python uses positional children, not named fields for typed_parameter
    // Structure: identifier ":" type
    // BUT for *args: type, it's: list_splat_pattern ":" type
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    let splat_kind = typed_param_splat_kind(node);
    let is_varargs = splat_kind == Some("varargs");
    let is_kwargs = splat_kind == Some("kwargs");

    // Find the identifier - it could be direct or inside a splat pattern
    let name = children.iter().find_map(|c| {
        if c.kind() == "identifier" {
            Some(node_text(c, source).to_string())
        } else if c.kind() == "list_splat_pattern" || c.kind() == "dictionary_splat_pattern" {
            let mut inner_cursor = c.walk();
            c.children(&mut inner_cursor)
                .find(|inner| inner.kind() == "identifier")
                .map(|n| node_text(&n, source).to_string())
        } else {
            None
        }
    })?;

    let param_type = children
        .iter()
        .find(|c| c.kind() == "type")
        .map(|n| node_text(n, source).to_string());

    let is_optional = param_type
        .as_ref()
        .is_some_and(|t| t.contains("| None") || t.starts_with("Optional["));

    Some((
        ParameterDef {
            name,
            param_type,
            default_value: None,
            is_optional,
            kind: ParameterKind::PositionalOrKeyword,
        },
        is_varargs,
        is_kwargs,
    ))
}

/// Parse a default parameter (name=value)
fn parse_default_parameter(node: &Node, source: &str) -> Option<ParameterDef> {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    let name = children
        .iter()
        .find(|c| c.kind() == "identifier")
        .map(|n| node_text(n, source).to_string())?;

    let default_value = children
        .iter()
        .find(|c| c.kind() != "identifier" && c.kind() != "=")
        .map(|n| node_text(n, source).to_string());

    let is_optional = default_value.as_ref().is_some_and(|v| v == "None");

    Some(ParameterDef {
        name,
        param_type: None,
        default_value,
        is_optional,
        kind: ParameterKind::PositionalOrKeyword,
    })
}

/// Parse a typed default parameter (name: type = value)
fn parse_typed_default_parameter(node: &Node, source: &str) -> Option<ParameterDef> {
    let mut cursor = node.walk();
    let children: Vec<_> = node.children(&mut cursor).collect();

    let name = children
        .iter()
        .find(|c| c.kind() == "identifier")
        .map(|n| node_text(n, source).to_string())?;

    let param_type = children
        .iter()
        .find(|c| c.kind() == "type")
        .map(|n| node_text(n, source).to_string());

    let eq_pos = children.iter().position(|c| c.kind() == "=");
    let default_value = eq_pos.and_then(|pos| {
        children
            .iter()
            .skip(pos + 1)
            .find(|c| c.kind() != "=" && c.kind() != ":" && c.kind() != ",")
            .map(|n| node_text(n, source).to_string())
    });

    let is_optional = param_type
        .as_ref()
        .is_some_and(|t| t.contains("| None") || t.starts_with("Optional["))
        || default_value.as_ref().is_some_and(|v| v == "None");

    Some(ParameterDef {
        name,
        param_type,
        default_value,
        is_optional,
        kind: ParameterKind::PositionalOrKeyword,
    })
}

/// Parse a property annotation (x: Type or x: Type = value)
pub fn parse_property_annotation(node: &Node, source: &str) -> Option<(String, String)> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "assignment" {
            // x: Type = value or x = value
            // Look for type annotation
            let left = child.child_by_field_name("left")?;
            if left.kind() == "identifier" {
                let name = node_text(&left, source).to_string();
                if let Some(type_node) = child.child_by_field_name("type") {
                    let ty = node_text(&type_node, source).to_string();
                    return Some((name, ty));
                }
            }
        } else if child.kind() == "expression" || child.kind() == "type" {
            // Could be just a type annotation: x: Type
            // This might be inside an expression_statement directly
        }
    }

    let text = node_text(node, source);

    let trimmed = text.trim();
    if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
        return None;
    }

    if text.contains('\n') {
        return None;
    }

    if let Some(colon_pos) = text.find(':') {
        let name = text[..colon_pos].trim();
        let rest = text[colon_pos + 1..].trim();

        // Skip if name looks like a docstring fragment or contains spaces
        if name.contains(' ') || name.starts_with('"') || name.starts_with('\'') {
            return None;
        }

        // Handle "name: Type" or "name: Type = value"
        let ty = if let Some(eq_pos) = rest.find('=') {
            rest[..eq_pos].trim()
        } else {
            rest.trim_end_matches("...").trim()
        };

        if !name.is_empty() && !ty.is_empty() {
            return Some((name.to_string(), ty.to_string()));
        }
    }

    None
}

/// Get the text content of a node
fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}
