use std::path::Path;

use anyhow::Result;
use tree_sitter::{Node, Parser};

use super::method_def::{parse_function_def, parse_property_annotation};
use crate::model::{ClassAttrDef, EnumVariantDef, EnumVariantKind, PyClassDef, SourceLocation};

/// Parse a Python stub file and extract class definitions
pub fn parse_stub_file(content: &str, path: &Path) -> Result<Vec<PyClassDef>> {
    let mut parser = Parser::new();
    let language = tree_sitter_python::LANGUAGE;
    parser.set_language(&language.into())?;

    let tree = parser
        .parse(content, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse Python file"))?;

    let root = tree.root_node();
    let mut classes = Vec::new();

    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "class_definition" {
            if let Some(class) = parse_class_def(&child, content, path) {
                classes.push(class);
            }
        } else if child.kind() == "decorated_definition" {
            // Handle decorated classes (e.g., @final class Foo)
            let mut inner_cursor = child.walk();
            for inner_child in child.children(&mut inner_cursor) {
                if inner_child.kind() == "class_definition"
                    && let Some(class) = parse_class_def(&inner_child, content, path)
                {
                    classes.push(class);
                }
            }
        }
    }

    Ok(classes)
}

/// Parse a class definition node
fn parse_class_def(node: &Node, source: &str, path: &Path) -> Option<PyClassDef> {
    let mut class = PyClassDef::default();

    let name_node = node.child_by_field_name("name")?;
    class.python_name = node_text(&name_node, source).to_string();
    class.rust_name = format!("Py{}", class.python_name); // Assume Rust name has Py prefix

    class.location = Some(SourceLocation {
        file: path.to_path_buf(),
        line: node.start_position().row + 1,
        column: node.start_position().column,
    });

    if let Some(superclasses) = node.child_by_field_name("superclasses") {
        class.extends = extract_first_base_class(&superclasses, source);
    }

    let body = node.child_by_field_name("body")?;

    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                parse_function_in_class(&child, source, path, &mut class);
            }
            "decorated_definition" => {
                // Handle decorated functions
                parse_decorated_definition(&child, source, path, &mut class);
            }
            "expression_statement" => {
                // Handle property annotations like `x: float`
                if let Some((name, ty)) = parse_property_annotation(&child, source) {
                    let is_self_preset = ty
                        .split(|character: char| !character.is_alphanumeric() && character != '_')
                        .any(|part| part == class.python_name);
                    let is_class_attribute = ty.starts_with("ClassVar")
                        || ty.starts_with("Final")
                        || is_self_preset
                        || name.to_uppercase() == name;
                    if is_class_attribute {
                        class.class_attrs.push(ClassAttrDef {
                            name,
                            attr_type: extract_classvar_type(&ty).or(Some(ty)),
                            location: Some(SourceLocation {
                                file: path.to_path_buf(),
                                line: child.start_position().row + 1,
                                column: child.start_position().column,
                            }),
                        });
                    } else {
                        // It's a property
                        // Check if we already have this property
                        if !class.properties.iter().any(|p| p.name == name) {
                            class.properties.push(crate::model::PropertyDef {
                                name,
                                property_type: Some(ty),
                                has_getter: true, // Assume readable in .pyi
                                has_setter: true, // Assume writable unless marked read-only
                                ..Default::default()
                            });
                        }
                    }
                } else if let Some(assignment) = child.named_child(0)
                    && assignment.kind() == "assignment"
                    && let Some(left) = assignment.child_by_field_name("left")
                    && left.kind() == "identifier"
                    && let Ok(name) = left.utf8_text(source.as_bytes())
                    && !name.starts_with('_')
                {
                    class.class_attrs.push(ClassAttrDef {
                        name: name.to_string(),
                        location: Some(SourceLocation {
                            file: path.to_path_buf(),
                            line: child.start_position().row + 1,
                            column: child.start_position().column,
                        }),
                        ..Default::default()
                    });
                }
            }
            "class_definition" => {
                if let Some(variant) = parse_variant_def(&child, &class.python_name, source, path) {
                    class.is_enum = true;
                    class.enum_variants.push(variant);
                } else if let Some(name) = child
                    .child_by_field_name("name")
                    .and_then(|name| name.utf8_text(source.as_bytes()).ok())
                    .filter(|name| !name.starts_with('_'))
                {
                    class.nested_types.push(name.to_string());
                }
            }
            _ => {}
        }
    }

    Some(class)
}

