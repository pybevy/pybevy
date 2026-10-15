use std::collections::{HashMap, HashSet};

use crate::{
    bevy_parser::types::{BevyCrate, BevyItem, BevyItemKind, BevyVariantKind, foreign_type_key},
    comparison::{SignatureDiff, compare_single_type},
    config::BevyConfig,
    model::PyClassDef,
    output::{Diagnostic, DiagnosticCode},
};

/// One API-drift finding, keyed by the wrapper member it concerns so a
/// reviewed exception covers exactly that member.
#[derive(Debug)]
pub struct DriftFinding {
    pub member: Option<String>,
    pub diagnostic: Diagnostic,
}

/// W038/W039/W040: members, return types and enum shape that drift from the
/// resolved upstream Bevy item.
pub fn check_api_drift(
    class: &PyClassDef,
    bevy_item: &BevyItem,
    crate_key: &str,
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
) -> Vec<DriftFinding> {
    let Some(bevy_crate) = bevy_crates.get(crate_key) else {
        return Vec::new();
    };
    let bevy_item = &with_foreign_methods(bevy_item, bevy_crates);
    let coverage = compare_single_type(bevy_item, class, config, bevy_crate);
    let variant_names: Vec<&str> = bevy_item
        .variants
        .iter()
        .map(|variant| variant.name.as_str())
        .collect();
    let deref_members = deref_members(bevy_item, crate_key, bevy_crates);
    let unexplained_properties = coverage
        .extra_methods
        .iter()
        .filter(|extra| {
            extra.is_property
                && !extra.pybevy_name.starts_with('_')
                && !deref_members.contains(&extra.pybevy_name)
        })
        .count();
    let payload_accessors = !bevy_item.tuple_fields.is_empty()
        && unexplained_properties <= bevy_item.tuple_fields.len();
    let mut findings = Vec::new();

    for extra in &coverage.extra_methods {
        let name = extra.pybevy_name.as_str();
        if name.starts_with('_') || deref_members.contains(name) {
            continue;
        }
        if extra.is_property && payload_accessors {
            continue;
        }
        if bevy_item.kind == BevyItemKind::Enum
            && variant_names.contains(&name.trim_end_matches('_'))
        {
            continue;
        }
        let kind = if extra.is_property {
            "property"
        } else if extra.is_static {
            "static method"
        } else {
            "method"
        };
        findings.push(DriftFinding {
            member: Some(name.to_string()),
            diagnostic: located(
                Diagnostic::warning(
                    DiagnosticCode::W038,
                    format!(
                        "{kind} '{}.{name}' has no counterpart on Bevy '{}'",
                        class.python_name, bevy_item.full_path
                    ),
                )
                .with_note(
                    "remove the Python-only member, or record a reviewed exception naming why Python needs it",
                ),
                class,
            ),
        });
    }

    for method in &coverage.methods {
        if !method.is_implemented {
            continue;
        }
        for difference in &method.differences {
            let (bevy_type, pybevy_type) = match difference {
                SignatureDiff::ReturnTypeMismatch {
                    bevy_type,
                    pybevy_type,
                } if !bevy_type.starts_with("Self::") => (bevy_type.as_str(), pybevy_type.as_str()),
                SignatureDiff::ExpectedTypeMismatch {
                    context,
                    expected,
                    actual,
                } if context == "return" => (expected.as_str(), actual.as_str()),
                _ => continue,
            };
            findings.push(DriftFinding {
                member: Some(method.bevy_name.clone()),
                diagnostic: located(
                    Diagnostic::warning(
                        DiagnosticCode::W039,
                        format!(
                            "'{}.{}' returns '{}' but Bevy '{}::{}' returns '{}'",
                            class.python_name,
                            method.bevy_name,
                            compact(pybevy_type),
                            bevy_item.full_path,
                            method.bevy_name,
                            compact(bevy_type)
                        ),
                    ),
                    class,
                ),
            });
        }
    }

    if coverage.enum_representation_matches == Some(false) {
        let shape = if bevy_item
            .variants
            .iter()
            .any(|variant| variant.kind != BevyVariantKind::Unit)
        {
            "data enum"
        } else {
            "enum"
        };
        findings.push(DriftFinding {
            member: None,
            diagnostic: located(
                Diagnostic::warning(
                    DiagnosticCode::W040,
                    format!(
                        "Bevy {shape} '{}' is wrapped by ordinary class '{}'",
                        bevy_item.full_path, class.python_name
                    ),
                )
                .with_note("expose the variants as exact nested classes through #[pyenum]"),
                class,
            ),
        });
    }

    findings
}

/// The item plus trait-impl methods other crates declare for it.
fn with_foreign_methods(item: &BevyItem, bevy_crates: &HashMap<String, BevyCrate>) -> BevyItem {
    let key = foreign_type_key(&item.full_path);
    let mut item = item.clone();
    for bevy_crate in bevy_crates.values() {
        if let Some(methods) = bevy_crate.foreign_methods.get(&key) {
            item.methods.extend(methods.iter().cloned());
        }
    }
    item
}

/// Fields and methods reachable through the item's `Deref` chain.
fn deref_members(
    item: &BevyItem,
    crate_key: &str,
    bevy_crates: &HashMap<String, BevyCrate>,
) -> HashSet<String> {
    let mut members = HashSet::new();
    let mut current = item.clone();
    let mut current_crate = crate_key.to_string();
    for _ in 0..3 {
        let Some((_, target)) = current
            .associated_types
            .iter()
            .find(|(name, _)| name == "Target")
        else {
            break;
        };
        let Some((target_crate, target_item)) =
            find_deref_target(target, &current_crate, bevy_crates)
        else {
            break;
        };
        let target_item = with_foreign_methods(target_item, bevy_crates);
        members.extend(target_item.fields.iter().map(|field| field.name.clone()));
        if let Some(target_bevy_crate) = bevy_crates.get(target_crate) {
            members.extend(
                target_bevy_crate
                    .get_methods_for_item(&target_item)
                    .into_iter()
                    .map(|method| method.name.clone()),
            );
            let base = target_item.name.as_str();
            for (name, specialized) in &target_bevy_crate.items {
                if name.split('<').next() == Some(base) {
                    members.extend(specialized.methods.iter().map(|method| method.name.clone()));
                }
            }
        }
        current_crate = target_crate.to_string();
        current = target_item;
    }
    members
}

fn find_deref_target<'a>(
    target: &str,
    preferred_crate: &str,
    bevy_crates: &'a HashMap<String, BevyCrate>,
) -> Option<(&'a str, &'a BevyItem)> {
    let base = target.split('<').next().unwrap_or(target);
    let name = base.rsplit("::").next().unwrap_or(base);
    let matches: Vec<(&str, &BevyItem)> = bevy_crates
        .iter()
        .filter_map(|(crate_name, bevy_crate)| {
            bevy_crate
                .items
                .get(name)
                .filter(|item| matches!(item.kind, BevyItemKind::Struct | BevyItemKind::Enum))
                .map(|item| (crate_name.as_str(), item))
        })
        .collect();
    matches
        .iter()
        .find(|(crate_name, _)| *crate_name == preferred_crate)
        .or_else(|| (matches.len() == 1).then(|| &matches[0]))
        .copied()
}

fn located(diagnostic: Diagnostic, class: &PyClassDef) -> Diagnostic {
    match &class.location {
        Some(location) => diagnostic.with_location(location.clone()),
        None => diagnostic,
    }
}

fn compact(ty: &str) -> String {
    ty.split_whitespace().collect::<Vec<_>>().join(" ")
}
