use std::collections::{HashMap, HashSet};

use crate::{
    bevy_parser::types::{
        BevyCrate, BevyField, BevyItem, BevyItemKind, BevyVariantKind, foreign_type_key,
    },
    comparison::{ConversionIndex, SignatureDiff, compare_single_type},
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

/// W038-W041: members, return types, enum shape and contract-trait methods
/// that drift from the resolved upstream Bevy item.
pub fn check_api_drift(
    class: &PyClassDef,
    bevy_item: &BevyItem,
    crate_key: &str,
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
    conversions: &ConversionIndex,
) -> Vec<DriftFinding> {
    let (bevy_item, crate_key) = resolve_alias(bevy_item, crate_key, bevy_crates);
    let Some(bevy_crate) = bevy_crates.get(crate_key) else {
        return Vec::new();
    };
    let bevy_item = &drift_view(bevy_item, bevy_crates, config);
    let coverage = compare_single_type(bevy_item, class, config, bevy_crate, conversions);
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
                } if !bevy_type.starts_with("Self::") && !mentions_type_parameter(bevy_type) => {
                    (bevy_type.as_str(), pybevy_type.as_str())
                }
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

    for method in &coverage.methods {
        if method.is_implemented {
            continue;
        }
        let Some(trait_path) = method.bevy_trait.as_deref() else {
            continue;
        };
        let trait_name = trait_path.rsplit("::").next().unwrap_or(trait_path);
        if !config
            .contract_traits
            .iter()
            .any(|contract| contract == trait_name)
        {
            continue;
        }
        findings.push(DriftFinding {
            member: Some(method.bevy_name.clone()),
            diagnostic: located(
                Diagnostic::warning(
                    DiagnosticCode::W041,
                    format!(
                        "'{}' lacks '{}' from Bevy trait {trait_name}, which '{}' implements",
                        class.python_name, method.bevy_name, bevy_item.full_path
                    ),
                ),
                class,
            ),
        });
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

/// A type alias compares as the type it names (`ForwardDecalMaterial<B>`
/// is `ExtendedMaterial<B, ForwardDecalMaterialExt>`).
fn resolve_alias<'a>(
    item: &'a BevyItem,
    crate_key: &'a str,
    bevy_crates: &'a HashMap<String, BevyCrate>,
) -> (&'a BevyItem, &'a str) {
    if item.kind != BevyItemKind::TypeAlias {
        return (item, crate_key);
    }
    item.alias_of
        .as_deref()
        .and_then(|target| find_deref_target(target, crate_key, bevy_crates))
        .map(|(target_crate, target)| (target, target_crate))
        .unwrap_or((item, crate_key))
}

/// The item as Rust callers see it: plus trait-impl methods other crates
/// declare for it, provided methods of the traits it implements (wherever
/// they are declared), blanket-impl traits from `bevy.blanket_impls`, and
/// field names a contract-only merge kept out of `fields`.
fn drift_view(
    item: &BevyItem,
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
) -> BevyItem {
    let mut view = with_foreign_methods(item, bevy_crates);
    for template in unmarked_templates(item, bevy_crates) {
        view.methods.extend(template.methods.iter().cloned());
    }
    view.fields
        .extend(item.unordered_field_names.iter().map(|name| BevyField {
            name: name.clone(),
            field_type: String::new(),
            is_public: true,
        }));
    let blanket = config
        .blanket_impls
        .get(item.name.split('<').next().unwrap_or(&item.name))
        .into_iter()
        .flatten();
    let traits: Vec<String> = item
        .trait_impls
        .iter()
        .cloned()
        .chain(blanket.cloned())
        .collect();
    for trait_path in traits {
        let Some(trait_item) = find_trait(&trait_path, bevy_crates) else {
            continue;
        };
        for method in &trait_item.methods {
            if !view
                .methods
                .iter()
                .any(|existing| existing.name == method.name)
            {
                let mut method = method.clone();
                method.from_trait = Some(trait_path.clone());
                view.methods.push(method);
            }
        }
    }
    view
}

/// Generic impls the parser did not mark as templates because their
/// parameters have descriptive names (`NodeIndex<Ix>`,
/// `GizmoBuffer<Config, Clear>`): same base name, and no argument names a
/// known type, so it is not a specialization such as `Time<Virtual>`.
fn unmarked_templates<'a>(
    item: &BevyItem,
    bevy_crates: &'a HashMap<String, BevyCrate>,
) -> Vec<&'a BevyItem> {
    if item.name.contains('<') {
        return Vec::new();
    }
    let known = |name: &str| {
        name.chars().next().is_some_and(char::is_lowercase)
            || bevy_crates
                .values()
                .any(|bevy_crate| bevy_crate.items.contains_key(name))
    };
    bevy_crates
        .values()
        .flat_map(|bevy_crate| bevy_crate.items.values())
        .filter(|candidate| !candidate.is_generic_base)
        .filter(|candidate| {
            let Some((base, args)) = candidate.name.split_once('<') else {
                return false;
            };
            base == item.name
                && candidate.full_path.split("::").next() == item.full_path.split("::").next()
                && args
                    .trim_end_matches('>')
                    .split(',')
                    .map(str::trim)
                    .filter(|arg| !arg.is_empty() && !arg.starts_with('\''))
                    .all(|arg| !known(arg))
        })
        .collect()
}

fn find_trait<'a>(
    trait_path: &str,
    bevy_crates: &'a HashMap<String, BevyCrate>,
) -> Option<&'a BevyItem> {
    let short_name = trait_path.rsplit("::").next().unwrap_or(trait_path);
    let root = trait_path.split("::").next().unwrap_or(trait_path);
    let matches: Vec<(&str, &BevyItem)> = bevy_crates
        .iter()
        .filter_map(|(crate_name, bevy_crate)| {
            bevy_crate
                .items
                .get(short_name)
                .filter(|item| item.kind == BevyItemKind::Trait)
                .map(|item| (crate_name.as_str(), item))
        })
        .collect();
    matches
        .iter()
        .find(|(crate_name, _)| *crate_name == root)
        .or_else(|| (matches.len() == 1).then(|| &matches[0]))
        .map(|(_, item)| *item)
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
            .find(|(name, _)| name == "Deref::Target")
        else {
            break;
        };
        if let Some(fields) = glam_deref_fields(target) {
            members.extend(fields.iter().map(|field| field.to_string()));
            break;
        }
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

/// glam's SIMD types deref to its `deref` helper structs, which
/// cargo-public-api files under the same short names as the real vectors
/// (glam 0.32.1 `src/deref.rs`).
fn glam_deref_fields(target: &str) -> Option<&'static [&'static str]> {
    let base = target.split('<').next().unwrap_or(target);
    match base.strip_prefix("glam::deref::")? {
        "Vec3" => Some(&["x", "y", "z"]),
        "Vec4" => Some(&["x", "y", "z", "w"]),
        "Cols2" => Some(&["x_axis", "y_axis"]),
        "Cols3" => Some(&["x_axis", "y_axis", "z_axis"]),
        "Cols4" => Some(&["x_axis", "y_axis", "z_axis", "w_axis"]),
        _ => None,
    }
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

/// An unresolved generic parameter (`T` in a provided `Curve<T>` method)
/// cannot be compared without the impl's arguments.
fn mentions_type_parameter(ty: &str) -> bool {
    ty.split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
        .any(|token| token.len() == 1 && token.chars().all(|ch| ch.is_ascii_uppercase()))
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
