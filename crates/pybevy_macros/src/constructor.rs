use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, ImplItem, ItemImpl, Result, parse_quote};

use super::constructor_spec::ConstructorSpec;

pub fn expand(tokens: TokenStream, mut item: ItemImpl, core: &TokenStream) -> Result<ItemImpl> {
    let spec = ConstructorSpec::parse(tokens, &item)?;
    let ImplItem::Fn(method) = &mut item.items[spec.method_index] else {
        unreachable!()
    };
    let class_name = &spec.name;
    let raw_arg = quote!(#core::constructor::pyo3::RawArg);
    let mut signature = Vec::new();
    let mut inputs: Vec<FnArg> = Vec::new();
    let mut extraction = Vec::new();
    for (i, parameter) in spec.parameters.iter().enumerate() {
        let name = &parameter.name;
        let label = name.to_string();
        let value_type = &parameter.value_type;
        let expected = &parameter.expected;
        if i == spec.keyword_start {
            signature.push(quote!(*));
        }
        let optional = parameter.optional;
        signature.push(if optional || parameter.default.is_some() {
            quote!(#name = #raw_arg::Missing)
        } else {
            quote!(#name)
        });
        inputs.push(parse_quote!(#name: #raw_arg<'_>));
        let value = if let Some(default) = &parameter.default {
            quote!(#name.optional::<#value_type>(#class_name, #label, #expected)?.unwrap_or(#default))
        } else if optional {
            quote!(#name.optional::<#value_type>(#class_name, #label, #expected)?)
        } else {
            quote!(#name.required::<#value_type>(#class_name, #label, #expected)?)
        };
        extraction.push(quote!(let #name = #value;));
    }
    let text_signature = spec.text_signature();
    method.attrs.push(
        parse_quote!(#[pyo3(signature = (#(#signature),*), text_signature = #text_signature)]),
    );
    method.sig.inputs = inputs.into_iter().collect();
    let members = |names: &[syn::Ident]| -> Vec<TokenStream> {
        names
            .iter()
            .map(|name| {
                let label = name.to_string();
                quote!((#label, &#name))
            })
            .collect()
    };
    let conflicts = spec.conflicts.iter().map(|(left, right)| {
        let left = members(left);
        let right = members(right);
        quote!(#core::constructor::pyo3::reject_conflicting_forms(#class_name, &[#(#left),*], &[#(#right),*])?;)
    });
    let complete = spec.complete.iter().map(|group| {
        let group = members(group);
        quote!(#core::constructor::pyo3::require_complete_form(#class_name, &[#(#group),*])?;)
    });
    let body = &method.block;
    method.block = parse_quote!({
        #(#conflicts)*
        #(#complete)*
        #(#extraction)*
        #body
    });
    Ok(item)
}
