use std::collections::{HashMap, HashSet};

use proc_macro2::{Group, TokenStream, TokenTree};
use syn::{Expr, ImplItem, Item, Type, parse::Parser, visit::Visit};

use super::types::type_to_string;
use crate::model::{EnumVariantDef, EnumVariantKind, MacroInfo, PyClassDef};

/// Recover only topology witnessed in Python class attributes or registrations.
pub(super) fn extract(file: &syn::File, classes: &mut [PyClassDef]) {
    if !classes.iter().any(|class| {
        !class.is_enum
            && matches!(
                class.macro_info,
                Some(MacroInfo::BevyEnum { manual: true, .. })
            )
    }) {
        return;
    }
    let (expanded, expanded_macros) = expand_ident_lists(file);
    let items: Vec<_> = file.items.iter().chain(&expanded).collect();
    for index in 0..classes.len() {
        let base = &classes[index];
        let Some(MacroInfo::BevyEnum {
            bevy_type,
            manual: true,
            ..
        }) = &base.macro_info
        else {
            continue;
        };
        if base.is_enum {
            continue;
        }
        let native = bevy_type.split('<').next().unwrap_or(bevy_type);
        let mut variants = Vec::new();
        let mut supported = true;
        let mut witnessed = false;
        for item in &items {
            if let Item::Impl(block) = item
                && is_impl(block, &base.rust_name)
            {
                for member in &block.items {
                    if matches!(member, ImplItem::Macro(_))
                        || matches!(member, ImplItem::Const(value) if has_attr(&value.attrs, "classattr") && !value.ident.to_string().starts_with("__"))
                    {
                        supported = false;
                    }
                    if let ImplItem::Fn(method) = member
                        && has_attr(&method.attrs, "classattr")
                        && !method.sig.ident.to_string().starts_with("__")
                    {
                        witnessed = true;
                        let name = method.sig.ident.to_string();
                        if let Some(kind) = construction(method, native, &name) {
                            variants.push(EnumVariantDef {
                                name,
                                kind,
                                constructor: None,
                                writable_fields: Vec::new(),
                            });
                        } else {
                            supported = false;
                        }
                    }
                }
            }
            if let Item::Fn(function) = item {
                let mut registrations = Registrations {
                    base: &base.python_name,
                    aliases: HashMap::new(),
                    entries: Vec::new(),
                    witnessed: false,
                    supported: true,
                };
                registrations.visit_block(&function.block);
                witnessed |= registrations.witnessed;
                supported &= registrations.supported;
                for (name, rust_name) in registrations.entries {
                    let variant = classes.iter().find(|class| {
                        class.rust_name == rust_name
                            && class.extends.as_deref() == Some(&base.rust_name)
                    });
                    let method = items.iter().find_map(|item| {
                        if let Item::Impl(block) = item
                            && is_impl(block, &rust_name)
                        {
                            return block.items.iter().find_map(|member| match member {
                                ImplItem::Fn(method) if has_attr(&method.attrs, "new") => {
                                    Some(method)
                                }
                                _ => None,
                            });
                        }
                        None
                    });
                    if let Some((variant, kind)) =
                        variant.zip(method.and_then(|method| construction(method, native, &name)))
                    {
                        variants.push(EnumVariantDef {
                            name,
                            kind,
                            constructor: variant.constructor.clone(),
                            writable_fields: Vec::new(),
                        });
                    } else {
                        supported = false;
                    }
                }
            }
        }
        // An unexpanded local macro may add methods to this base.
        for item in &file.items {
            if let Item::Macro(definition) = item
                && definition.mac.path.is_ident("macro_rules")
                && definition
                    .mac
                    .tokens
                    .to_string()
                    .split_whitespace()
                    .any(|word| word == base.rust_name)
                && !definition
                    .ident
                    .as_ref()
                    .is_some_and(|name| expanded_macros.contains(&name.to_string()))
            {
                supported = false;
            }
        }
        let mut names = HashSet::new();
        supported &= variants
            .iter()
            .all(|variant| names.insert(variant.name.clone()));
        classes[index].manual_enum_verified = witnessed && supported;
        if witnessed && supported {
            classes[index].enum_variants = variants;
        }
    }
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn is_impl(block: &syn::ItemImpl, name: &str) -> bool {
    has_attr(&block.attrs, "pymethods")
        && matches!(&*block.self_ty, Type::Path(path) if path.path.is_ident(name))
}

fn strip(expr: &Expr) -> &Expr {
    match expr {
        Expr::Try(expr) => strip(&expr.expr),
        Expr::Paren(expr) => strip(&expr.expr),
        _ => expr,
    }
}

fn string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(value),
            ..
        }) => Some(value.value()),
        _ => None,
    }
}

