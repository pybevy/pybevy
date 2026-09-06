//! Reproducible inventory of the pinned upstream Bevy public API.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail};

use crate::{
    CoverageReport,
    bevy_parser::{BevyCrate, BevyItemKind},
    config::{BevyConfig, SignatureDiffSpec},
};

pub const BEVY_REPOSITORY: &str = "https://github.com/bevyengine/bevy";
pub const BEVY_TAG: &str = "v0.19.0";
pub const BEVY_REVISION: &str = "c6f634ca9f406d68ba5109d921247b654cb42c10";
pub const CARGO_PUBLIC_API_VERSION: &str = "0.50.1";
pub const PINNED_LOCKFILE_PATH: &str = "tests/parity/bevy_019_Cargo.lock";

pub const DEFAULT_RAW_PATH: &str = "tests/parity/bevy_019_public_api.txt";
pub const DEFAULT_CLASSIFIED_PATH: &str = "tests/parity/bevy_019_binding_inventory.txt";
pub const DEFAULT_COVERAGE_PATH: &str = "docs/coverage.md";
pub const COVERAGE_BEGIN: &str = "<!-- BEGIN GENERATED BEVY 0.19 COVERAGE -->";
pub const COVERAGE_END: &str = "<!-- END GENERATED BEVY 0.19 COVERAGE -->";

#[derive(Debug, Clone)]
pub struct Provenance {
    repository: &'static str,
    tag: &'static str,
    revision: &'static str,
    cargo_public_api: &'static str,
    cargo_public_api_flags: String,
}

