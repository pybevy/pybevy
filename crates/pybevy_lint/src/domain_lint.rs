//! E018: a wrapper that declares `domains` must call the generated checks on
//! the write paths it writes by hand. The macro cannot see a separate
//! `#[pymethods]` block, so each `set_<field>` setter and `with_<field>`
//! builder must reach `check_<field>`, `validate_<field>` or
//! `validate_domains`, and `#[new]` must do so for every declared field it
//! takes as a parameter. Writes to either end of an `ordered(lower, upper)`
//! must reach `check_<lower>_order`. A `#[getter]` that lends a declared
//! field for nested writes must use `borrow_validated_field_as` with its
//! validator. Calls resolve through functions in the same file only.

use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use proc_macro2::{Delimiter, LineColumn, TokenStream, TokenTree};
use syn::{Attribute, Expr, FnArg, ImplItem, ImplItemFn, Item, ItemImpl, Pat, Type, visit::Visit};
use walkdir::WalkDir;

use crate::{
    config::ValidationConfig,
    model::SourceLocation,
    output::{Diagnostic, DiagnosticCode},
};

const DOMAIN_MACROS: [&str; 5] = ["pycomponent", "pyresource", "pyasset", "pyvalue", "pyfield"];

/// The rules one wrapper declares.
#[derive(Debug, Default, PartialEq)]
struct Declared {
    fields: Vec<String>,
    orders: Vec<(String, String)>,
    /// `<module>.<Class>` from `#[pyclass(name, module)]`, for exceptions.
    path: String,
}

impl Declared {
    fn order_of(&self, field: &str) -> Option<&(String, String)> {
        self.orders
            .iter()
            .find(|(lower, upper)| lower == field || upper == field)
    }
}

/// Split a token stream at top-level commas.
fn split_commas(tokens: TokenStream) -> Vec<Vec<TokenTree>> {
    let mut items = vec![Vec::new()];
    for token in tokens {
        match &token {
            TokenTree::Punct(punct) if punct.as_char() == ',' => items.push(Vec::new()),
            _ => items.last_mut().expect("never empty").push(token),
        }
    }
    items.retain(|item| !item.is_empty());
    items
}

fn ident_text(token: &TokenTree) -> Option<String> {
    match token {
        TokenTree::Ident(ident) => Some(ident.to_string()),
        _ => None,
    }
}

/// The `domains = [...]` declaration of a wrapper macro attribute, if any.
fn declared_domains(attribute: &Attribute) -> Option<Declared> {
    let name = attribute.path().segments.last()?.ident.to_string();
    if !DOMAIN_MACROS.contains(&name.as_str()) {
        return None;
    }
    let syn::Meta::List(list) = &attribute.meta else {
        return None;
    };
    let options = split_commas(list.tokens.clone());
    let list = options
        .into_iter()
        .find_map(|option| match option.as_slice() {
            [
                TokenTree::Ident(key),
                TokenTree::Punct(eq),
                TokenTree::Group(group),
            ] if key == "domains"
                && eq.as_char() == '='
                && group.delimiter() == Delimiter::Bracket =>
            {
                Some(group.stream())
            }
            _ => None,
        })?;
    let mut declared = Declared::default();
    for entry in split_commas(list) {
        match entry.as_slice() {
            [TokenTree::Ident(kind), TokenTree::Group(args)]
                if (kind == "ordered" || kind == "ordered_strict")
                    && args.delimiter() == Delimiter::Parenthesis =>
            {
                let bounds: Vec<String> = args
                    .stream()
                    .into_iter()
                    .filter_map(|t| ident_text(&t))
                    .collect();
                if let [lower, upper] = bounds.as_slice() {
                    declared.orders.push((lower.clone(), upper.clone()));
                }
            }
            _ => {
                // `name => [...]` or `0 as name => [...]`: the name precedes `=>`.
                let arrow = entry.windows(2).position(|pair| {
                    matches!(
                        pair,
                        [TokenTree::Punct(eq), TokenTree::Punct(gt)]
                            if eq.as_char() == '=' && gt.as_char() == '>'
                    )
                });
                if let Some(name) = arrow
                    .and_then(|index| index.checked_sub(1))
                    .and_then(|index| ident_text(&entry[index]))
                {
                    declared.fields.push(name);
                }
            }
        }
    }
    (!declared.fields.is_empty() || !declared.orders.is_empty()).then_some(declared)
}

/// Every function and method name a body calls.
#[derive(Default)]
struct Calls(HashSet<String>);

