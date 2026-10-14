pub mod bridge_rules;
pub mod class_rules;
pub mod drift_rules;
pub mod method_rules;
pub mod origin;
pub mod prelude_rules;
pub mod property_rules;
mod protocol_rules;
mod specialized_receivers;
pub mod style_rules;
pub mod wrapper_types;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use crate::{
    bevy_parser::types::{BevyCrate, BevyItem},
    comparison::pybevy_bevy_name,
    config::Config,
    model::{
        ConstructorOrigin, EnumVariantDef, EnumVariantKind, MacroInfo, MethodDef, ParameterDef,
        PropertyDef, PyClassDef,
    },
    output::{Diagnostic, DiagnosticCode},
    validation::method_rules::SharedBorrowTarget,
};

/// Validate all Rust classes against Python stubs
pub fn validate_all(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: Option<&Config>,
) -> Vec<Diagnostic> {
    validate_all_impl(rust_classes, python_classes, config, true)
}

/// Validate a filtered subset without treating out-of-scope exceptions as stale.
pub fn validate_scoped(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: Option<&Config>,
) -> Vec<Diagnostic> {
    validate_all_impl(rust_classes, python_classes, config, false)
}

/// Validate with pinned Bevy declarations available: constructor-policy
/// checks run per class against the resolved audited origin. A class whose
/// origin cannot be resolved reports E013 (fail closed).
pub fn validate_with_bevy(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: Option<&Config>,
    bevy_crates: &HashMap<String, BevyCrate>,
    bevy_path: Option<&std::path::Path>,
    check_stale_exceptions: bool,
    internal_type_names: &HashSet<String>,
) -> Vec<Diagnostic> {
    let mut consumed_exceptions = config
        .map(|config| vec![false; config.validation.exceptions.len()])
        .unwrap_or_default();
    let mut diagnostics = validate_all_impl_with_consumed(
        rust_classes,
        python_classes,
        config,
        &mut consumed_exceptions,
        false,
    );

    let Some(config) = config else {
        return diagnostics;
    };

    if check_stale_exceptions {
        diagnostics.extend(specialized_receivers::audit_mappings(rust_classes, config));
        diagnostics.extend(origin::audit_constructor_mappings(
            rust_classes,
            &config.bevy,
        ));
    }

    let wrapper_index = wrapper_types::wrapper_index(rust_classes, &config.bevy);
    let unwrapped = wrapper_types::unwrapped_bevy_types(
        rust_classes,
        python_classes,
        &config.bevy,
        bevy_crates,
    );
    let wrapper_rust_names: HashSet<String> = rust_classes
        .iter()
        .filter(|class| {
            class
                .python_module_path
                .as_deref()
                .is_some_and(|module| module.starts_with("pybevy"))
        })
        .map(|class| class.rust_name.clone())
        .collect();
    let mut non_option_ctor_params: Vec<(String, String)> = Vec::new();
    for rust_class in rust_classes {
        if rust_class.python_name.starts_with('_') {
            continue;
        }
        let in_scope = rust_class.constructor.is_some()
            || !rust_class.properties.is_empty()
            || rust_class.is_enum
            || rust_class
                .enum_variants
                .iter()
                .any(|variant| variant.constructor.is_some());
        if !in_scope {
            continue;
        }

        let origin = if rust_class.constructor.is_some() {
            origin::resolve_constructor_origin(rust_class, bevy_crates, &config.bevy, bevy_path)
        } else {
            None
        };

        let mut policy_diagnostics = origin
            .as_ref()
            .map(|origin| origin::check_constructor_policy(origin, rust_class))
            .unwrap_or_default();

        let (resolved_bevy, mut resolution_diagnostics) =
            resolve_bevy_item(rust_class, bevy_crates, config, internal_type_names);
        policy_diagnostics.append(&mut resolution_diagnostics);

        if let Some(origin) = &origin
            && let Some(upstream_type) = origin.upstream_type().map(str::to_string).or_else(|| {
                matches!(
                    origin,
                    crate::model::ConstructorOrigin::Unresolved
                        | crate::model::ConstructorOrigin::PythonAdapter { .. }
                )
                .then(|| crate::comparison::pybevy_bevy_name(rust_class, &config.bevy))
            })
            && let Some(field_types) = origin::source_field_types(&upstream_type, bevy_path)
        {
            policy_diagnostics.extend(method_rules::check_optional_field_parameters(
                origin,
                rust_class,
                find_matching_python_class(rust_class, python_classes),
                &field_types,
            ));
            for upstream in
                method_rules::non_option_upstream_params(origin, rust_class, &field_types)
            {
                non_option_ctor_params
                    .push((rust_class.python_name.clone(), upstream.param.name.clone()));
            }
        }

        if let Some(python_class) = find_matching_python_class(rust_class, python_classes) {
            let upstream = upstream_slots(
                rust_class,
                origin.as_ref(),
                bevy_crates,
                &config.bevy,
                bevy_path,
            );
            policy_diagnostics.extend(wrapper_types::check_wrapper_types(
                rust_class,
                python_class,
                origin.as_ref(),
                &upstream,
                &wrapper_index,
                &unwrapped,
            ));
        }

        if let Some((_, bevy_item)) = resolved_bevy
            && origin::source_struct_is_generic(&bevy_item.full_path, bevy_path) == Some(false)
        {
            policy_diagnostics.extend(wrapper_types::check_hand_mirrored_value(
                rust_class, bevy_item,
            ));
        }

        if let Some(MacroInfo::BevyEnum { .. }) = &rust_class.macro_info
            && let Some((_, bevy_item)) = resolved_bevy
        {
            policy_diagnostics.extend(class_rules::validate_enum_hashability(
                rust_class, bevy_item,
            ));
        }

        // W021: extra-hash parity for both pyenum and plain classes with a
        // resolved upstream item; mutually exclusive with W015.
        if let Some((_, bevy_item)) = resolved_bevy {
            policy_diagnostics.extend(class_rules::validate_extra_hash(rust_class, bevy_item));
        }

        // W022: the stub must not promise writability the runtime rejects.
        if let Some(python_class) = find_matching_python_class(rust_class, python_classes) {
            policy_diagnostics.extend(property_rules::validate_stub_writability(
                rust_class,
                python_class,
            ));
        }

        // W022 (payload arm): writable stub variant payload over a
        // non-borrowed storage field is a detached write.
        policy_diagnostics.extend(property_rules::validate_nested_variant_payload_writability(
            rust_class,
            python_classes,
        ));

        // W023: nested wrapper fields must not use plain field getters.
        policy_diagnostics.extend(class_rules::validate_nested_wrapper_getters(
            rust_class,
            &wrapper_rust_names,
        ));

        // W024: non-Copy upstream constants need factory exposure.
        if let Some(python_class) = find_matching_python_class(rust_class, python_classes)
            && let Some((_, bevy_item)) = resolved_bevy
        {
            policy_diagnostics.extend(class_rules::validate_constant_typing(
                rust_class,
                python_class,
                bevy_item,
            ));
        }

        // W026: upstream tuple-payload variants must stay positional.
        if let Some((_, bevy_item)) = resolved_bevy
            && rust_class.is_enum
        {
            policy_diagnostics.extend(class_rules::validate_tuple_variant_exposure(
                rust_class, bevy_item,
            ));
        }

        // W031: exposed variant names must exist in the resolved upstream
        // enum (renamed/misspelled variant detection).
        if let Some((_, bevy_item)) = resolved_bevy
            && rust_class.is_enum
        {
            policy_diagnostics.extend(class_rules::validate_variant_name_parity(
                rust_class,
                bevy_item,
                &config.bevy,
            ));
        }

        // W031 (struct-spelling arm): a struct wrapper mirroring upstream
        // enum variants as classattrs/builder methods must spell them with
        // the upstream names.
        if let Some((_, bevy_item)) = resolved_bevy
            && !rust_class.is_enum
        {
            policy_diagnostics.extend(class_rules::validate_struct_variant_spelling(
                rust_class,
                bevy_item,
                &config.bevy,
            ));
        }

        // W033: stub `__match_args__` must match the variant payload fields.
        if rust_class.is_enum
            && let Some(python_class) = find_matching_python_class(rust_class, python_classes)
        {
            policy_diagnostics.extend(class_rules::validate_match_args_conformance(
                rust_class,
                python_class,
            ));
        }

        // W029: the Python registration module must match the canonical
        // Bevy placement for the resolved upstream item.
        if let Some((crate_key, bevy_item)) = resolved_bevy {
            policy_diagnostics.extend(class_rules::validate_module_placement(
                rust_class,
                crate_key,
                bevy_item,
                &config.bevy,
            ));
        }

        // W037: a Bevy `Default` type must be constructible with no arguments.
        if let Some((_, bevy_item)) = resolved_bevy {
            policy_diagnostics.extend(class_rules::validate_default_constructor(
                rust_class, bevy_item,
            ));
        }

        // W035: stub `Handle[T]` annotations must name the upstream asset
        // type of the field they document.
        if let Some((_, bevy_item)) = resolved_bevy
            && let Some(python_class) = find_matching_python_class(rust_class, python_classes)
        {
            policy_diagnostics.extend(class_rules::validate_handle_argument_parity(
                rust_class,
                python_class,
                bevy_item,
                bevy_path,
            ));

            // W036 property arm: the stub's `None` acceptance for a readable
            // property must match the upstream field's `Option` status.
            policy_diagnostics.extend(class_rules::validate_property_optionality_parity(
                rust_class,
                python_class,
                bevy_item,
                bevy_path,
            ));
        }

        // Generated variants are covered even when the base has no
        // constructor: merge the stub-side variant constructors.
        if rust_class.is_enum {
            let python_variants: Vec<&EnumVariantDef> =
                match find_matching_python_class(rust_class, python_classes) {
                    Some(py) => py.enum_variants.iter().collect(),
                    None => Vec::new(),
                };
            let mut combined: Vec<(String, &EnumVariantDef)> = Vec::new();
            for variant in &rust_class.enum_variants {
                if variant.constructor.is_some() {
                    combined.push((variant.name.clone(), variant));
                }
            }
            for variant in python_variants {
                if variant.constructor.is_some()
                    && !combined.iter().any(|(name, _)| name == &variant.name)
                {
                    combined.push((variant.name.clone(), variant));
                }
            }
            let mut seen: HashSet<String> = HashSet::new();
            for (variant_name, variant) in combined {
                if seen.contains(&variant_name) {
                    continue;
                }
                seen.insert(variant_name.clone());
                policy_diagnostics.extend(variant_policy(
                    rust_class,
                    variant,
                    bevy_crates,
                    &config.bevy,
                    bevy_path,
                ));
            }
        }

        let path = class_path(rust_class);
        if origin.is_none() && rust_class.constructor.is_some() {
            let code = Diagnostic::error(
                DiagnosticCode::E013,
                format!(
                    "constructor '{}' has no resolved audited origin from the pinned Bevy source; the upstream declaration must be resolved before constructor-policy checks can pass",
                    rust_class.python_name
                ),
            )
            .with_note(format!("parser-origin path: {}", path));
            // Fail closed, but reviewed per-class exceptions may document a
            // genuinely Python-specific adaptation whose constructor has no
            // upstream identity.
            if !consume_exception(Some(config), &mut consumed_exceptions, &path, &code) {
                diagnostics.push(code);
            }
        }
        // A reviewed policy exception covers every diagnostic of its class
        // and code (a class can emit several E014s for its parameters), so
        // the exception is consumed on the first one and suppresses the rest.
        let mut policy_seen: HashSet<(String, String)> = HashSet::new();
        for diagnostic in policy_diagnostics {
            if is_disabled_code(Some(config), diagnostic.code.as_str()) {
                continue;
            }
            let key: (String, String) = (path.clone(), diagnostic.code.as_str().to_string());
            if policy_seen.contains(&key) {
                continue;
            }
            if consume_exception(Some(config), &mut consumed_exceptions, &path, &diagnostic) {
                if config.validation.exceptions.iter().any(|exception| {
                    exception.path == path
                        && exception.code == diagnostic.code.as_str()
                        && exception.message.is_none()
                        && !exception.reason.trim().is_empty()
                }) {
                    policy_seen.insert(key);
                }
                continue;
            }
            diagnostics.push(diagnostic);
        }
    }

    // W038-W040 compare every public wrapper with a resolved upstream item,
    // including method-only classes the constructor-policy loop skips.
    for rust_class in rust_classes {
        if rust_class.python_name.starts_with('_') {
            continue;
        }
        let (Some((crate_key, bevy_item)), _) =
            resolve_bevy_item(rust_class, bevy_crates, config, internal_type_names)
        else {
            continue;
        };
        let path = class_path(rust_class);
        for finding in drift_rules::check_api_drift(
            rust_class,
            bevy_item,
            crate_key,
            bevy_crates,
            &config.bevy,
        ) {
            if is_disabled_code(Some(config), finding.diagnostic.code.as_str()) {
                continue;
            }
            let finding_path = match &finding.member {
                Some(member) => format!("{path}.{member}"),
                None => path.clone(),
            };
            if consume_exception(
                Some(config),
                &mut consumed_exceptions,
                &finding_path,
                &finding.diagnostic,
            ) {
                continue;
            }
            diagnostics.push(
                finding
                    .diagnostic
                    .with_note(format!("exception path: {finding_path}")),
            );
        }
    }

    // W017 owns inputs whose upstream is not an `Option`: the fix is to reject
    // `None` in Rust, not to advertise it in the stub.
    diagnostics.retain(|diagnostic| {
        !non_option_ctor_params
            .iter()
            .any(|(class, param)| method_rules::is_w036_stub_none_advice(diagnostic, class, param))
    });

    // W030: the curated prelude must mirror the pinned Bevy prelude for
    // every name PyBevy implements.
    let prelude_path = "pybevy.prelude".to_string();
    for diagnostic in
        prelude_rules::check_prelude_parity(rust_classes, bevy_path, Path::new("pybevy/prelude.py"))
    {
        if is_disabled_code(Some(config), diagnostic.code.as_str()) {
            continue;
        }
        if consume_exception(
            Some(config),
            &mut consumed_exceptions,
            &prelude_path,
            &diagnostic,
        ) {
            continue;
        }
        diagnostics.push(diagnostic);
    }

    // Exactly-once consumption across the standard and constructor-policy
    // passes, with one stale-exception check when requested.
    if check_stale_exceptions {
        for (exception, consumed) in config
            .validation
            .exceptions
            .iter()
            .zip(consumed_exceptions.iter())
        {
            if !*consumed {
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
    }

    diagnostics
}

/// The upstream item a wrapper resolves to: an explicit claim (macro, bridge
/// or configured type mapping) resolves strictly and reports W025 on
/// failure; the implicit Py-strip fallback stays silent.
fn resolve_bevy_item<'a>(
    rust_class: &PyClassDef,
    bevy_crates: &'a HashMap<String, BevyCrate>,
    config: &Config,
    internal_type_names: &HashSet<String>,
) -> (Option<(&'a str, &'a BevyItem)>, Vec<Diagnostic>) {
    let claimed_bevy_name: Option<String> = rust_class
        .macro_info
        .as_ref()
        .map(|MacroInfo::BevyEnum { bevy_type, .. }| bevy_type.clone())
        .or_else(|| rust_class.bridge_info.as_ref().map(|b| b.bevy_type.clone()))
        .or_else(|| {
            config
                .bevy
                .type_mappings
                .get(&rust_class.rust_name)
                .cloned()
        });
    match &claimed_bevy_name {
        Some(name) => resolve_claimed_bevy_item(
            rust_class,
            bevy_crates,
            name,
            &config.bevy.claim_overrides,
            internal_type_names,
        ),
        None => (
            find_bevy_item(bevy_crates, &pybevy_bevy_name(rust_class, &config.bevy)),
            Vec::new(),
        ),
    }
}

/// Per-variant constructor-policy checks for generated nested classes.
fn variant_policy(
    class: &PyClassDef,
    variant: &EnumVariantDef,
    bevy_crates: &HashMap<String, BevyCrate>,
    bevy_config: &crate::config::BevyConfig,
    bevy_path: Option<&std::path::Path>,
) -> Vec<Diagnostic> {
    let Some(constructor) = variant.constructor.as_ref() else {
        return Vec::new();
    };

    let parent_rust_name = class.extends.as_deref().or(Some(class.rust_name.as_str()));
    let Some(parent_rust_name) = parent_rust_name else {
        return Vec::new();
    };

    let parent_class = PyClassDef {
        rust_name: parent_rust_name.to_string(),
        ..Default::default()
    };
    let bevy_name = crate::comparison::pybevy_bevy_name(&parent_class, bevy_config);
    let Some((crate_name, parent_item)) = find_bevy_item(bevy_crates, &bevy_name) else {
        return Vec::new();
    };

    let Some(bevy_variant) = parent_item.variants.iter().find(|v| v.name == variant.name) else {
        return Vec::new();
    };

    let origin = match &bevy_variant.kind {
        crate::bevy_parser::types::BevyVariantKind::Unit => ConstructorOrigin::ZeroArg {
            upstream_type: parent_item.full_path.clone(),
        },
        crate::bevy_parser::types::BevyVariantKind::Tuple(_) => {
            // Named adapters marked #[py_bevy(tuple)] keep the positional
            // payload contract, same as true tuple payloads.
            let _ = &class.bevy_tuple_variants;
            ConstructorOrigin::TuplePayload {
                upstream_type: parent_item.full_path.clone(),
            }
        }
        crate::bevy_parser::types::BevyVariantKind::Struct(fields) => {
            // cargo public-api emits variant fields alphabetically; the true
            // declaration order lives in the pinned source's variant block.
            let source_order = origin::source_variant_field_order(
                &parent_item.full_path,
                crate_name,
                bevy_path,
                &variant.name,
            );
            ConstructorOrigin::EnumVariant {
                upstream_type: parent_item.full_path.clone(),
                variant: variant.name.clone(),
                field_order: source_order
                    .unwrap_or_else(|| fields.iter().map(|(name, _)| name.clone()).collect()),
            }
        }
    };

    let variant_class = PyClassDef {
        python_name: variant.name.clone(),
        extends: Some(class.python_name.clone()),
        constructor: Some(constructor.clone()),
        bevy_tuple_variants: class.bevy_tuple_variants.clone(),
        ..Default::default()
    };
    origin::check_constructor_policy(&origin, &variant_class)
}

/// Upstream field and variant payload types behind one wrapper's stub slots.
fn upstream_slots(
    class: &PyClassDef,
    origin: Option<&ConstructorOrigin>,
    bevy_crates: &HashMap<String, BevyCrate>,
    bevy_config: &crate::config::BevyConfig,
    bevy_path: Option<&std::path::Path>,
) -> wrapper_types::UpstreamSlots {
    let item = find_bevy_item(
        bevy_crates,
        &crate::comparison::pybevy_bevy_name(class, bevy_config),
    )
    .map(|(_, item)| item);
    let upstream_type = origin
        .and_then(ConstructorOrigin::upstream_type)
        .or(item.map(|item| item.full_path.as_str()));
    let fields = upstream_type
        .and_then(|upstream_type| origin::source_field_types(upstream_type, bevy_path))
        .unwrap_or_default();
    let variants = item
        .map(|item| {
            item.variants
                .iter()
                .map(|variant| (variant.name.clone(), variant.kind.clone()))
                .collect()
        })
        .unwrap_or_default();
    wrapper_types::UpstreamSlots { fields, variants }
}

/// Collect upstream Bevy items carrying the given short name across all
/// parsed crates; a qualified `crate::Name` claim is reduced to its short
/// name so legacy resolution semantics are preserved.
fn collect_bevy_matches<'a>(
    bevy_crates: &'a HashMap<String, BevyCrate>,
    bevy_name: &str,
) -> Vec<(&'a str, &'a BevyItem)> {
    let short_name = claim_short_name(bevy_name);
    let mut matches = Vec::new();
    for (crate_name, bevy_crate) in bevy_crates {
        if let Some(item) = bevy_crate.items.get(short_name) {
            matches.push((crate_name.as_str(), item));
        }
    }
    matches
}