impl Default for Provenance {
    fn default() -> Self {
        Self {
            repository: BEVY_REPOSITORY,
            tag: BEVY_TAG,
            revision: BEVY_REVISION,
            cargo_public_api: CARGO_PUBLIC_API_VERSION,
            cargo_public_api_flags: crate::bevy_parser::public_api_flags(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawEntry {
    pub path: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClassifiedEntry {
    pub path: String,
    pub kind: String,
    pub disposition: String,
    pub category: String,
    pub reason: String,
}

pub struct GeneratedAudit {
    pub raw_json: String,
    pub classified_json: String,
    pub coverage_markdown: String,
}

/// Clone the immutable pinned Bevy tag into a repository-owned build cache.
pub fn prepare_pinned_bevy(cache_root: &Path) -> Result<PathBuf> {
    let checkout = cache_root.join(format!("bevy-{}", &BEVY_REVISION[..12]));
    if !checkout.exists() {
        fs::create_dir_all(cache_root).with_context(|| {
            format!("failed to create Bevy audit cache {}", cache_root.display())
        })?;
        let status = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "--branch",
                BEVY_TAG,
                BEVY_REPOSITORY,
            ])
            .arg(&checkout)
            .status()
            .context("failed to execute git clone for the pinned Bevy source")?;
        if !status.success() {
            bail!("failed to clone {BEVY_REPOSITORY} at {BEVY_TAG}");
        }
    }

    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&checkout)
        .output()
        .context("failed to inspect the pinned Bevy checkout")?;
    let revision = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || revision != BEVY_REVISION {
        bail!(
            "Bevy audit checkout is not the pinned revision: expected {BEVY_REVISION}, found {revision}"
        );
    }
    install_pinned_lockfile(&checkout)?;
    Ok(checkout)
}

/// Read the reviewed dependency graph.
///
/// Read rather than `include_str!`: the file belongs to the unpublished test
/// tree, and embedding it would make the whole crate unbuildable wherever that
/// tree is absent, for the sake of one private-only subcommand.
fn pinned_lockfile() -> Result<String> {
    fs::read_to_string(PINNED_LOCKFILE_PATH)
        .with_context(|| format!("failed to read {PINNED_LOCKFILE_PATH}"))
}

/// Install the reviewed dependency graph into Bevy's lockfile-free release tag.
pub fn install_pinned_lockfile(checkout: &Path) -> Result<()> {
    fs::write(checkout.join("Cargo.lock"), pinned_lockfile()?)
        .context("failed to install the pinned Bevy audit Cargo.lock")
}

/// Fail if Cargo changed the reviewed dependency graph during extraction.
pub fn check_pinned_lockfile(checkout: &Path) -> Result<()> {
    let actual = fs::read_to_string(checkout.join("Cargo.lock"))
        .context("the pinned Bevy audit Cargo.lock is missing")?;
    if actual != pinned_lockfile()? {
        bail!(
            "the Bevy audit dependency graph changed while extracting the API; refresh tests/parity/bevy_019_Cargo.lock explicitly"
        );
    }
    Ok(())
}

pub fn check_cargo_public_api_version() -> Result<()> {
    let output = Command::new("cargo")
        .args(["public-api", "--version"])
        .output()
        .context("cargo-public-api is required for the Bevy audit")?;
    let actual = String::from_utf8_lossy(&output.stdout);
    let expected = format!("cargo-public-api {CARGO_PUBLIC_API_VERSION}");
    if !output.status.success() || actual.trim() != expected {
        bail!(
            "the Bevy audit requires {expected}; install it with `cargo install cargo-public-api --version {CARGO_PUBLIC_API_VERSION} --locked` (found `{}`)",
            actual.trim()
        );
    }
    Ok(())
}

pub fn generate(
    crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
    report: &CoverageReport,
    existing_coverage_doc: &str,
) -> Result<GeneratedAudit> {
    let mut crate_names: Vec<_> = crates.keys().cloned().collect();
    crate_names.sort();
    let raw_entries = raw_entries(crates);
    let (classified_entries, adaptations) = classified_entries(crates, config);

    let raw_json = render_raw_inventory(&crate_names, &raw_entries);
    let classified_json = render_classified_inventory(&classified_entries, &adaptations);
    let coverage_markdown = replace_coverage_block(existing_coverage_doc, report)?;
    Ok(GeneratedAudit {
        raw_json,
        classified_json,
        coverage_markdown,
    })
}

fn raw_entries(crates: &HashMap<String, BevyCrate>) -> Vec<RawEntry> {
    let mut entries = BTreeSet::new();
    for bevy_crate in crates.values() {
        for item in bevy_crate.items.values() {
            let item_path = item.full_path.clone();
            entries.insert(RawEntry {
                path: item_path.clone(),
                kind: item.kind.to_string(),
            });
            for method in &item.methods {
                entries.insert(RawEntry {
                    path: format!("{item_path}::{}", method.name),
                    kind: "method".to_owned(),
                });
            }
            for field in &item.fields {
                entries.insert(RawEntry {
                    path: format!("{item_path}::{}", field.name),
                    kind: "field".to_owned(),
                });
            }
            for variant in &item.variants {
                entries.insert(RawEntry {
                    path: format!("{item_path}::{}", variant.name),
                    kind: "variant".to_owned(),
                });
            }
            for (name, _) in &item.constants {
                entries.insert(RawEntry {
                    path: format!("{item_path}::{name}"),
                    kind: "constant".to_owned(),
                });
            }
        }
    }
    entries.into_iter().collect()
}

fn classified_entries(
    crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
) -> (Vec<ClassifiedEntry>, Vec<ClassifiedEntry>) {
    let mut entries = BTreeSet::new();
    let mut adaptations = BTreeSet::new();

    for (crate_name, bevy_crate) in crates {
        if config.is_crate_excluded(crate_name) {
            entries.insert(decision(
                crate_name,
                "crate",
                "excluded",
                "configured_crate_exclusion",
                "the complete upstream crate is outside the supported Python binding boundary",
            ));
            continue;
        }
        for item in bevy_crate.items.values() {
            if !matches!(item.kind, BevyItemKind::Struct | BevyItemKind::Enum) {
                continue;
            }
            let type_rule = config.type_exclusion_rule(&item.name, Some(crate_name));
            let type_decision = if item.is_generic_base {
                decision(
                    &item.full_path,
                    &item.kind.to_string(),
                    "excluded",
                    "generic_template",
                    "generic implementation template; concrete exported specializations are audited separately",
                )
            } else if let Some(rule) = &type_rule {
                decision(
                    &item.full_path,
                    &item.kind.to_string(),
                    "excluded",
                    "configured_type_exclusion",
                    &format!("matched reviewed exclusion rule `{rule}`"),
                )
            } else {
                decision(
                    &item.full_path,
                    &item.kind.to_string(),
                    "binding_worthy",
                    "public_type",
                    "exported public Bevy struct or enum in a mapped crate",
                )
            };
            let parent_excluded = type_decision.disposition == "excluded";
            entries.insert(type_decision);

            // The exact parent decision covers its entire member namespace.
            // Raw inventory still records every child, so additions beneath an
            // excluded type remain visible without duplicating thousands of
            // identical `excluded_parent` rows here.
            if parent_excluded {
                continue;
            }

            for method in &item.methods {
                let path = format!("{}::{}", item.full_path, method.name);
                let method_decision = if let Some(rule) = config.method_exclusion_rule(&method.name)
                {
                    decision(
                        &path,
                        "method",
                        "excluded",
                        "configured_method_exclusion",
                        &format!("matched reviewed exclusion rule `{rule}`"),
                    )
                } else if let Some(trait_name) = &method.from_trait {
                    if let Some(rule) = config.trait_exclusion_rule(trait_name) {
                        decision(
                            &path,
                            "method",
                            "excluded",
                            "configured_trait_exclusion",
                            &format!("provided by reviewed excluded `{rule}`"),
                        )
                    } else {
                        binding_child(&path, "method")
                    }
                } else {
                    binding_child(&path, "method")
                };
                entries.insert(method_decision);
            }
            for field in &item.fields {
                entries.insert(child_decision(
                    &format!("{}::{}", item.full_path, field.name),
                    "field",
                ));
            }
            for variant in &item.variants {
                entries.insert(child_decision(
                    &format!("{}::{}", item.full_path, variant.name),
                    "variant",
                ));
            }
            for (name, _) in &item.constants {
                entries.insert(child_decision(
                    &format!("{}::{name}", item.full_path),
                    "constant",
                ));
            }
        }
    }

    for (python, bevy) in &config.type_mappings {
        adaptations.insert(decision(
            &format!("pybevy::{python}"),
            "adaptation",
            "adapted",
            "type_mapping",
            &format!("maps to upstream Bevy type `{bevy}`"),
        ));
    }
    for path in &config.false_match_types {
        adaptations.insert(decision(
            path,
            "adaptation",
            "adapted",
            "false_name_match",
            "same short name denotes a different Python and Bevy concept",
        ));
    }
    for (path, module) in &config.module_placement_overrides {
        adaptations.insert(decision(
            path,
            "adaptation",
            "adapted",
            "module_placement",
            &format!("intentionally exported from Python module `{module}`"),
        ));
    }
    append_type_ignore_adaptations(config, &mut adaptations);

    (
        entries.into_iter().collect(),
        adaptations.into_iter().collect(),
    )
}

fn render_raw_inventory(crate_names: &[String], entries: &[RawEntry]) -> String {
    let provenance = Provenance::default();
    let mut output = format!(
        "# pybevy-bevy-public-api v1\n# repository: {}\n# tag: {}\n# revision: {}\n# cargo-public-api: {} {}\n# crates: {}\n# columns: kind\\tpath\n",
        provenance.repository,
        provenance.tag,
        provenance.revision,
        provenance.cargo_public_api,
        provenance.cargo_public_api_flags,
        crate_names.join(","),
    );
    for entry in entries {
        output.push_str(&format!("{}\t{}\n", entry.kind, entry.path));
    }
    output
}

fn render_classified_inventory(
    entries: &[ClassifiedEntry],
    adaptations: &[ClassifiedEntry],
) -> String {
    let provenance = Provenance::default();
    let mut output = format!(
        "# pybevy-bevy-binding-inventory v1\n# repository: {}\n# tag: {}\n# revision: {}\n# cargo-public-api: {} {}\n# dispositions: B=binding_worthy, X=excluded, A=adapted\n# columns: disposition\\tkind\\tpath\\tcategory\\treason\n",
        provenance.repository,
        provenance.tag,
        provenance.revision,
        provenance.cargo_public_api,
        provenance.cargo_public_api_flags,
    );
    for entry in entries.iter().chain(adaptations) {
        let disposition = match entry.disposition.as_str() {
            "binding_worthy" => "B",
            "excluded" => "X",
            "adapted" => "A",
            other => other,
        };
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            tsv_cell(disposition),
            tsv_cell(&entry.kind),
            tsv_cell(&entry.path),
            tsv_cell(&entry.category),
            tsv_cell(&entry.reason),
        ));
    }
    output
}