impl<'ast> Visit<'ast> for Calls {
    fn visit_expr(&mut self, expr: &'ast Expr) {
        match expr {
            Expr::Path(path) => {
                if let Some(segment) = path.path.segments.last() {
                    self.0.insert(segment.ident.to_string());
                }
            }
            Expr::MethodCall(call) => {
                self.0.insert(call.method.to_string());
            }
            _ => {}
        }
        syn::visit::visit_expr(self, expr);
    }
}

fn calls_of(block: &syn::Block) -> HashSet<String> {
    let mut calls = Calls::default();
    calls.visit_block(block);
    calls.0
}

/// Names reachable from `start` through functions defined in the file.
fn reachable<'a>(
    start: &'a HashSet<String>,
    functions: &'a HashMap<String, HashSet<String>>,
) -> HashSet<&'a str> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut pending: Vec<&str> = start.iter().map(String::as_str).collect();
    while let Some(name) = pending.pop() {
        if !seen.insert(name) {
            continue;
        }
        if let Some(callees) = functions.get(name) {
            pending.extend(callees.iter().map(String::as_str));
        }
    }
    seen
}

fn has_attribute(attributes: &[Attribute], name: &str) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident(name))
}

/// The property a setter writes: `#[setter(name)]`, else `set_<name>`.
fn written_property(method: &ImplItemFn) -> Option<String> {
    let name = method.sig.ident.to_string();
    for attribute in &method.attrs {
        if attribute.path().is_ident("setter")
            && let Ok(ident) = attribute.parse_args::<syn::Ident>()
        {
            return Some(ident.to_string());
        }
    }
    name.strip_prefix("set_").map(String::from)
}

fn parameter_names(method: &ImplItemFn) -> HashSet<String> {
    method
        .sig
        .inputs
        .iter()
        .filter_map(|input| match input {
            FnArg::Typed(typed) => match typed.pat.as_ref() {
                Pat::Ident(ident) => Some(ident.ident.to_string()),
                _ => None,
            },
            FnArg::Receiver(_) => None,
        })
        .collect()
}

fn self_type_name(item: &ItemImpl) -> Option<String> {
    match item.self_ty.as_ref() {
        Type::Path(path) => path.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// `<module>.<Class>` of a wrapper struct, the path exceptions name.
fn pyclass_path(item: &syn::ItemStruct) -> String {
    let mut name = item.ident.to_string().trim_start_matches("Py").to_string();
    let mut module = String::new();
    for attribute in item.attrs.iter().filter(|a| a.path().is_ident("pyclass")) {
        let _ = attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") || meta.path.is_ident("module") {
                let value: syn::LitStr = meta.value()?.parse()?;
                if meta.path.is_ident("name") {
                    name = value.value();
                } else {
                    module = value.value();
                }
            } else if meta.input.peek(syn::Token![=]) {
                meta.value()?.parse::<Expr>()?;
            }
            Ok(())
        });
    }
    let module = module.strip_prefix("pybevy.").unwrap_or(&module);
    format!("{module}.{name}")
}

struct Finding {
    path: String,
    wrapper: String,
    method: String,
    missing: String,
    location: LineColumn,
}

fn missing_checks(
    declared: &Declared,
    method: &ImplItemFn,
    reached: &HashSet<&str>,
) -> Vec<String> {
    let covers = |check: &str| reached.contains(check) || reached.contains("validate_domains");
    let field_check = |field: &str| {
        let check = format!("check_{field}");
        (!covers(&check) && !reached.contains(format!("validate_{field}").as_str()))
            .then_some(check)
    };
    let order_check = |field: &str| {
        declared.order_of(field).and_then(|(lower, _)| {
            let check = format!("check_{lower}_order");
            (!covers(&check)).then_some(check)
        })
    };
    let name = method.sig.ident.to_string();
    let mut missing = Vec::new();
    if has_attribute(&method.attrs, "getter") {
        let lends = ["borrow_field_as", "borrow_resolved_field_as"]
            .iter()
            .any(|borrow| reached.contains(borrow));
        let validated = reached.contains("borrow_validated_field_as");
        if declared.fields.contains(&name) && (lends || validated) {
            let validator = format!("validate_{name}");
            if lends || !(reached.contains(validator.as_str()) || covers(&validator)) {
                missing.push(format!("borrow_validated_field_as with {validator}"));
            }
        }
    } else if has_attribute(&method.attrs, "new") {
        let parameters = parameter_names(method);
        for field in declared.fields.iter().filter(|f| parameters.contains(*f)) {
            missing.extend(field_check(field));
        }
        for (lower, upper) in &declared.orders {
            if parameters.contains(lower) && parameters.contains(upper) {
                missing.extend(order_check(lower));
            }
        }
    } else if let Some(field) = name.strip_prefix("with_") {
        if declared.fields.iter().any(|f| f == field) {
            missing.extend(field_check(field));
        }
        missing.extend(order_check(field));
    } else if let Some(field) = written_property(method) {
        if declared.fields.contains(&field) {
            missing.extend(field_check(&field));
        }
        missing.extend(order_check(&field));
    }
    missing.dedup();
    missing
}

