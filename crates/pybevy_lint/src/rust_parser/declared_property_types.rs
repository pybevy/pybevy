use std::collections::{HashMap, hash_map::Entry};

use quote::quote;
use syn::{GenericArgument, Ident, PathArguments, Type, parse_str};

use crate::{model::PyClassDef, rust_parser::types::normalize_rust_type};

/// Resolve before scoped validation discards other modules' class declarations.
pub(super) fn annotate_classes(classes: &mut [PyClassDef]) {
    let mut names = HashMap::new();
    for class in classes.iter() {
        let entry = names.entry(class.rust_name.clone());
        match entry {
            Entry::Occupied(mut entry) => {
                entry.insert(None);
            }
            Entry::Vacant(entry) => {
                let public = class
                    .python_module_path
                    .as_deref()
                    .is_some_and(|module| module == "pybevy" || module.starts_with("pybevy."));
                let name = &class.python_name;
                let usable = public
                    && !name.starts_with('_')
                    && normalize_rust_type(name) == *name
                    && parse_str::<Ident>(name).is_ok();
                entry.insert(usable.then(|| name.clone()));
            }
        }
    }
    for class in classes {
        for property in &mut class.properties {
            property.declared_property_type = property.property_type.as_deref().and_then(|raw| {
                let mut ty = parse_str::<Type>(raw).ok()?;
                let changed = rewrite(&mut ty, &names);
                changed.then(|| quote!(#ty).to_string())
            });
        }
    }
}

fn rewrite(ty: &mut Type, names: &HashMap<String, Option<String>>) -> bool {
    match ty {
        Type::Path(path) if path.qself.is_none() => {
            let unqualified = path.path.leading_colon.is_none() && path.path.segments.len() == 1;
            let Some(segment) = path.path.segments.last_mut() else {
                return false;
            };
            let mut changed = false;
            if let PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
                for argument in &mut arguments.args {
                    if let GenericArgument::Type(ty) = argument {
                        changed |= rewrite(ty, names);
                    }
                }
            }
            if unqualified
                && matches!(segment.arguments, PathArguments::None)
                && let Some(Some(name)) = names.get(&segment.ident.to_string())
            {
                segment.ident = Ident::new(name, segment.ident.span());
                changed = true;
            }
            changed
        }
        Type::Reference(reference) => rewrite(&mut reference.elem, names),
        Type::Array(array) => rewrite(&mut array.elem, names),
        Type::Slice(slice) => rewrite(&mut slice.elem, names),
        Type::Paren(paren) => rewrite(&mut paren.elem, names),
        Type::Group(group) => rewrite(&mut group.elem, names),
        Type::Tuple(tuple) => tuple
            .elems
            .iter_mut()
            .fold(false, |changed, element| rewrite(element, names) | changed),
        _ => false,
    }
}
