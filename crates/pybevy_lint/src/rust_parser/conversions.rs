use std::path::Path;

use syn::{Expr, ItemImpl, Pat, PathArguments, Type, visit::Visit};

use crate::model::{PyClassDef, SilentFallbackInfo, SourceLocation};

/// Record wildcard match arms that yield a concrete value inside a
/// `From`/`TryFrom` impl targeting a `Py*` wrapper.
pub fn process_conversion_impl(item_impl: &ItemImpl, classes: &mut [PyClassDef], file_path: &Path) {
    let Some(source_type) = conversion_source(item_impl) else {
        return;
    };
    let Some(target) = self_type_name(&item_impl.self_ty) else {
        return;
    };
    if !target.starts_with("Py") {
        return;
    }
    let Some(class) = classes.iter_mut().find(|c| c.rust_name == target) else {
        return;
    };

    let mut finder = FallbackFinder {
        file_path,
        found: Vec::new(),
    };
    finder.visit_item_impl(item_impl);

    for location in finder.found {
        class.silent_fallbacks.push(SilentFallbackInfo {
            source_type: source_type.clone(),
            location: Some(location),
        });
    }
}

/// The `T` of `impl From<T> for Py...`, when the trait is a conversion.
fn conversion_source(item_impl: &ItemImpl) -> Option<String> {
    let (_, path, _) = item_impl.trait_.as_ref()?;
    let segment = path.segments.last()?;
    let name = segment.ident.to_string();
    if name != "From" && name != "TryFrom" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let arg = args.args.first()?;
    let source = quote::quote!(#arg).to_string().replace(' ', "");
    // A Py -> Py or Py -> bevy conversion is not the direction that can hide a
    // native variant behind a fallback.
    (!source.trim_start_matches('&').starts_with("Py")).then_some(source)
}

fn self_type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(type_path) => Some(type_path.path.segments.last()?.ident.to_string()),
        _ => None,
    }
}

struct FallbackFinder<'a> {
    file_path: &'a Path,
    found: Vec<SourceLocation>,
}

impl<'ast> Visit<'ast> for FallbackFinder<'_> {
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        for arm in &node.arms {
            if is_catch_all(&arm.pat) && !yields_explicit_failure(&arm.body) {
                let span = arm.pat.span_start();
                self.found.push(SourceLocation {
                    file: self.file_path.to_path_buf(),
                    line: span.0,
                    column: span.1,
                });
            }
        }
        syn::visit::visit_expr_match(self, node);
    }
}

trait SpanStart {
    fn span_start(&self) -> (usize, usize);
}

impl SpanStart for Pat {
    fn span_start(&self) -> (usize, usize) {
        let span = syn::spanned::Spanned::span(self).start();
        (span.line, span.column)
    }
}

/// `_` or a bare binding, both of which absorb every remaining variant.
fn is_catch_all(pat: &Pat) -> bool {
    match pat {
        Pat::Wild(_) => true,
        Pat::Ident(ident) => ident.subpat.is_none() && ident.by_ref.is_none(),
        _ => false,
    }
}

/// Whether the arm reports the unmapped input rather than substituting a value.
fn yields_explicit_failure(expr: &Expr) -> bool {
    match expr {
        Expr::Return(_) | Expr::Macro(_) => true,
        Expr::Try(inner) => yields_explicit_failure(&inner.expr),
        Expr::Call(call) => {
            let func = &call.func;
            let name = quote::quote!(#func).to_string().replace(' ', "");
            name.ends_with("Err") || name.ends_with("new_err")
        }
        Expr::Path(path) => {
            let name = quote::quote!(#path).to_string().replace(' ', "");
            name.ends_with("None")
        }
        Expr::Block(block) => block.block.stmts.iter().any(
            |stmt| matches!(stmt, syn::Stmt::Expr(inner, _) if yields_explicit_failure(inner)),
        ),
        _ => false,
    }
}
