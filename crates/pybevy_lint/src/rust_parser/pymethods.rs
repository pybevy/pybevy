use std::path::Path;

use syn::{Attribute, FnArg, ImplItem, ItemImpl, Pat, ReturnType, Type};

use super::{macros::parse_pyo3_attr, signature::parse_signature_attr, types::type_to_string};
use crate::model::{
    ClassAttrDef, GetterResultClassification, MethodDef, ParameterDef, ParameterKind, PropertyDef,
    PyClassDef, SelfMutability, SourceLocation,
};

/// Process an impl block and attach methods to the appropriate class
pub fn process_impl_block(item_impl: &ItemImpl, classes: &mut [PyClassDef], file_path: &Path) {
    let is_pymethods = item_impl
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("pymethods"));

    if !is_pymethods {
        return;
    }

    let Type::Path(type_path) = &*item_impl.self_ty else {
        return;
    };

    let rust_name = type_path
        .path
        .segments
        .last()
        .map(|s| s.ident.to_string())
        .unwrap_or_default();

    let Some(class) = classes.iter_mut().find(|c| c.rust_name == rust_name) else {
        return;
    };

    for item in &item_impl.items {
        match item {
            ImplItem::Fn(method) => {
                process_method(method, class, file_path);
            }
            ImplItem::Const(const_item) => {
                process_const(const_item, class, file_path);
            }
            _ => {}
        }
    }
}

/// Process a const item in a pymethods block (for #[classattr] consts)
fn process_const(const_item: &syn::ImplItemConst, class: &mut PyClassDef, file_path: &Path) {
    let is_classattr = has_attr(&const_item.attrs, "classattr");
    if !is_classattr {
        return;
    }

    let name = const_item.ident.to_string();
    let attr_type = Some(type_to_string(&const_item.ty));
    let location = SourceLocation {
        file: file_path.to_path_buf(),
        line: const_item.ident.span().start().line,
        column: const_item.ident.span().start().column,
    };

    class.class_attrs.push(ClassAttrDef {
        name,
        attr_type,
        location: Some(location),
    });
}