fn parse_variant_def(
    node: &Node<'_>,
    outer_class_name: &str,
    source: &str,
    path: &Path,
) -> Option<EnumVariantDef> {
    let superclasses = node.child_by_field_name("superclasses")?;
    if extract_first_base_class(&superclasses, source).as_deref() != Some(outer_class_name) {
        return None;
    }

    let name = node_text(&node.child_by_field_name("name")?, source).to_string();
    if name.starts_with('_') {
        return None;
    }

    let body = node.child_by_field_name("body")?;
    let mut fields = Vec::new();
    let mut constructor = None;
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "function_definition"
            && child
                .child_by_field_name("name")
                .is_some_and(|name| node_text(&name, source) == "__init__")
        {
            constructor = parse_function_def(&child, source, path, &[]);
            continue;
        }
        if child.kind() != "expression_statement" {
            continue;
        }
        let Some((field_name, field_type)) = parse_property_annotation(&child, source) else {
            continue;
        };
        if field_name.starts_with('_')
            || field_type.starts_with("ClassVar")
            || field_type.starts_with("Final")
        {
            continue;
        }
        fields.push((field_name, field_type));
    }

    let kind = match fields.as_slice() {
        [] => EnumVariantKind::EmptyTuple,
        [(field_name, field_type)] if field_name == "value" => {
            EnumVariantKind::Tuple(vec![field_type.clone()])
        }
        _ => EnumVariantKind::Struct(fields),
    };
    Some(EnumVariantDef {
        name,
        kind,
        constructor,
    })
}

/// Extract the first base class name
fn extract_first_base_class(superclasses: &Node, source: &str) -> Option<String> {
    let mut cursor = superclasses.walk();
    for child in superclasses.children(&mut cursor) {
        if let Some(base) = extract_base_class_name(&child, source) {
            return Some(base);
        }
    }
    None
}

fn extract_base_class_name(node: &Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" | "attribute" => Some(node_text(node, source).to_string()),
        "subscript" => node
            .child_by_field_name("value")
            .and_then(|value| extract_base_class_name(&value, source)),
        _ => None,
    }
}

/// Parse a function definition in a class context
fn parse_function_in_class(node: &Node, source: &str, path: &Path, class: &mut PyClassDef) {
    if let Some(method) = parse_function_def(node, source, path, &[]) {
        if method.name == "__init__" {
            class.constructor = Some(method);
        } else if method.is_static {
            class.static_methods.push(method);
        } else {
            class.methods.push(method);
        }
    }
}

/// Parse a decorated definition (function with decorators)
fn parse_decorated_definition(node: &Node, source: &str, path: &Path, class: &mut PyClassDef) {
    let mut decorators = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "decorator" {
            decorators.push(node_text(&child, source).to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_definition"
            && let Some(method) = parse_function_def(&child, source, path, &decorators)
        {
            // Check decorators to determine method type
            let is_property = decorators.iter().any(|d| d.contains("@property"));
            let is_setter = decorators
                .iter()
                .any(|d| d.contains(".setter") || d.ends_with("setter"));
            let is_staticmethod = decorators.iter().any(|d| d.contains("@staticmethod"));
            let is_classmethod = decorators.iter().any(|d| d.contains("@classmethod"));
            let is_overload = decorators.iter().any(|d| d.contains("@overload"));

            if is_overload {
                // Add as a regular method (first overload wins)
                if !class.methods.iter().any(|m| m.name == method.name)
                    && !class.static_methods.iter().any(|m| m.name == method.name)
                {
                    if is_staticmethod {
                        class.static_methods.push(method);
                    } else {
                        class.methods.push(method);
                    }
                }
            } else if is_property {
                // It's a property getter
                let prop_name = method.name.clone();
                if let Some(prop) = class.properties.iter_mut().find(|p| p.name == prop_name) {
                    prop.has_getter = true;
                    if prop.property_type.is_none() {
                        prop.property_type = method.return_type;
                    }
                } else {
                    class.properties.push(crate::model::PropertyDef {
                        name: prop_name,
                        property_type: method.return_type,
                        has_getter: true,
                        has_setter: false,
                        ..Default::default()
                    });
                }
            } else if is_setter {
                // It's a property setter
                let prop_name = method.name.clone();
                if let Some(prop) = class.properties.iter_mut().find(|p| p.name == prop_name) {
                    prop.has_setter = true;
                } else {
                    let setter_type = method.parameters.first().and_then(|p| p.param_type.clone());
                    class.properties.push(crate::model::PropertyDef {
                        name: prop_name,
                        property_type: setter_type,
                        has_getter: false,
                        has_setter: true,
                        ..Default::default()
                    });
                }
            } else if method.name == "__init__" {
                class.constructor = Some(method);
            } else if is_staticmethod || is_classmethod {
                class.static_methods.push(method);
            } else {
                class.methods.push(method);
            }
        }
    }
}

/// Extract type from ClassVar[T]
fn extract_classvar_type(ty: &str) -> Option<String> {
    let ty = ty.trim();
    if ty.starts_with("ClassVar[") && ty.ends_with(']') {
        Some(ty[9..ty.len() - 1].to_string())
    } else {
        None
    }
}

/// Get the text content of a node
fn node_text<'a>(node: &Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}