struct Registrations<'a> {
    base: &'a str,
    aliases: HashMap<String, bool>,
    entries: Vec<(String, String)>,
    witnessed: bool,
    supported: bool,
}

impl Registrations<'_> {
    fn is_base(&self, expr: &Expr) -> bool {
        match strip(expr) {
            Expr::Path(path) => self
                .aliases
                .get(
                    &path
                        .path
                        .get_ident()
                        .map(ToString::to_string)
                        .unwrap_or_default(),
                )
                .copied()
                .unwrap_or(false),
            Expr::MethodCall(call) => {
                call.method == "getattr"
                    && call.args.first().and_then(string).as_deref() == Some(self.base)
            }
            _ => false,
        }
    }
}

impl<'ast> Visit<'ast> for Registrations<'_> {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let syn::Pat::Ident(name) = &local.pat
            && let Some(init) = &local.init
        {
            let is_base = self.is_base(&init.expr);
            self.witnessed |= is_base;
            self.aliases.insert(name.ident.to_string(), is_base);
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "setattr" && self.is_base(&call.receiver) {
            self.witnessed = true;
            let entry = call
                .args
                .first()
                .and_then(string)
                .zip(call.args.iter().nth(1).and_then(|expr| {
                    let Expr::MethodCall(value) = expr else {
                        return None;
                    };
                    if value.method != "get_type" {
                        return None;
                    }
                    let syn::GenericArgument::Type(Type::Path(path)) =
                        value.turbofish.as_ref()?.args.first()?
                    else {
                        return None;
                    };
                    Some(path.path.get_ident()?.to_string())
                }));
            if let Some(entry) = entry {
                self.entries.push(entry);
            } else {
                self.supported = false;
            }
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}

fn construction(method: &syn::ImplItemFn, native: &str, name: &str) -> Option<EnumVariantKind> {
    let parameters: HashMap<_, _> = method
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            let syn::FnArg::Typed(arg) = arg else {
                return None;
            };
            let syn::Pat::Ident(name) = &*arg.pat else {
                return None;
            };
            let ty = match &*arg.ty {
                Type::Reference(reference) => &*reference.elem,
                ty => ty,
            };
            Some((name.ident.to_string(), type_to_string(ty)))
        })
        .collect();
    let mut visitor = Construction {
        native,
        name,
        parameters,
        kinds: Vec::new(),
    };
    visitor.visit_block(&method.block);
    if visitor.kinds.len() == 1 {
        visitor.kinds.pop().flatten()
    } else {
        None
    }
}

struct Construction<'a> {
    native: &'a str,
    name: &'a str,
    parameters: HashMap<String, String>,
    kinds: Vec<Option<EnumVariantKind>>,
}

impl Construction<'_> {
    fn matches(&self, path: &syn::Path) -> bool {
        path.segments.len() == 2
            && path.segments[0].ident == self.native
            && path.segments[1].ident == self.name
    }

    fn payload_type(&self, expr: &Expr) -> Option<String> {
        match strip(expr) {
            Expr::Path(path) => self
                .parameters
                .get(&path.path.get_ident()?.to_string())
                .cloned(),
            Expr::MethodCall(call) if call.method == "into" || call.method == "to_bevy" => {
                self.payload_type(&call.receiver)
            }
            _ => None,
        }
    }
}