/// Find one item by short name across the parsed crates.
fn find_bevy_item<'a>(
    bevy_crates: &'a HashMap<String, BevyCrate>,
    bevy_name: &str,
) -> Option<(&'a str, &'a BevyItem)> {
    let matches = collect_bevy_matches(bevy_crates, bevy_name);
    (matches.len() == 1).then(|| matches[0])
}

/// Reduce a claim to its inventory lookup key: the last `::` segment with
/// any generic arguments stripped (`bevy::asset::AssetEvent<Image>` ->
/// `AssetEvent`).
fn claim_short_name(bevy_name: &str) -> &str {
    bevy_name
        .rsplit("::")
        .next()
        .unwrap_or(bevy_name)
        .split('<')
        .next()
        .unwrap_or(bevy_name)
        .trim()
}

/// Crate candidates implied by a qualified claim path: the
/// `bevy::gltf::X` segments yield `bevy` then `bevy_gltf`.
fn path_crate_candidates(bevy_name: &str) -> Vec<String> {
    let segments: Vec<&str> = bevy_name.split("::").collect();
    (1..segments.len())
        .map(|k| segments[..k].join("_"))
        .collect()
}

fn crate_key_matches(key: &str, candidate: &str) -> bool {
    key.replace('-', "_")
        .eq_ignore_ascii_case(&candidate.replace('-', "_"))
}