fn lint_source(contents: &str) -> syn::Result<Vec<Finding>> {
    let file = syn::parse_file(contents)?;
    let mut declared: HashMap<String, Declared> = HashMap::new();
    let mut functions: HashMap<String, HashSet<String>> = HashMap::new();
    let mut method_blocks: Vec<(&ItemImpl, &ImplItemFn)> = Vec::new();
    for item in &file.items {
        match item {
            Item::Struct(item) => {
                if let Some(mut domains) = item.attrs.iter().find_map(declared_domains) {
                    domains.path = pyclass_path(item);
                    declared.insert(item.ident.to_string(), domains);
                }
            }
            Item::Fn(item) => {
                functions
                    .entry(item.sig.ident.to_string())
                    .or_default()
                    .extend(calls_of(&item.block));
            }
            Item::Impl(item) => {
                let pymethods = has_attribute(&item.attrs, "pymethods");
                for member in &item.items {
                    if let ImplItem::Fn(method) = member {
                        functions
                            .entry(method.sig.ident.to_string())
                            .or_default()
                            .extend(calls_of(&method.block));
                        if pymethods {
                            method_blocks.push((item, method));
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut findings = Vec::new();
    for (item, method) in method_blocks {
        let Some(wrapper) = self_type_name(item) else {
            continue;
        };
        let Some(domains) = declared.get(&wrapper) else {
            continue;
        };
        let direct = calls_of(&method.block);
        let reached = reachable(&direct, &functions);
        for missing in missing_checks(domains, method, &reached) {
            findings.push(Finding {
                path: domains.path.clone(),
                wrapper: wrapper.clone(),
                method: method.sig.ident.to_string(),
                missing,
                location: method.sig.ident.span().start(),
            });
        }
    }
    Ok(findings)
}

/// Append E018 diagnostics, unless the caller narrowed to named types.
/// Reviewed `[[validation.exceptions]]` with code E018 suppress a finding by
/// `<module>.<Class>` path and optional exact message; an exception that
/// suppresses nothing is reported as stale (E010). Takes and returns the
/// vector so the call site is one rebinding line.
pub fn with_domain_lint(
    mut diagnostics: Vec<Diagnostic>,
    root: &Path,
    validation: &ValidationConfig,
    type_filter: &[String],
) -> Result<Vec<Diagnostic>> {
    if type_filter.is_empty() {
        diagnostics.extend(validate_domain_checks(root, validation)?);
    }
    Ok(diagnostics)
}

pub fn validate_domain_checks(
    root: &Path,
    validation: &ValidationConfig,
) -> Result<Vec<Diagnostic>> {
    let exceptions: Vec<_> = validation
        .exceptions
        .iter()
        .filter(|exception| exception.code == DiagnosticCode::E018.as_str())
        .collect();
    let mut consumed = vec![false; exceptions.len()];
    let mut diagnostics = Vec::new();
    let sources = [root.join("crates"), root.join("src")];
    for entry in sources
        .iter()
        .filter(|source| source.is_dir())
        .flat_map(|source| {
            WalkDir::new(source)
                .into_iter()
                .filter_map(|entry| entry.ok())
        })
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "rs")
                && !entry
                    .path()
                    .components()
                    .any(|part| part.as_os_str() == "target")
        })
    {
        let path = entry.path();
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if !contents.contains("domains") {
            continue;
        }
        let findings = lint_source(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        for finding in findings {
            let message = format!(
                "{}::{} writes a declared domain without calling {}",
                finding.wrapper, finding.method, finding.missing
            );
            let excepted = exceptions.iter().position(|exception| {
                exception.path == finding.path
                    && exception
                        .message
                        .as_ref()
                        .is_none_or(|text| *text == message)
            });
            if let Some(index) = excepted {
                consumed[index] = true;
                continue;
            }
            diagnostics.push(
                Diagnostic::error(DiagnosticCode::E018, message)
                    .with_location(SourceLocation {
                        file: PathBuf::from(path),
                        line: finding.location.line,
                        column: finding.location.column,
                    })
                    .with_note(format!("parser-origin path: {}", finding.path)),
            );
        }
    }
    for (exception, consumed) in exceptions.iter().zip(consumed) {
        if !consumed {
            diagnostics.push(
                Diagnostic::error(
                    DiagnosticCode::E010,
                    format!(
                        "validation exception {} for '{}' is stale or invalid",
                        exception.code, exception.path
                    ),
                )
                .with_note(exception.reason.clone()),
            );
        }
    }
    Ok(diagnostics)
}
