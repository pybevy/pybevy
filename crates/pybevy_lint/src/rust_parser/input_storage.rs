use std::collections::{HashMap, HashSet, hash_map::Entry};

use syn::{Fields, File, GenericArgument, Item, PathArguments, Type, parse_str};

use super::types::type_to_string;
use crate::model::{EnumVariantKind, PyClassDef};

/// Checked scalar inputs compare as their stored native value, not their Python input.
pub(super) fn annotate(file: &File, classes: &mut [PyClassDef]) {
    let inputs: HashSet<_> = file
        .items
        .iter()
        .filter_map(|item| {
            let Item::Impl(block) = item else { return None };
            let (_, path, _) = block.trait_.as_ref()?;
            if path.segments.last()?.ident != "FromPyObject" {
                return None;
            }
            let Type::Path(ty) = &*block.self_ty else {
                return None;
            };
            ty.path.get_ident().map(ToString::to_string)
        })
        .collect();
    let candidates = file.items.iter().filter_map(|item| {
        let Item::Struct(item) = item else {
            return None;
        };
        let name = item.ident.to_string();
        if !inputs.contains(&name)
            || !item.generics.params.is_empty()
            || item
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("pyclass"))
        {
            return None;
        }
        let Fields::Unnamed(fields) = &item.fields else {
            return None;
        };
        (fields.unnamed.len() == 1).then(|| (name, fields.unnamed[0].ty.clone()))
    });
    let mut storage = HashMap::new();
    for (name, ty) in candidates {
        match storage.entry(name) {
            Entry::Vacant(entry) => {
                entry.insert(Some(ty));
            }
            Entry::Occupied(mut entry) => {
                entry.insert(None);
            }
        }
    }
    for class in classes {
        for variant in &mut class.enum_variants {
            match &mut variant.kind {
                EnumVariantKind::Tuple(fields) => {
                    for ty in fields {
                        rewrite_field(ty, &storage);
                    }
                }
                EnumVariantKind::Struct(fields) => {
                    for (_, ty) in fields {
                        rewrite_field(ty, &storage);
                    }
                }
                _ => {}
            }
        }
    }
}

fn rewrite_field(raw: &mut String, storage: &HashMap<String, Option<Type>>) {
    if let Ok(mut ty) = parse_str::<Type>(raw) {
        rewrite(&mut ty, storage);
        *raw = type_to_string(&ty);
    }
}

fn rewrite(ty: &mut Type, storage: &HashMap<String, Option<Type>>) {
    let Type::Path(path) = ty else { return };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return;
    }
    let segment = &mut path.path.segments[0];
    match &mut segment.arguments {
        PathArguments::None => {
            if let Some(Some(native)) = storage.get(&segment.ident.to_string()) {
                *ty = native.clone();
            }
        }
        PathArguments::AngleBracketed(arguments) => {
            for argument in &mut arguments.args {
                if let GenericArgument::Type(ty) = argument {
                    rewrite(ty, storage);
                }
            }
        }
        _ => {}
    }
}