/// Resolve the upstream item for a wrapper that explicitly claims a Bevy
/// type. Configured claim overrides rewrite an ambiguous raw claim to its
/// crate-qualified form before matching. Short-name ambiguity is decided by
/// the crate implied by the claim path, then by module affinity (a wrapper in
/// `crates/pybevy_X` prefers `bevy_X`); whatever remains unresolved reports
/// W025 so broken or ambiguous claims cannot stay silent. Claims for types
/// declared by PyBevy itself are internal wrappers, not upstream claims.
fn resolve_claimed_bevy_item<'a>(
    rust_class: &PyClassDef,
    bevy_crates: &'a HashMap<String, BevyCrate>,
    bevy_name: &str,
    claim_overrides: &HashMap<String, String>,
    internal_type_names: &HashSet<String>,
) -> (Option<(&'a str, &'a BevyItem)>, Vec<Diagnostic>) {
    let effective_name = claim_overrides
        .get(bevy_name)
        .map(String::as_str)
        .unwrap_or(bevy_name);
    let short_name = claim_short_name(effective_name);
    if internal_type_names.contains(short_name) {
        return (None, Vec::new());
    }

    let matches = collect_bevy_matches(bevy_crates, effective_name);
    if matches.len() == 1 {
        return (Some(matches[0]), Vec::new());
    }

    let with_location = |diagnostic: Diagnostic, rust_class: &PyClassDef| -> Diagnostic {
        match &rust_class.location {
            Some(location) => diagnostic.with_location(location.clone()),
            None => diagnostic,
        }
    };

    if matches.len() > 1 {
        for candidate in path_crate_candidates(effective_name) {
            let winners: Vec<(&str, &BevyItem)> = matches
                .iter()
                .filter(|(crate_name, _)| crate_key_matches(crate_name, &candidate))
                .map(|(crate_name, item)| (*crate_name, *item))
                .collect();
            if winners.len() == 1 {
                return (Some(winners[0]), Vec::new());
            }
        }

        if let Some(module) = rust_class.module_path.as_deref() {
            let short = module
                .split('.')
                .find_map(|segment| segment.strip_prefix("pybevy_"))
                .unwrap_or(module);
            let preferred = format!("bevy_{short}");
            let winners: Vec<(&str, &BevyItem)> = matches
                .iter()
                .filter(|(crate_name, _)| crate_key_matches(crate_name, &preferred))
                .map(|(crate_name, item)| (*crate_name, *item))
                .collect();
            if winners.len() == 1 {
                return (Some(winners[0]), Vec::new());
            }
        }

        let candidates: Vec<&str> = matches.iter().map(|(crate_name, _)| *crate_name).collect();
        let diagnostic = Diagnostic::warning(
            DiagnosticCode::W025,
            format!(
                "upstream Bevy item '{}' for wrapper '{}' is ambiguous among {}; add an explicit crate-qualified type mapping or a reviewed exception",
                bevy_name, rust_class.python_name, candidates.join(", ")
            ),
        )
        .with_note(
            "unresolved upstream items skip every Bevy-referencing validation for this wrapper",
        );
        return (None, vec![with_location(diagnostic, rust_class)]);
    }

    let diagnostic = Diagnostic::warning(
        DiagnosticCode::W025,
        format!(
            "claimed upstream Bevy item '{}' for wrapper '{}' not found in the pinned Bevy",
            bevy_name, rust_class.python_name
        ),
    )
    .with_note(
        "check the pinned revision, crate feature gates in the API inventory, or record a reviewed exception",
    );
    (None, vec![with_location(diagnostic, rust_class)])
}