impl<'ast> Visit<'ast> for Construction<'_> {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            Expr::Call(call) if matches!(&*call.func, Expr::Path(path) if self.matches(&path.path)) =>
            {
                let fields: Option<Vec<_>> =
                    call.args.iter().map(|arg| self.payload_type(arg)).collect();
                self.kinds.push(fields.map(EnumVariantKind::Tuple));
            }
            Expr::Struct(value) if self.matches(&value.path) => {
                let fields: Option<Vec<_>> = value
                    .fields
                    .iter()
                    .map(|field| {
                        let syn::Member::Named(name) = &field.member else {
                            return None;
                        };
                        Some((name.to_string(), self.payload_type(&field.expr)?))
                    })
                    .collect();
                self.kinds.push(
                    fields
                        .filter(|_| value.rest.is_none())
                        .map(EnumVariantKind::Struct),
                );
            }
            Expr::Path(path) if self.matches(&path.path) => {
                self.kinds.push(Some(EnumVariantKind::Unit))
            }
            _ => syn::visit::visit_expr(self, expr),
        }
    }
}

/// Support the single ident-list repetition used by handwritten classattrs.
fn ident_list_template(item: &syn::ItemMacro) -> Option<(String, TokenStream)> {
    let tokens: Vec<_> = item.mac.tokens.clone().into_iter().collect();
    let [
        TokenTree::Group(matcher),
        TokenTree::Punct(eq),
        TokenTree::Punct(gt),
        TokenTree::Group(body),
        ..,
    ] = tokens.as_slice()
    else {
        return None;
    };
    if eq.as_char() != '=' || gt.as_char() != '>' || tokens.len() > 5 {
        return None;
    }
    let compact = matcher.stream().to_string().replace(' ', "");
    let variable = compact
        .strip_prefix("$($")?
        .strip_suffix(":ident),*$(,)?")?;
    syn::parse_str::<syn::Ident>(variable).ok()?;
    Some((variable.to_string(), body.stream()))
}

fn expand_ident_lists(file: &syn::File) -> (Vec<Item>, HashSet<String>) {
    let mut items = Vec::new();
    let mut expanded_macros = HashSet::new();
    for item in &file.items {
        let Item::Macro(definition) = item else {
            continue;
        };
        if !definition.mac.path.is_ident("macro_rules") {
            continue;
        }
        let Some((variable, template)) = ident_list_template(definition) else {
            continue;
        };
        let Some(name) = &definition.ident else {
            continue;
        };
        let mut seen = false;
        let mut supported = true;
        for item in &file.items {
            let Item::Macro(invocation) = item else {
                continue;
            };
            if !invocation.mac.path.is_ident(name) {
                continue;
            }
            seen = true;
            let parser =
                syn::punctuated::Punctuated::<syn::Ident, syn::Token![,]>::parse_terminated;
            let Ok(values) = parser.parse2(invocation.mac.tokens.clone()) else {
                supported = false;
                continue;
            };
            if let Some(tokens) = substitute(
                template.clone(),
                &variable,
                &values.into_iter().collect::<Vec<_>>(),
                None,
            ) && let Ok(expanded) = syn::parse2::<syn::File>(tokens)
            {
                items.extend(expanded.items);
            } else {
                supported = false;
            }
        }
        if seen && supported {
            expanded_macros.insert(name.to_string());
        }
    }
    (items, expanded_macros)
}

fn substitute(
    stream: TokenStream,
    variable: &str,
    values: &[syn::Ident],
    value: Option<&syn::Ident>,
) -> Option<TokenStream> {
    let mut tokens = stream.into_iter().peekable();
    let mut output = TokenStream::new();
    while let Some(token) = tokens.next() {
        match token {
            TokenTree::Punct(ref punct) if punct.as_char() == '$' => match tokens.next()? {
                TokenTree::Ident(name) if name == variable => {
                    output.extend([TokenTree::Ident(value?.clone())])
                }
                TokenTree::Group(group) if value.is_none() => {
                    let TokenTree::Punct(op) = tokens.next()? else {
                        return None;
                    };
                    if op.as_char() != '*' {
                        return None;
                    }
                    for value in values {
                        output.extend(substitute(group.stream(), variable, values, Some(value))?);
                    }
                }
                _ => return None,
            },
            TokenTree::Group(group) => output.extend([TokenTree::Group(Group::new(
                group.delimiter(),
                substitute(group.stream(), variable, values, value)?,
            ))]),
            token => output.extend([token]),
        }
    }
    Some(output)
}
