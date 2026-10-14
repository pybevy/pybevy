use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use syn::{
    Expr, ExprLit, Ident, Lit, LitStr, Path as SynPath, Token, parse::ParseStream,
    punctuated::Punctuated, visit::Visit,
};
use walkdir::WalkDir;

use crate::{
    bevy_parser::types::BevyCrate,
    config::BevyConfig,
    model::SourceLocation,
    output::{Diagnostic, DiagnosticCode, Suggestion},
};

/// One `register_native_system_set!(intern_fn, bevy::path::Set, module = "m", name = "N")`.
#[derive(Debug, Clone)]
pub struct NativeSystemSetDef {
    pub bevy_path: Vec<String>,
    pub module: String,
    pub name: String,
    pub location: SourceLocation,
}

struct Invocation {
    bevy_path: SynPath,
    module: String,
    name: String,
}

fn parse_invocation(input: ParseStream<'_>) -> syn::Result<Invocation> {
    input.parse::<Ident>()?;
    input.parse::<Token![,]>()?;
    let bevy_path = input.parse::<SynPath>()?;
    input.parse::<Token![,]>()?;
    let options = Punctuated::<syn::MetaNameValue, Token![,]>::parse_terminated(input)?;
    let option = |key: &str| -> syn::Result<String> {
        options
            .iter()
            .find(|option| option.path.is_ident(key))
            .and_then(|option| match &option.value {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(value),
                    ..
                }) => Some(LitStr::value(value)),
                _ => None,
            })
            .ok_or_else(|| input.error(format!("missing `{key} = \"...\"`")))
    };
    Ok(Invocation {
        bevy_path,
        module: option("module")?,
        name: option("name")?,
    })
}

struct RegistrationVisitor<'a> {
    file: &'a Path,
    found: Vec<NativeSystemSetDef>,
}

impl<'ast> Visit<'ast> for RegistrationVisitor<'_> {
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let is_registration = mac
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "register_native_system_set");
        if is_registration && let Ok(invocation) = mac.parse_body_with(parse_invocation) {
            let start = mac.path.segments.last().map_or_else(
                || proc_macro2::Span::call_site().start(),
                |segment| segment.ident.span().start(),
            );
            self.found.push(NativeSystemSetDef {
                bevy_path: invocation
                    .bevy_path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect(),
                module: invocation.module,
                name: invocation.name,
                location: SourceLocation {
                    file: PathBuf::from(self.file),
                    line: start.line,
                    column: start.column,
                },
            });
        }
        syn::visit::visit_macro(self, mac);
    }
}

/// Collect native system-set registrations under `root`'s `src/` and `crates/`.
pub fn parse_native_system_sets(root: &Path) -> Result<Vec<NativeSystemSetDef>> {
    let mut sets = Vec::new();
    for source in [root.join("src"), root.join("crates")] {
        if !source.is_dir() {
            continue;
        }
        for entry in WalkDir::new(&source)
            .into_iter()
            .filter_entry(|entry| entry.file_name() != "target")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
        {
            let path = entry.path();
            let contents = fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            if !contents.contains("register_native_system_set!") {
                continue;
            }
            let syntax = syn::parse_file(&contents)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            let mut visitor = RegistrationVisitor {
                file: path,
                found: Vec::new(),
            };
            visitor.visit_file(&syntax);
            sets.extend(visitor.found);
        }
    }
    sets.sort_by(|a, b| (&a.module, &a.name).cmp(&(&b.module, &b.name)));
    Ok(sets)
}

/// The Bevy crate declaring a set: the `bevy::<crate>::` facade segment when
/// that crate declares the name, otherwise the single parsed crate that does.
fn declaring_crate<'a>(
    set: &NativeSystemSetDef,
    bevy_crates: &'a HashMap<String, BevyCrate>,
) -> Option<&'a str> {
    let declared_name = set.bevy_path.last()?;
    let declares = |krate: &BevyCrate| krate.items.values().any(|item| &item.name == declared_name);
    if let [root, facade, ..] = set.bevy_path.as_slice()
        && root == "bevy"
        && let Some((key, krate)) = bevy_crates.get_key_value(&format!("bevy_{facade}"))
        && declares(krate)
    {
        return Some(key.as_str());
    }
    let mut declaring = bevy_crates
        .iter()
        .filter(|(_, krate)| declares(krate))
        .map(|(key, _)| key.as_str());
    let first = declaring.next()?;
    declaring.next().is_none().then_some(first)
}

/// W029 for native system sets: the Python module named by the registration
/// must match the module mapped from the Bevy crate that declares the set.
pub fn validate_native_system_set_placement(
    sets: &[NativeSystemSetDef],
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for set in sets {
        let Some(bevy_crate) = declaring_crate(set, bevy_crates) else {
            continue;
        };
        let Some(expected) = config
            .crate_mappings
            .iter()
            .filter(|(_, crate_name)| crate_name.as_str() == bevy_crate)
            .map(|(module, _)| module.as_str())
            .min()
        else {
            continue;
        };
        let declared_name = set.bevy_path.last().map_or("", String::as_str);
        if expected == set.module
            || config.is_module_placement_override(declared_name, bevy_crate, &set.module)
        {
            continue;
        }
        diagnostics.push(
            Diagnostic::warning(
                DiagnosticCode::W029,
                format!(
                    "native system set '{}' is registered in Python module 'pybevy.{}', but Bevy set '{}::{}' belongs to 'pybevy.{}'",
                    set.name, set.module, bevy_crate, declared_name, expected
                ),
            )
            .with_note(
                "the canonical Bevy public item path fixes the Python module; Rust ownership may stay in a lower-level crate",
            )
            .with_suggestion(Suggestion::new(
                "change the registration's `module` and move the stub constant to the canonical module, or record [bevy.module_placement_overrides] with the reason when the placement is intentional",
            ))
            .with_location(set.location.clone()),
        );
    }
    diagnostics
}

/// Append native system-set placement diagnostics when Bevy declarations are
/// available and the caller did not narrow to named types.
pub fn with_native_system_set_lint(
    mut diagnostics: Vec<Diagnostic>,
    root: &Path,
    bevy_crates: Option<&HashMap<String, BevyCrate>>,
    config: &BevyConfig,
    type_filter: &[String],
) -> Result<Vec<Diagnostic>> {
    if let Some(bevy_crates) = bevy_crates
        && type_filter.is_empty()
    {
        let sets = parse_native_system_sets(root)?;
        diagnostics.extend(validate_native_system_set_placement(
            &sets,
            bevy_crates,
            config,
        ));
    }
    Ok(diagnostics)
}