fn validate_all_impl(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: Option<&Config>,
    check_stale_exceptions: bool,
) -> Vec<Diagnostic> {
    let mut consumed_exceptions = config
        .map(|config| vec![false; config.validation.exceptions.len()])
        .unwrap_or_default();
    validate_all_impl_with_consumed(
        rust_classes,
        python_classes,
        config,
        &mut consumed_exceptions,
        check_stale_exceptions,
    )
}

/// Core validation pass over a shared exception-consumption table so the
/// constructor-policy pass and the standard pass agree on exactly-once
/// consumption and one stale-exception check.
fn validate_all_impl_with_consumed(
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
    config: Option<&Config>,
    consumed_exceptions: &mut [bool],
    check_stale_exceptions: bool,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut matched_shared_borrow_types = config
        .map(|config| vec![false; config.validation.shared_borrow_types.len()])
        .unwrap_or_default();
    let borrow_targets = shared_borrow_targets(config, rust_classes);

    // A missing `module = "pybevy...."` leaves `__module__` as `builtins` for
    // private helpers too, so this rule runs ahead of the `_`-prefix skip below.
    for rust_class in rust_classes {
        let path = class_path(rust_class);
        for diagnostic in class_rules::validate_pyclass_module_attribute(rust_class) {
            if is_disabled_code(config, diagnostic.code.as_str()) {
                continue;
            }
            if consume_exception(config, consumed_exceptions, &path, &diagnostic) {
                continue;
            }
            diagnostics.push(diagnostic);
        }
    }

    // W032 is a safety invariant, not an API-surface check: it runs ahead of
    // the `_`-prefix and in_scope skips because any GC-visible pyclass can
    // hold Python references (context managers included).
    for rust_class in rust_classes {
        let path = class_path(rust_class);
        for diagnostic in class_rules::validate_traverse_presence(rust_class) {
            if is_disabled_code(config, diagnostic.code.as_str()) {
                continue;
            }
            if consume_exception(config, consumed_exceptions, &path, &diagnostic) {
                continue;
            }
            diagnostics.push(diagnostic);
        }
    }

    for rust_class in rust_classes {
        if rust_class.python_name.starts_with('_') {
            continue;
        }
        let mut class_diagnostics = Vec::new();
        // Find corresponding Python class - prefer module path match
        let python_class = find_matching_python_class(rust_class, python_classes);
        let manual_variant = find_manual_enum_variant(rust_class, rust_classes, python_classes);

        // Rust-only validations (don't require Python stub)
        class_diagnostics.extend(class_rules::validate_enum_classattr_redundancy(rust_class));
        class_diagnostics.extend(class_rules::validate_value_enum_without_pyenum(rust_class));
        class_diagnostics.extend(method_rules::validate_builder_receivers(rust_class));
        class_diagnostics.extend(class_rules::validate_conversion_fallbacks(rust_class));
        let self_borrow_reason =
            shared_borrow_reason(config, &mut matched_shared_borrow_types, rust_class);
        if let Some(reason) = self_borrow_reason {
            class_diagnostics.extend(method_rules::validate_shared_borrow_receivers(
                rust_class, reason,
            ));
        }
        class_diagnostics.extend(method_rules::validate_shared_borrow_parameters(
            rust_class,
            &class_borrow_targets(&borrow_targets, self_borrow_reason),
        ));

        match python_class.or(manual_variant.as_ref()) {
            Some(py_class) => {
                // Validate class match
                class_diagnostics.extend(class_rules::validate_class_match(rust_class, py_class));
                class_diagnostics
                    .extend(class_rules::validate_pyenum_stub_base(rust_class, py_class));

                // Validate constructor
                if !is_manual_enum_base(rust_class, py_class) {
                    class_diagnostics
                        .extend(method_rules::validate_constructor(rust_class, py_class));
                    class_diagnostics.extend(method_rules::validate_constructor_default_parity(
                        rust_class,
                        py_class,
                        config.map(|config| &config.bevy),
                    ));
                    class_diagnostics.extend(method_rules::validate_constructor_none_parity(
                        rust_class, py_class,
                    ));
                }

                // Validate methods
                class_diagnostics.extend(specialized_receivers::validate_methods(
                    rust_class,
                    py_class,
                    rust_classes,
                    config,
                ));

                class_diagnostics.extend(protocol_rules::validate_protocols(
                    rust_class,
                    py_class,
                    rust_classes,
                    python_classes,
                ));

                // Validate properties
                class_diagnostics.extend(property_rules::validate_properties(
                    rust_class,
                    py_class,
                    config.map(|config| &config.bevy),
                ));

                // Validate bridge batch stub parity
                class_diagnostics.extend(bridge_rules::validate_bridge_batch(rust_class, py_class));

                // Detect eligible but unexposed fields
                class_diagnostics.extend(bridge_rules::detect_unexposed_eligible_fields(
                    rust_class, py_class,
                ));

                // Style checks
                class_diagnostics.extend(style_rules::check_class_style(rust_class, rust_classes));
            }
            None => {
                // Class not found in Python stubs
                class_diagnostics.push(class_rules::missing_class_in_stub(rust_class));
            }
        }

        let path = class_path(rust_class);
        for diagnostic in class_diagnostics {
            if is_disabled_code(config, diagnostic.code.as_str()) {
                continue;
            }
            if consume_exception(config, consumed_exceptions, &path, &diagnostic) {
                continue;
            }
            diagnostics.push(diagnostic);
        }
    }

    for py_class in python_classes {
        if !rust_classes
            .iter()
            .any(|r| r.python_name == py_class.python_name)
        {
            // Python-only classes are not errors; skip
        }
    }

    if let Some(config) = config.filter(|_| check_stale_exceptions) {
        diagnostics.extend(specialized_receivers::audit_mappings(rust_classes, config));
        for (exception, consumed) in config
            .validation
            .exceptions
            .iter()
            .zip(consumed_exceptions.iter())
        {
            if !*consumed {
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

        for (declared, matched) in config
            .validation
            .shared_borrow_types
            .iter()
            .zip(matched_shared_borrow_types)
        {
            if !matched {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::E010,
                        format!(
                            "shared-borrow type '{}' matches no Rust class",
                            declared.path
                        ),
                    )
                    .with_note(declared.reason.clone()),
                );
            }
        }
    }

    diagnostics
}

