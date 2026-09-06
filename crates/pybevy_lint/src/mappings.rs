//! Fail-closed field mapping IR for the parity reflection oracle.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::{
    bevy_parser::{BevyCrate, BevyField, BevyItem},
    config::{BevyConfig, FieldMappingOverride},
    model::{BridgeStorageKind, PropertyDef, PyClassDef},
};

pub const FIELD_MAP_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldMapProvenance {
    pub bevy_tag: String,
    pub bevy_revision: String,
    pub cargo_public_api: String,
}

impl FieldMapProvenance {
    pub fn pinned_bevy_019() -> Self {
        Self {
            bevy_tag: crate::bevy_audit::BEVY_TAG.to_owned(),
            bevy_revision: crate::bevy_audit::BEVY_REVISION.to_owned(),
            cargo_public_api: crate::bevy_audit::CARGO_PUBLIC_API_VERSION.to_owned(),
        }
    }

    fn unverified() -> Self {
        Self {
            bevy_tag: "unverified".to_owned(),
            bevy_revision: "unverified".to_owned(),
            cargo_public_api: "unverified".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldMapArtifact {
    pub version: u32,
    pub provenance: FieldMapProvenance,
    pub types: BTreeMap<String, TypeFieldMap>,
    pub uncovered_types: Vec<String>,
    pub uncovered_type_reasons: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TypeFieldMap {
    pub bevy_type: String,
    pub bevy_path: String,
    pub fields: Vec<FieldMapping>,
    pub excluded: Vec<ExcludedField>,
    pub uncovered: Vec<UncoveredField>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldMapping {
    pub bevy_name: String,
    pub pybevy_name: String,
    pub bevy_type: String,
    pub pybevy_type: String,
    pub normalizer: NormalizerKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExcludedField {
    pub bevy_name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UncoveredField {
    pub bevy_name: String,
    pub bevy_type: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NormalizerKind {
    Identity,
    NewtypeValue,
    RangeTuple,
    EnumVariantName,
    HandleToAssetRef,
    NestedRecurse,
}

impl NormalizerKind {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "identity" => Ok(Self::Identity),
            "newtype-value" => Ok(Self::NewtypeValue),
            "range-tuple" => Ok(Self::RangeTuple),
            "enum-variant-name" => Ok(Self::EnumVariantName),
            "handle-to-asset-ref" => Ok(Self::HandleToAssetRef),
            "nested-recurse" => Ok(Self::NestedRecurse),
            other => bail!("unknown field-map normalizer {other:?}"),
        }
    }
}

pub fn generate_field_mappings(
    classes: &[PyClassDef],
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
) -> Result<FieldMapArtifact> {
    generate_field_mappings_with_provenance(
        classes,
        bevy_crates,
        config,
        FieldMapProvenance::unverified(),
    )
}

pub fn generate_field_mappings_with_provenance(
    classes: &[PyClassDef],
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
    provenance: FieldMapProvenance,
) -> Result<FieldMapArtifact> {
    let mut types = BTreeMap::new();
    let mut uncovered_types = BTreeSet::new();
    let mut uncovered_type_reasons = BTreeMap::new();

    for class in classes {
        let Some(bridge) = &class.bridge_info else {
            if class.extends.as_deref() == Some("PyComponent") {
                uncovered_types.insert(class.python_name.clone());
                uncovered_type_reasons.insert(
                    class.python_name.clone(),
                    "component wrapper has no generated bridge metadata".to_owned(),
                );
            }
            continue;
        };
        if bridge.storage_kind != BridgeStorageKind::Component {
            continue;
        }
        if bridge.no_reflect {
            uncovered_types.insert(class.python_name.clone());
            uncovered_type_reasons.insert(
                class.python_name.clone(),
                "component bridge explicitly disables reflection".to_owned(),
            );
            continue;
        }

        let bevy_type = short_type_name(&bridge.bevy_type);
        let Some(bevy_item) = find_bevy_item(&bevy_type, class, bevy_crates, config) else {
            uncovered_types.insert(class.python_name.clone());
            uncovered_type_reasons.insert(
                class.python_name.clone(),
                "wrapped Bevy type is absent or ambiguous in the pinned public-API inventory"
                    .to_owned(),
            );
            continue;
        };

        let overrides = config
            .mapping_overrides
            .get(&class.python_name)
            .cloned()
            .unwrap_or_default();
        validate_overrides(class, bevy_item, &overrides)?;

        let mut fields = Vec::new();
        let mut excluded = Vec::new();
        let mut uncovered = Vec::new();
        for bevy_field in bevy_item.fields.iter().filter(|field| field.is_public) {
            let field_override = overrides.get(&bevy_field.name);
            if let Some(reason) = field_override.and_then(|value| value.exclude_reason.as_deref()) {
                if reason.trim().is_empty() {
                    bail!(
                        "{}.{} has an empty exclude_reason",
                        class.python_name,
                        bevy_field.name
                    );
                }
                excluded.push(ExcludedField {
                    bevy_name: bevy_field.name.clone(),
                    reason: reason.to_string(),
                });
                continue;
            }

            let pybevy_name = mapped_python_name(
                class,
                bevy_field,
                bevy_item.fields.len(),
                field_override,
                config,
            );
            let Some(pybevy_name) = pybevy_name else {
                if config.is_intentional_missing_field(&class.python_name, &bevy_field.name) {
                    let ignores = config.get_type_ignores(&class.python_name);
                    let reason = ignores
                        .and_then(|value| {
                            value
                                .intentional_missing_fields
                                .iter()
                                .find(|field| field.name() == bevy_field.name)
                                .and_then(|field| field.note())
                                .or(value.notes.as_deref())
                        })
                        .unwrap_or("configured intentional omission");
                    excluded.push(ExcludedField {
                        bevy_name: bevy_field.name.clone(),
                        reason: reason.to_string(),
                    });
                } else {
                    uncovered.push(UncoveredField {
                        bevy_name: bevy_field.name.clone(),
                        bevy_type: bevy_field.field_type.clone(),
                        reason: "no readable Python property or exact exclusion maps this public Bevy field"
                            .to_owned(),
                    });
                }
                continue;
            };

            let property = class
                .properties
                .iter()
                .find(|property| property.name == pybevy_name)
                .expect("mapped_python_name only returns existing properties");
            let Some(pybevy_type) = property.property_type.clone() else {
                bail!(
                    "{}.{} maps to property {} without a parsed type",
                    class.python_name,
                    bevy_field.name,
                    pybevy_name
                );
            };
            let normalizer = match field_override.and_then(|value| value.normalizer.as_deref()) {
                Some(value) => NormalizerKind::parse(value)?,
                None => infer_normalizer(bevy_field, property, bevy_crates),
            };
            fields.push(FieldMapping {
                bevy_name: bevy_field.name.clone(),
                pybevy_name,
                bevy_type: bevy_field.field_type.clone(),
                pybevy_type,
                normalizer,
            });
        }

        fields.sort_by(|left, right| left.bevy_name.cmp(&right.bevy_name));
        excluded.sort_by(|left, right| left.bevy_name.cmp(&right.bevy_name));
        uncovered.sort_by(|left, right| left.bevy_name.cmp(&right.bevy_name));
        if fields.is_empty()
            && excluded.is_empty()
            && uncovered.is_empty()
            && class.properties.iter().any(|property| property.has_getter)
        {
            // Opaque/newtype Bevy components can expose useful Python getters
            // while presenting no public named field to cargo-public-api.
            // Keep their descriptor snapshot and report the missing oracle.
            uncovered_types.insert(class.python_name.clone());
            uncovered_type_reasons.insert(
                class.python_name.clone(),
                "upstream type has no public named fields but the Python wrapper exposes readable properties"
                    .to_owned(),
            );
        }
        if types.contains_key(&class.python_name) {
            bail!(
                "multiple wrapped components are named {}; field-map keys must be unique",
                class.python_name
            );
        }
        types.insert(
            class.python_name.clone(),
            TypeFieldMap {
                bevy_type,
                bevy_path: bevy_item.full_path.clone(),
                fields,
                excluded,
                uncovered,
            },
        );
    }

    for component_name in &config.reflection_unavailable_types {
        if !types.contains_key(component_name) && !uncovered_types.contains(component_name) {
            bail!(
                "reflection_unavailable_types references unknown wrapped component {component_name}"
            );
        }
        uncovered_types.insert(component_name.clone());
        uncovered_type_reasons
            .entry(component_name.clone())
            .or_insert_with(|| "wrapped Bevy type does not expose reflection data".to_owned());
    }

    let uncovered_type_names: BTreeSet<_> = uncovered_types.iter().cloned().collect();
    let reasoned_type_names: BTreeSet<_> = uncovered_type_reasons.keys().cloned().collect();
    if uncovered_type_names != reasoned_type_names {
        bail!("every uncovered field-map type must have exactly one reason");
    }
    if uncovered_type_reasons
        .values()
        .any(|reason| reason.trim().is_empty())
        || types.values().any(|type_map| {
            type_map
                .uncovered
                .iter()
                .any(|field| field.reason.trim().is_empty())
        })
    {
        bail!("field-map exclusion and uncovered reasons must be non-empty");
    }

    Ok(FieldMapArtifact {
        version: FIELD_MAP_VERSION,
        provenance,
        types,
        uncovered_types: uncovered_types.into_iter().collect(),
        uncovered_type_reasons,
    })
}

fn short_type_name(value: &str) -> String {
    let compact = value.replace(' ', "");
    compact
        .split('<')
        .next()
        .unwrap_or(&compact)
        .rsplit("::")
        .next()
        .unwrap_or(&compact)
        .to_string()
}

fn find_bevy_item<'a>(
    bevy_type: &str,
    class: &PyClassDef,
    bevy_crates: &'a HashMap<String, BevyCrate>,
    config: &BevyConfig,
) -> Option<&'a BevyItem> {
    let preferred_crate = class
        .location
        .as_ref()
        .and_then(|location| module_from_path(&location.file))
        .and_then(|module| config.crate_mappings.get(&module));
    if let Some(preferred_crate) = preferred_crate
        && let Some(item) = bevy_crates
            .get(preferred_crate)
            .and_then(|bevy_crate| bevy_crate.items.get(bevy_type))
    {
        return Some(item);
    }

    let mut matches = bevy_crates
        .values()
        .filter_map(|bevy_crate| bevy_crate.items.get(bevy_type));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn module_from_path(path: &std::path::Path) -> Option<String> {
    let components: Vec<_> = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    if let Some(index) = components
        .iter()
        .position(|component| *component == "crates")
    {
        return components
            .get(index + 1)
            .and_then(|component| component.strip_prefix("pybevy_"))
            .map(str::to_string);
    }
    components
        .iter()
        .position(|component| *component == "src")
        .and_then(|index| components.get(index + 1))
        .map(|value| value.to_string())
}

fn validate_overrides(
    class: &PyClassDef,
    bevy_item: &BevyItem,
    overrides: &HashMap<String, FieldMappingOverride>,
) -> Result<()> {
    for (bevy_name, field_override) in overrides {
        if !bevy_item
            .fields
            .iter()
            .any(|field| field.name == *bevy_name)
        {
            bail!(
                "mapping override references missing Bevy field {}.{}",
                class.python_name,
                bevy_name
            );
        }
        if field_override.pybevy_name.is_some() && field_override.exclude_reason.is_some() {
            bail!(
                "mapping override {}.{} cannot map and exclude the same field",
                class.python_name,
                bevy_name
            );
        }
        if let Some(pybevy_name) = &field_override.pybevy_name
            && !class
                .properties
                .iter()
                .any(|property| property.name == *pybevy_name)
        {
            bail!(
                "mapping override references missing PyBevy property {}.{}",
                class.python_name,
                pybevy_name
            );
        }
    }
    Ok(())
}

fn mapped_python_name(
    class: &PyClassDef,
    bevy_field: &BevyField,
    bevy_field_count: usize,
    field_override: Option<&FieldMappingOverride>,
    config: &BevyConfig,
) -> Option<String> {
    let candidates = [
        field_override.and_then(|value| value.pybevy_name.clone()),
        config
            .get_rename(&class.python_name, &bevy_field.name)
            .map(str::to_string),
        Some(bevy_field.name.clone()),
        Some(format!("{}_", bevy_field.name)),
        (bevy_field_count == 1).then(|| "value".to_string()),
    ];
    candidates.into_iter().flatten().find(|candidate| {
        class
            .properties
            .iter()
            .any(|property| property.name == *candidate && property.has_getter)
    })
}

fn infer_normalizer(
    bevy_field: &BevyField,
    property: &PropertyDef,
    bevy_crates: &HashMap<String, BevyCrate>,
) -> NormalizerKind {
    let bevy_type = bevy_field.field_type.replace(' ', "");
    if bevy_type.contains("Handle<") {
        return NormalizerKind::HandleToAssetRef;
    }
    if bevy_type.contains("Range<") {
        return NormalizerKind::RangeTuple;
    }
    if property.name == "value" {
        return NormalizerKind::NewtypeValue;
    }
    let short = short_type_name(&bevy_type);
    if bevy_crates.values().any(|bevy_crate| {
        bevy_crate
            .items
            .get(&short)
            .is_some_and(|item| !item.variants.is_empty())
    }) {
        return NormalizerKind::EnumVariantName;
    }
    if is_identity_type(&bevy_type) {
        NormalizerKind::Identity
    } else {
        NormalizerKind::NestedRecurse
    }
}

fn is_identity_type(value: &str) -> bool {
    matches!(
        value,
        "bool"
            | "f32"
            | "f64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "usize"
            | "String"
    )
}
