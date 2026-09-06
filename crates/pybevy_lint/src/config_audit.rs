//! Report configuration entries that no longer match anything upstream.
//!
//! Exclusions and per-type ignores are written against one Bevy release. When
//! Bevy renames or deletes a type the entry stops matching in silence, so the
//! config keeps growing while saying less.
//!
//! The oracle is Bevy's source tree, not the parsed public API. `cargo
//! public-api` reports only what a crate exports under its default features,
//! and for some crates that is a small fraction of the types the config names,
//! so judging staleness from it would condemn entries that are perfectly live.
//!
//! Within the source, the test is whether the identifier occurs in the crate at
//! all rather than whether that crate declares it. Bevy re-exports heavily and
//! generates types from macros such as `define_atomic_id!`, so a declaration
//! scan misses `BufferId`, `ErasedMaterialKey` and their kin. Occurrence errs
//! toward keeping a live entry, which is the safe direction for an error.

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use regex::Regex;
use walkdir::WalkDir;

use crate::{
    config::Config,
    model::PyClassDef,
    output::{Diagnostic, DiagnosticCode},
};

/// The name a config entry refers to, without generic arguments.
fn base_of(name: &str) -> &str {
    name.split('<').next().unwrap_or(name)
}

/// Every identifier that occurs in each Bevy crate's source.
pub struct BevySource {
    /// Short crate name (`bevy_render` -> `render`) to identifiers occurring in it.
    by_module: HashMap<String, HashSet<String>>,
}

impl BevySource {
    /// Index `<bevy_path>/crates` by the identifiers each crate mentions.
    pub fn scan(bevy_path: &Path) -> Self {
        let ident = Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("identifier pattern is valid");

        let mut by_module: HashMap<String, HashSet<String>> = HashMap::new();
        let crates_dir = bevy_path.join("crates");
        for entry in WalkDir::new(&crates_dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_dir())
        {
            let crate_name = entry.file_name().to_string_lossy().into_owned();
            let short = crate_name
                .strip_prefix("bevy_")
                .unwrap_or(&crate_name)
                .to_owned();
            let names = by_module.entry(short).or_default();
            for file in WalkDir::new(entry.path())
                .into_iter()
                .filter_map(Result::ok)
                .filter(|f| f.path().extension().is_some_and(|e| e == "rs"))
            {
                let Ok(source) = std::fs::read_to_string(file.path()) else {
                    continue;
                };
                names.extend(ident.find_iter(&source).map(|m| m.as_str().to_owned()));
            }
        }
        Self { by_module }
    }

    /// Did the scan find any crate at all? A missing or moved checkout yields
    /// nothing, and auditing against nothing would condemn every entry.
    pub fn is_empty(&self) -> bool {
        self.by_module.values().all(HashSet::is_empty)
    }

    fn mentions(&self, module: &str, name: &str) -> bool {
        self.by_module
            .get(module)
            .is_some_and(|names| names.contains(base_of(name)))
    }

    fn mentioned_in(&self, name: &str) -> Vec<&str> {
        let mut found: Vec<&str> = self
            .by_module
            .iter()
            .filter(|(_, names)| names.contains(base_of(name)))
            .map(|(module, _)| module.as_str())
            .collect();
        found.sort_unstable();
        found
    }
}

fn stale(entry: &str, section: &str, note: String) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::E012,
        format!("{section} entry '{entry}' no longer matches a Bevy item"),
    )
    .with_note(note)
}

/// Judge one `module::Type` entry, if it names a type Bevy no longer declares.
fn audit_qualified(source: &BevySource, section: &str, entry: &str) -> Option<Diagnostic> {
    if entry.contains('*') {
        return None;
    }
    let (module, name) = entry.split_once("::")?;
    if source.mentions(module, name) {
        return None;
    }
    let elsewhere = source.mentioned_in(name);
    let note = if elsewhere.is_empty() {
        "no Bevy crate mentions this type; drop the entry".to_owned()
    } else {
        format!(
            "the type moved to {}; re-point the entry or drop it",
            elsewhere.join(", ")
        )
    };
    Some(stale(entry, section, note))
}