/// Rust type names of the declared shared-borrow proxies, so a
/// `PyRefMut<'_, T>` parameter naming one is found on whatever class declares
/// it. A declaration matching no parsed class is reported as stale instead.
fn shared_borrow_targets<'a>(
    config: Option<&'a Config>,
    rust_classes: &[PyClassDef],
) -> Vec<(String, &'a str)> {
    let Some(config) = config else {
        return Vec::new();
    };
    config
        .validation
        .shared_borrow_types
        .iter()
        .filter_map(|declared| {
            let class = rust_classes
                .iter()
                .find(|class| class_path(class) == declared.path)?;
            Some((class.rust_name.clone(), declared.reason.as_str()))
        })
        .collect()
}

/// The targets visible from one class: every declared type by name, plus
/// `Self` when the class is itself declared.
fn class_borrow_targets<'a>(
    targets: &'a [(String, &'a str)],
    self_reason: Option<&'a str>,
) -> Vec<SharedBorrowTarget<'a>> {
    let mut visible: Vec<SharedBorrowTarget<'a>> = targets
        .iter()
        .map(|(type_name, reason)| SharedBorrowTarget {
            type_name: type_name.as_str(),
            reason,
        })
        .collect();

    if let Some(reason) = self_reason {
        visible.push(SharedBorrowTarget {
            type_name: "Self",
            reason,
        });
    }

    visible
}

