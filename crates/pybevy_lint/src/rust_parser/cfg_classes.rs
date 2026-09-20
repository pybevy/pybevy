use anyhow::{Result, bail};
use syn::{Attribute, ImplItem, ItemImpl, Type};

use crate::model::PyClassDef;

fn predicates(attributes: &[Attribute]) -> Vec<String> {
    let mut predicates: Vec<_> = attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .map(|attribute| quote::quote!(#attribute).to_string())
        .collect();
    predicates.sort();
    predicates
}

pub(super) fn matching_classes(
    implementation: &ItemImpl,
    classes: &[PyClassDef],
    declarations: &[&[Attribute]],
) -> Result<Vec<usize>> {
    let Type::Path(path) = &*implementation.self_ty else {
        return Ok(Vec::new());
    };
    let Some(name) = path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
    else {
        return Ok(Vec::new());
    };
    let candidates: Vec<_> = classes
        .iter()
        .enumerate()
        .filter_map(|(index, class)| (class.rust_name == name).then_some(index))
        .collect();
    if candidates.len() < 2 {
        return Ok(candidates);
    }
    if implementation
        .attrs
        .iter()
        .any(|attr| attr.path().is_ident("cfg_attr"))
        || candidates.iter().any(|index| {
            declarations[*index]
                .iter()
                .any(|attr| attr.path().is_ident("cfg_attr"))
        })
    {
        bail!(
            "cannot route cfg_attr-gated duplicate class '{name}'; explicit cfg gates are required"
        );
    }
    let conditions: Vec<_> = candidates
        .iter()
        .map(|index| predicates(declarations[*index]))
        .collect();
    for (index, condition) in conditions.iter().enumerate() {
        if condition.is_empty() || conditions[..index].contains(condition) {
            bail!("ambiguous cfg declarations for duplicate class '{name}'");
        }
    }
    for member in &implementation.items {
        let attributes = match member {
            ImplItem::Fn(method) => &method.attrs,
            ImplItem::Const(constant) => &constant.attrs,
            _ => continue,
        };
        if attributes
            .iter()
            .any(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        {
            bail!(
                "cannot route cfg-gated impl members for duplicate class '{name}'; use separately gated impl blocks"
            );
        }
    }
    let condition = predicates(&implementation.attrs);
    if condition.is_empty() {
        return Ok(candidates);
    }
    let matching: Vec<_> = candidates
        .into_iter()
        .zip(conditions)
        .filter_map(|(index, declared)| (declared == condition).then_some(index))
        .collect();
    if matching.len() != 1 {
        bail!(
            "cannot route impl for duplicate class '{name}': cfg gate must exactly match one declaration"
        );
    }
    Ok(matching)
}