fn tsv_cell(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn append_type_ignore_adaptations(config: &BevyConfig, entries: &mut BTreeSet<ClassifiedEntry>) {
    for (type_name, ignores) in &config.type_ignores {
        let note = ignores
            .notes
            .as_deref()
            .unwrap_or("exact reviewed Python adaptation for this Bevy type");
        let mut add = |suffix: &str, category: &str, reason: String| {
            entries.insert(decision(
                &format!("{type_name}::{suffix}"),
                "adaptation",
                "adapted",
                category,
                &reason,
            ));
        };
        if ignores.ignore_constructor_warnings {
            add("__init__", "constructor_policy", note.to_owned());
        }
        for name in &ignores.ignore_constructor_params {
            add(name, "constructor_parameter", note.to_owned());
        }
        for omission in &ignores.intentional_missing_methods {
            add(
                omission.name(),
                "missing_method",
                omission.note().unwrap_or(note).to_owned(),
            );
        }
        for omission in &ignores.intentional_missing_fields {
            add(
                omission.name(),
                "missing_field",
                omission.note().unwrap_or(note).to_owned(),
            );
        }
        for method in &ignores.runtime_injected_methods {
            add(
                method.name(),
                "runtime_injected_method",
                method.note().unwrap_or(note).to_owned(),
            );
        }
        for name in &ignores.intentional_extra_methods {
            add(name, "python_extension_method", note.to_owned());
        }
        for name in &ignores.deref_target_methods {
            add(name, "deref_target_method", note.to_owned());
        }
        for name in &ignores.intentional_readonly_properties {
            add(name, "readonly_property", note.to_owned());
        }
        for name in &ignores.intentional_not_in_constructor {
            add(name, "not_constructor_settable", note.to_owned());
        }
        for (bevy, python) in &ignores.renames {
            add(
                bevy,
                "python_rename",
                format!("renamed to `{python}` in Python; {note}"),
            );
        }
        for signature in &ignores.intentional_signature_diffs {
            let name = match signature {
                SignatureDiffSpec::Simple(name) | SignatureDiffSpec::WithExpected { name, .. } => {
                    name
                }
            };
            add(name, "signature_adaptation", note.to_owned());
        }
    }
}

fn child_decision(path: &str, kind: &str) -> ClassifiedEntry {
    binding_child(path, kind)
}

fn binding_child(path: &str, kind: &str) -> ClassifiedEntry {
    decision(
        path,
        kind,
        "binding_worthy",
        &format!("public_{kind}"),
        "public API declared on a binding-worthy Bevy type",
    )
}

fn decision(
    path: &str,
    kind: &str,
    disposition: &str,
    category: &str,
    reason: &str,
) -> ClassifiedEntry {
    ClassifiedEntry {
        path: path.to_owned(),
        kind: kind.to_owned(),
        disposition: disposition.to_owned(),
        category: category.to_owned(),
        reason: reason.to_owned(),
    }
}

fn replace_coverage_block(document: &str, report: &CoverageReport) -> Result<String> {
    let begin = document
        .find(COVERAGE_BEGIN)
        .context("docs/coverage.md is missing the generated coverage begin marker")?;
    let end_start = document
        .find(COVERAGE_END)
        .context("docs/coverage.md is missing the generated coverage end marker")?;
    if begin >= end_start {
        bail!("docs/coverage.md coverage markers are out of order");
    }
    let end = end_start + COVERAGE_END.len();
    let block = render_coverage_block(report);
    Ok(format!(
        "{}{}{}",
        &document[..begin],
        block,
        &document[end..]
    ))
}

fn render_coverage_block(report: &CoverageReport) -> String {
    let reverse_module_map: BTreeMap<_, _> = report
        .crates
        .iter()
        .map(|(crate_name, coverage)| {
            (
                coverage
                    .pybevy_module
                    .as_deref()
                    .unwrap_or(crate_name)
                    .to_owned(),
                coverage,
            )
        })
        .collect();
    let mut output = format!(
        "{COVERAGE_BEGIN}\n## Current Coverage (Bevy 0.19)\n\nProvenance: [`{BEVY_TAG}`]({BEVY_REPOSITORY}/tree/{BEVY_REVISION}) at `{BEVY_REVISION}`, generated with `cargo-public-api {CARGO_PUBLIC_API_VERSION} {}`.\n\n| Module | Types | Type % | Methods | Meth % |\n|--------|------:|-------:|--------:|-------:|\n",
        crate::bevy_parser::public_api_flags()
    );
    for (module, coverage) in reverse_module_map {
        let (matched_methods, total_methods) =
            coverage
                .types
                .iter()
                .fold((0usize, 0usize), |(matched, total), type_coverage| {
                    (
                        matched + type_coverage.matched_method_count,
                        total + type_coverage.bevy_method_count,
                    )
                });
        let method_percent = if total_methods == 0 {
            "-".to_owned()
        } else {
            format!(
                "{:.0}%",
                matched_methods as f64 / total_methods as f64 * 100.0
            )
        };
        output.push_str(&format!(
            "| {module} | {}/{} | {:.0}% | {matched_methods}/{total_methods} | {method_percent} |\n",
            coverage.matched_count,
            coverage.bevy_type_count,
            coverage.coverage_percent(),
        ));
    }
    output.push_str(&format!(
        "| **Total** | **{}/{}** | **{:.0}%** | **{}/{}** | **{:.0}%** |\n\nFields in implemented types: {}/{} ({:.0}%). Enum variants in implemented enums: {}/{} ({:.0}%).\n{COVERAGE_END}",
        report.matched_types,
        report.total_bevy_types,
        report.type_coverage_percent(),
        report.matched_methods,
        report.total_bevy_methods,
        report.method_coverage_percent(),
        report.matched_fields,
        report.total_bevy_fields,
        report.field_coverage_percent(),
        report.matched_variants,
        report.total_bevy_variants,
        report.variant_coverage_percent(),
    ));
    output
}

pub fn check_or_write(path: &Path, expected: &str, update: bool) -> Result<()> {
    if update {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, expected)
            .with_context(|| format!("failed to write generated artifact {}", path.display()))?;
        return Ok(());
    }
    let actual = fs::read_to_string(path)
        .with_context(|| format!("generated artifact is missing: {}", path.display()))?;
    if actual != expected {
        bail!(
            "generated artifact is stale: {}; run `cargo run -p pybevy_lint -- bevy-audit --update` and review the diff",
            path.display()
        );
    }
    Ok(())
}