/// The reason a class is declared a shared-borrow proxy, marking the
/// declaration as matched so a stale path is reported.
fn shared_borrow_reason<'a>(
    config: Option<&'a Config>,
    matched: &mut [bool],
    class: &PyClassDef,
) -> Option<&'a str> {
    let path = class_path(class);
    config?
        .validation
        .shared_borrow_types
        .iter()
        .enumerate()
        .find(|(_, declared)| declared.path == path)
        .map(|(index, declared)| {
            matched[index] = true;
            declared.reason.as_str()
        })
}

fn class_path(class: &PyClassDef) -> String {
    match &class.module_path {
        Some(module) => format!("{module}.{}", class.python_name),
        None => class.python_name.clone(),
    }
}

fn is_disabled_code(config: Option<&Config>, code: &str) -> bool {
    config.is_some_and(|config| {
        config
            .validation
            .disabled
            .iter()
            .any(|disabled| disabled.code == code && !disabled.reason.trim().is_empty())
    })
}

fn consume_exception(
    config: Option<&Config>,
    consumed: &mut [bool],
    path: &str,
    diagnostic: &Diagnostic,
) -> bool {
    let Some(config) = config else {
        return false;
    };
    let Some((index, _)) =
        config
            .validation
            .exceptions
            .iter()
            .enumerate()
            .find(|(index, exception)| {
                !consumed[*index]
                    && !exception.reason.trim().is_empty()
                    && exception.path == path
                    && exception.code == diagnostic.code.as_str()
                    && exception
                        .message
                        .as_ref()
                        .is_none_or(|message| message == &diagnostic.message)
            })
    else {
        return false;
    };
    consumed[index] = true;
    true
}