/// Audit every config list that names an upstream Bevy or PyBevy item.
pub fn audit(
    config: &Config,
    source: &BevySource,
    pybevy_classes: &[PyClassDef],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let py_any: HashSet<&str> = pybevy_classes
        .iter()
        .map(|class| class.python_name.as_str())
        .collect();
    let mut py_by_module: HashMap<&str, HashSet<&str>> = HashMap::new();
    for class in pybevy_classes {
        if let Some(module) = class.module_path.as_deref() {
            py_by_module
                .entry(module)
                .or_default()
                .insert(class.python_name.as_str());
        }
    }

    let qualified: [(&str, &[String]); 3] = [
        (
            "bevy.excluded_types.types",
            &config.bevy.excluded_types.types,
        ),
        (
            "bevy.excluded_types.audit_ignore",
            &config.bevy.excluded_types.audit_ignore,
        ),
        ("bevy.false_match_types", &config.bevy.false_match_types),
    ];
    for (section, entries) in qualified {
        diagnostics.extend(
            entries
                .iter()
                .filter_map(|entry| audit_qualified(source, section, entry)),
        );
    }
    diagnostics.extend(
        config
            .bevy
            .module_placement_overrides
            .keys()
            .filter_map(|entry| audit_qualified(source, "bevy.module_placement_overrides", entry)),
    );

    for entry in &config.bevy.excluded_types.engine_managed_types {
        if source.mentioned_in(entry).is_empty() {
            diagnostics.push(stale(
                entry,
                "bevy.excluded_types.engine_managed_types",
                "no Bevy crate mentions this type".to_owned(),
            ));
        }
    }

    for entry in &config.bevy.excluded_traits.traits {
        // Full paths name std or third-party traits outside Bevy's tree.
        if entry.contains("::") {
            continue;
        }
        if source.mentioned_in(entry).is_empty() {
            diagnostics.push(stale(
                entry,
                "bevy.excluded_traits.traits",
                "no Bevy crate mentions this trait".to_owned(),
            ));
        }
    }

    for entry in &config.bevy.reflection_unavailable_types {
        if !py_any.contains(entry.as_str()) {
            diagnostics.push(stale(
                entry,
                "bevy.reflection_unavailable_types",
                "no PyBevy wrapper declares this class".to_owned(),
            ));
        }
    }

    // type_ignores is keyed by the Bevy name at one call site and the PyBevy
    // class name at another, so an entry is stale only when neither side has it.
    for entry in config.bevy.type_ignores.keys() {
        let name = entry.rsplit("::").next().unwrap_or(entry);
        if source.mentioned_in(name).is_empty() && !py_any.contains(name) {
            diagnostics.push(stale(
                entry,
                "bevy.type_ignores",
                "neither Bevy nor PyBevy mentions this type".to_owned(),
            ));
        }
    }

    // An exclusion covering a type PyBevy really wraps hides it from coverage.
    let mut mapped: HashMap<&str, Vec<&str>> = HashMap::new();
    for (module, crate_name) in &config.bevy.crate_mappings {
        let short = crate_name.strip_prefix("bevy_").unwrap_or(crate_name);
        mapped.entry(short).or_default().push(module);
    }
    for entry in &config.bevy.excluded_types.types {
        if entry.contains('*') {
            continue;
        }
        let Some((module, name)) = entry.split_once("::") else {
            continue;
        };
        if !source.mentions(module, name) {
            continue;
        }
        let implemented = mapped.get(module).is_some_and(|modules| {
            modules
                .iter()
                .any(|module| py_by_module.get(module).is_some_and(|c| c.contains(name)))
        });
        if implemented {
            diagnostics.push(
                Diagnostic::warning(
                    DiagnosticCode::W012,
                    format!("excluded type '{entry}' is implemented in PyBevy"),
                )
                .with_note(
                    "the exclusion keeps a wrapped type out of coverage; drop it, or record why the wrapper is not comparable".to_owned(),
                ),
            );
        }
    }

    diagnostics
}