/// Check if a method body uses borrow_field, borrow_field_as, or borrow_optional_field pattern
fn uses_borrow_field_pattern(method: &syn::ImplItemFn) -> bool {
    let body = quote::quote!(#method).to_string();
    body.contains("borrow_field")
        || body.contains("borrow_field_as")
        || body.contains("borrow_optional_field")
        || body.contains("borrow_resolved")
        || body.contains("borrow_asset_field")
        || body.contains("from_component_field")
        || body.contains("from_resource_field")
        || body.contains("PyAnimationGraphNode :: view")
        || body.contains("try_acquire_read_view")
        || body.contains("prepare_write_view")
}

fn result_classification(method: &syn::ImplItemFn) -> GetterResultClassification {
    let body = quote::quote!(#method).to_string();
    if uses_borrow_field_pattern(method) {
        GetterResultClassification::Live
    } else if body.contains("snapshot_field")
        || body.contains("snapshot_field_as")
        || body.contains("read_only_snapshot")
        || body.contains("from_snapshot")
    {
        GetterResultClassification::ReadOnlySnapshot
    } else if body.contains("computed_owned") {
        GetterResultClassification::ComputedOwned
    } else {
        GetterResultClassification::Unclassified
    }
}

fn result_is_derived_from_self(method: &syn::ImplItemFn) -> bool {
    if parse_self_mutability(&method.sig) == SelfMutability::None {
        return false;
    }
    let block = &method.block;
    let body = quote::quote!(#block).to_string();
    body.contains("self . as_ref")
        || body.contains("self . storage")
        || body.contains("self . borrow")
        || body.contains("slf . borrow")
        || body.contains("slf . as_ref")
}

/// Process a single method in a pymethods block
fn process_method(method: &syn::ImplItemFn, class: &mut PyClassDef, file_path: &Path) {
    let is_new = has_attr(&method.attrs, "new");
    let getter_name = get_attr_arg(&method.attrs, "getter");
    let setter_name = get_attr_arg(&method.attrs, "setter");
    let is_getter = getter_name.is_some() || has_attr(&method.attrs, "getter");
    let is_setter = setter_name.is_some() || has_attr(&method.attrs, "setter");
    let is_staticmethod = has_attr(&method.attrs, "staticmethod");
    let is_classmethod = has_attr(&method.attrs, "classmethod");
    let is_classattr = has_attr(&method.attrs, "classattr");

    // PyO3 exposes all #[pymethods] items to Python regardless of visibility

    let location = SourceLocation {
        file: file_path.to_path_buf(),
        line: method.sig.ident.span().start().line,
        column: method.sig.ident.span().start().column,
    };

    let uses_borrow = is_getter && uses_borrow_field_pattern(method);

    let pyo3_args = method
        .attrs
        .iter()
        .find_map(parse_pyo3_attr)
        .unwrap_or_default();

    let signature_info = method.attrs.iter().find_map(parse_signature_attr);

    let rust_name = method.sig.ident.to_string();

    let rust_name_clean = rust_name
        .strip_prefix("r#")
        .unwrap_or(&rust_name)
        .to_string();

    let python_name = if let Some(name) = getter_name.clone().or(setter_name.clone()) {
        // Name from #[getter(name)] or #[setter(name)]
        name
    } else if let Some(name) = pyo3_args.name.clone() {
        // Name from #[pyo3(name = "...")]
        name
    } else if is_new {
        "__init__".to_string()
    } else if is_getter && rust_name_clean.starts_with("get_") {
        // PyO3 derives the property name by removing the conventional getter
        // prefix when #[getter] has no explicit Python name.
        rust_name_clean[4..].to_string()
    } else if is_setter && rust_name_clean.starts_with("set_") {
        // For setters without explicit name, strip set_ prefix
        rust_name_clean[4..].to_string()
    } else {
        rust_name_clean.clone()
    };

    let self_mutability = parse_self_mutability(&method.sig);

    let parameters = parse_parameters(&method.sig, &signature_info);

    let return_type = parse_return_type(&method.sig.output);

    let method_def = MethodDef {
        name: python_name.clone(),
        rust_name: rust_name_clean.clone(),
        parameters,
        return_type,
        is_static: is_staticmethod,
        is_class_method: is_classmethod,
        self_mutability,
        location: Some(location.clone()),
        signature_str: signature_info.map(|s| s.raw),
        has_varargs: false,
        has_kwargs: false,
        result_classification: result_classification(method),
        result_derived_from_self: result_is_derived_from_self(method),
    };

    if is_new {
        class.constructor = Some(method_def);
    } else if is_getter {
        let prop_name = python_name.clone();
        if let Some(prop) = class.properties.iter_mut().find(|p| p.name == prop_name) {
            prop.has_getter = true;
            prop.getter_mutability = self_mutability;
            prop.getter_location = Some(location);
            prop.getter_uses_borrow = uses_borrow;
            prop.getter_result_classification = result_classification(method);
            if prop.property_type.is_none() {
                prop.property_type = method_def.return_type;
            }
        } else {
            class.properties.push(PropertyDef {
                name: prop_name,
                property_type: method_def.return_type,
                has_getter: true,
                has_setter: false,
                getter_mutability: self_mutability,
                getter_location: Some(location),
                setter_location: None,
                getter_uses_borrow: uses_borrow,
                getter_result_classification: result_classification(method),
            });
        }
    } else if is_setter {
        let prop_name = python_name.clone();
        if let Some(prop) = class.properties.iter_mut().find(|p| p.name == prop_name) {
            prop.has_setter = true;
            prop.setter_location = Some(location);
        } else {
            let setter_type = method_def
                .parameters
                .first()
                .and_then(|p| p.param_type.clone());
            class.properties.push(PropertyDef {
                name: prop_name,
                property_type: setter_type,
                has_getter: false,
                has_setter: true,
                getter_mutability: SelfMutability::None,
                getter_location: None,
                setter_location: Some(location),
                getter_uses_borrow: false, // No getter yet
                getter_result_classification: GetterResultClassification::Unclassified,
            });
        }
    } else if is_classattr {
        class.class_attrs.push(ClassAttrDef {
            name: python_name,
            attr_type: method_def.return_type,
            location: Some(location),
        });
    } else if is_staticmethod {
        class.static_methods.push(method_def);
    } else {
        class.methods.push(method_def);
    }
}

/// Check if an attribute with the given name exists
fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

/// Get the argument from an attribute like #[getter(name)] or #[setter(name)]
fn get_attr_arg(attrs: &[Attribute], name: &str) -> Option<String> {
    for attr in attrs {
        if attr.path().is_ident(name) {
            if let syn::Meta::List(meta_list) = &attr.meta {
                let tokens = meta_list.tokens.to_string();
                let trimmed = tokens.trim();
                if !trimmed.is_empty() && !trimmed.contains('=') {
                    // Simple identifier argument like #[getter(global_)]
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

/// Parse self mutability from function signature
fn parse_self_mutability(sig: &syn::Signature) -> SelfMutability {
    for arg in &sig.inputs {
        match arg {
            FnArg::Receiver(receiver) => {
                if receiver.reference.is_some() {
                    if receiver.mutability.is_some() {
                        return SelfMutability::RefMut;
                    } else {
                        return SelfMutability::Ref;
                    }
                } else {
                    return SelfMutability::Owned;
                }
            }
            FnArg::Typed(pat_type) => {
                // Check for patterns like `self: PyRefMut<'_, Self>` or `slf: Py<Self>`
                // Also handles `self_` which is used to avoid shadowing Rust's self keyword
                if let Pat::Ident(pat_ident) = &*pat_type.pat {
                    let ident = pat_ident.ident.to_string();
                    if matches!(ident.as_str(), "self" | "self_" | "pyself" | "slf") {
                        // It's a typed self parameter
                        let type_str = type_to_string(&pat_type.ty);
                        if type_str.contains("PyRefMut") {
                            return SelfMutability::RefMut;
                        } else if type_str.contains("PyRef") {
                            return SelfMutability::Ref;
                        } else {
                            // Py<Self> or other owned patterns
                            return SelfMutability::Owned;
                        }
                    }
                }
            }
        }
    }
    SelfMutability::None
}

/// Parse parameters from function signature
fn parse_parameters(
    sig: &syn::Signature,
    signature_info: &Option<SignatureInfo>,
) -> Vec<ParameterDef> {
    let mut params = Vec::new();

    for arg in &sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            // Skip self parameters
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                let name = pat_ident.ident.to_string();

                // Skip special PyO3 parameters that aren't visible to Python
                // - self, self_, pyself: standard self receivers (self_ avoids Rust keyword)
                // - slf: Py<Self> receiver pattern (owned self for method chaining)
                // - py, _py: Python GIL token (underscore variant for unused)
                // - _cls, cls: class method receiver
                if matches!(
                    name.as_str(),
                    "self" | "self_" | "pyself" | "slf" | "py" | "_py" | "_cls" | "cls"
                ) {
                    continue;
                }

                let param_type = type_to_string(&pat_type.ty);
                let is_optional = param_type.starts_with("Option <")
                    || param_type.starts_with("Option<")
                    || param_type.contains("Option <")
                    || param_type.contains("Option<");

                let default_value = signature_info
                    .as_ref()
                    .and_then(|s| s.defaults.get(&name).cloned());

                params.push(ParameterDef {
                    name,
                    param_type: Some(param_type),
                    default_value,
                    is_optional,
                    kind: ParameterKind::PositionalOrKeyword,
                });
            }
        }
    }

    params
}

/// Parse return type
fn parse_return_type(output: &ReturnType) -> Option<String> {
    match output {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => Some(type_to_string(ty)),
    }
}

/// Signature info extracted from #[pyo3(signature = (...))]
pub struct SignatureInfo {
    pub raw: String,
    pub defaults: std::collections::HashMap<String, String>,
}