/// Find the matching Python class for a Rust class
/// Prefers matching by both name AND module path when possible
fn find_matching_python_class<'a>(
    rust_class: &PyClassDef,
    python_classes: &'a [PyClassDef],
) -> Option<&'a PyClassDef> {
    let public_module = rust_class
        .python_module_path
        .as_deref()
        .and_then(|module| module.strip_prefix("pybevy."))
        .or(rust_class.module_path.as_deref());
    if let Some(rust_module) = public_module {
        // Try exact module match first
        if let Some(py_class) = python_classes.iter().find(|p| {
            p.python_name == rust_class.python_name
                && p.module_path.as_ref().is_some_and(|pm| pm == rust_module)
        }) {
            return Some(py_class);
        }

        // Try matching just the first component of the module path
        // e.g., rust "camera.scaling_mode" should match python "camera"
        let rust_module_base = rust_module.split('.').next().unwrap_or(rust_module);
        if let Some(py_class) = python_classes.iter().find(|p| {
            p.python_name == rust_class.python_name
                && p.module_path
                    .as_ref()
                    .is_some_and(|pm| pm == rust_module_base || pm.starts_with(rust_module_base))
        }) {
            return Some(py_class);
        }
    }

    // Fallback: match by name only
    python_classes
        .iter()
        .find(|p| p.python_name == rust_class.python_name)
}

