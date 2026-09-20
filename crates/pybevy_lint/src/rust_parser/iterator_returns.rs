use syn::{GenericArgument, PathArguments, Type};

use crate::{
    model::{PyClassDef, SelfMutability},
    rust_parser::types::normalize_rust_type,
};

fn first_type(ty: &Type) -> Option<&Type> {
    let Type::Path(path) = ty else { return None };
    let PathArguments::AngleBracketed(arguments) = &path.path.segments.last()?.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| match argument {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn leaf(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else { return None };
    Some(path.path.segments.last()?.ident.to_string())
}

fn transparent(mut ty: &Type) -> &Type {
    loop {
        ty = match ty {
            Type::Reference(reference) => &reference.elem,
            Type::Paren(paren) => &paren.elem,
            Type::Group(group) => &group.elem,
            _ if leaf(ty).is_some_and(|name| {
                matches!(
                    name.as_str(),
                    "PyResult" | "Result" | "Py" | "Bound" | "PyRef" | "PyRefMut" | "Borrowed"
                )
            }) =>
            {
                match first_type(ty) {
                    Some(inner) => inner,
                    None => return ty,
                }
            }
            _ => return ty,
        };
    }
}

fn return_name(source: &str) -> Option<String> {
    let ty = syn::parse_str::<Type>(source).ok()?;
    let ty = transparent(&ty);
    let Type::Path(path) = ty else { return None };
    let segment = path.path.segments.last()?;
    matches!(segment.arguments, PathArguments::None).then(|| segment.ident.to_string())
}

fn same_module(left: &PyClassDef, right: &PyClassDef) -> bool {
    let module = |class: &PyClassDef| {
        class.python_module_path.clone().or_else(|| {
            class
                .module_path
                .as_ref()
                .map(|module| format!("pybevy.{module}"))
        })
    };
    module(left) == module(right)
}

fn has_iterator_methods(class: &PyClassDef) -> bool {
    ["__iter__", "__next__"].iter().all(|name| {
        let mut methods = class.methods.iter().filter(|method| method.name == *name);
        methods
            .next()
            .is_some_and(|method| method.self_mutability != SelfMutability::None)
            && methods.next().is_none()
    })
}

/// Infer contracts before CLI filtering discards internal iterator definitions.
pub(super) fn annotate_classes(classes: &mut [PyClassDef]) {
    let inferred: Vec<Vec<_>> = classes
        .iter()
        .map(|class| {
            class
                .methods
                .iter()
                .chain(&class.static_methods)
                .map(|method| {
                    method
                        .return_type
                        .as_deref()
                        .and_then(|source| resolve(source, class, classes))
                })
                .collect()
        })
        .collect();
    for (class, inferred) in classes.iter_mut().zip(inferred) {
        for (method, resolved) in class
            .methods
            .iter_mut()
            .chain(&mut class.static_methods)
            .zip(inferred)
        {
            method.iterator_return_type = resolved;
        }
    }
}

/// Resolve only concrete iteration contracts; dynamic item types remain unknown.
fn resolve(source: &str, owner: &PyClassDef, classes: &[PyClassDef]) -> Option<String> {
    let name = return_name(source)?;
    let mut candidates = classes
        .iter()
        .filter(|class| class.rust_name == name && same_module(class, owner));
    let iterator = candidates.next()?;
    if candidates.next().is_some() || !has_iterator_methods(iterator) {
        return None;
    }
    let iter = iterator
        .methods
        .iter()
        .find(|method| method.name == "__iter__")?;
    let next = iterator
        .methods
        .iter()
        .find(|method| method.name == "__next__")?;
    if !iter.parameters.is_empty() || !next.parameters.is_empty() {
        return None;
    }
    let returned = return_name(iter.return_type.as_deref()?)?;
    if returned != "Self" && returned != iterator.rust_name {
        return None;
    }
    let next_type = syn::parse_str::<Type>(next.return_type.as_deref()?).ok()?;
    let mut item = transparent(&next_type);
    if leaf(item).as_deref() == Some("Option") {
        item = first_type(item)?;
    }
    let item = normalize_rust_type(&quote::quote!(#item).to_string());
    if item
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|token| token == "Any")
    {
        return None;
    }
    Some(format!("Iterator[{item}]"))
}