fn is_manual_enum_base(rust_class: &PyClassDef, python_class: &PyClassDef) -> bool {
    python_class.is_enum
        && matches!(
            rust_class.macro_info,
            Some(MacroInfo::BevyEnum { manual: true, .. })
        )
}

fn find_manual_enum_variant(
    rust_class: &PyClassDef,
    rust_classes: &[PyClassDef],
    python_classes: &[PyClassDef],
) -> Option<PyClassDef> {
    let rust_parent_name = rust_class.extends.as_deref()?;
    let rust_parent = rust_classes.iter().find(|candidate| {
        candidate.rust_name == rust_parent_name
            && matches!(
                candidate.macro_info,
                Some(MacroInfo::BevyEnum { manual: true, .. })
            )
    })?;
    let python_parent = find_matching_python_class(rust_parent, python_classes)?;
    let python_variant = python_parent
        .enum_variants
        .iter()
        .find(|variant| variant.name == rust_class.python_name)?;

    Some(enum_variant_as_class(python_parent, python_variant))
}

fn enum_variant_as_class(parent: &PyClassDef, variant: &EnumVariantDef) -> PyClassDef {
    let fields: Vec<(String, String)> = match &variant.kind {
        EnumVariantKind::Unit | EnumVariantKind::EmptyTuple => Vec::new(),
        EnumVariantKind::Tuple(types) => types
            .iter()
            .enumerate()
            .map(|(index, field_type)| {
                let name = if types.len() == 1 {
                    "value".to_string()
                } else {
                    format!("_{index}")
                };
                (name, field_type.clone())
            })
            .collect(),
        EnumVariantKind::Struct(fields) => fields.clone(),
    };

    let parameters = fields
        .iter()
        .map(|(name, field_type)| ParameterDef {
            name: name.clone(),
            param_type: Some(field_type.clone()),
            ..Default::default()
        })
        .collect();
    let properties = fields
        .into_iter()
        .map(|(name, property_type)| PropertyDef {
            has_setter: variant.writable_fields.contains(&name),
            name,
            property_type: Some(property_type),
            has_getter: true,
            ..Default::default()
        })
        .collect();

    let constructor = variant.constructor.clone().unwrap_or_else(|| MethodDef {
        name: "__init__".to_string(),
        parameters,
        ..Default::default()
    });

    PyClassDef {
        python_name: variant.name.clone(),
        extends: Some(parent.python_name.clone()),
        module_path: parent.module_path.clone(),
        constructor: Some(constructor),
        properties,
        ..Default::default()
    }
}
