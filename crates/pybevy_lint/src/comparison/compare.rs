use std::collections::{HashMap, HashSet};

use super::report::{
    ConstructorWarning, CoverageReport, CrateCoverage, ExtendsWarning, ExtendsWarningKind,
    ExtraMethodCoverage, FieldCoverage, MethodCoverage, SignatureDiff, TypeCoverage,
    VariantCoverage,
};
use crate::{
    bevy_parser::{BevyCrate, BevyItem, BevyItemKind, BevyMethod, SelfKind},
    config::{BevyConfig, SignatureDiffSpec},
    model::{MethodDef, PropertyDef, PyClassDef, SelfMutability},
};

/// Result of comparing PyBevy API against Bevy API
#[derive(Debug)]
pub struct ComparisonResult {
    pub report: CoverageReport,
}

/// Extract the PyBevy module name from a full module path.
/// e.g., "crates.pybevy_render.src" -> "render"
/// e.g., "crates.pybevy_pbr.src" -> "pbr"
/// e.g., "src.app" -> "app"
fn extract_pybevy_module(module_path: &str) -> Option<String> {
    for segment in module_path.split('.') {
        if let Some(stripped) = segment.strip_prefix("pybevy_") {
            return Some(stripped.to_string());
        }
    }
    if module_path.starts_with("src.") {
        return module_path.strip_prefix("src.").map(|s| s.to_string());
    }
    None
}

/// Compare PyBevy classes against Bevy's public API
/// Bevy type name a PyBevy class maps to: the `#[pyenum(BevyType)]` target if
/// present, otherwise the configured type mapping (or Py-prefix strip).
pub fn pybevy_bevy_name(class: &PyClassDef, config: &BevyConfig) -> String {
    match &class.macro_info {
        Some(crate::model::MacroInfo::BevyEnum { bevy_type, .. }) => bevy_type
            .rsplit("::")
            .next()
            .unwrap_or(bevy_type)
            .split('<')
            .next()
            .unwrap_or(bevy_type)
            .trim()
            .to_string(),
        _ => config.map_type_name(&class.rust_name),
    }
}

pub fn compare_apis(
    pybevy_classes: &[PyClassDef],
    bevy_crates: &HashMap<String, BevyCrate>,
    config: &BevyConfig,
) -> ComparisonResult {
    let mut report = CoverageReport::new();

    // Use Vec to support multiple classes with the same name (from different modules)
    let mut pybevy_by_bevy_name: HashMap<String, Vec<&PyClassDef>> = HashMap::new();
    for c in pybevy_classes {
        let bevy_name = pybevy_bevy_name(c, config);
        pybevy_by_bevy_name.entry(bevy_name).or_default().push(c);
    }

    let mut pybevy_by_python_name: HashMap<&str, Vec<&PyClassDef>> = HashMap::new();
    for c in pybevy_classes {
        pybevy_by_python_name
            .entry(c.python_name.as_str())
            .or_default()
            .push(c);
    }

    for (crate_name, bevy_crate) in bevy_crates {
        if config.is_crate_excluded(crate_name) {
            continue;
        }

        let crate_coverage = compare_crate(
            crate_name,
            bevy_crate,
            &pybevy_by_bevy_name,
            &pybevy_by_python_name,
            config,
        );

        report.total_bevy_types += crate_coverage.bevy_type_count;
        report.total_pybevy_types += crate_coverage.pybevy_type_count;
        report.matched_types += crate_coverage.matched_count;
        report.missing_types += crate_coverage.missing_count;

        for type_cov in &crate_coverage.types {
            report.total_bevy_methods += type_cov.bevy_method_count;
            report.total_pybevy_methods += type_cov.pybevy_method_count;
            report.matched_methods += type_cov.matched_method_count;
            report.missing_methods += type_cov.bevy_method_count - type_cov.matched_method_count;
            report.signature_mismatches += type_cov
                .methods
                .iter()
                .filter(|method| method.is_implemented && !method.signature_matches)
                .count();

            // Track methods only for implemented types (more meaningful metric)
            if type_cov.is_implemented {
                report.implemented_type_bevy_methods += type_cov.bevy_method_count;
                report.implemented_type_matched_methods += type_cov.matched_method_count;
                report.extra_methods += type_cov.extra_method_count;

                // Track field coverage (only for implemented types)
                report.total_bevy_fields += type_cov.bevy_field_count;
                report.matched_fields += type_cov.matched_field_count;
                report.missing_fields += type_cov.bevy_field_count - type_cov.matched_field_count;

                // Track variant coverage (only for implemented enum types)
                report.total_bevy_variants += type_cov.bevy_variant_count;
                report.matched_variants += type_cov.matched_variant_count;
                report.missing_variants += type_cov.missing_variants().len();
                report.mismatched_variants += type_cov.mismatched_variants().len();
                report.extra_variants += type_cov.extra_variant_count;
                if type_cov.enum_representation_matches == Some(false) {
                    report.enum_representation_mismatches += 1;
                }
            }
        }

        report.crates.insert(crate_name.clone(), crate_coverage);
    }

    ComparisonResult { report }
}

/// Find the best matching PyBevy class for a type name
/// Prefers classes from the same module (e.g., camera module for bevy_camera types)
fn find_pybevy_class<'a>(
    type_name: &str,
    pybevy_module: Option<&str>,
    pybevy_by_bevy_name: &'a HashMap<String, Vec<&'a PyClassDef>>,
    pybevy_by_python_name: &'a HashMap<&str, Vec<&'a PyClassDef>>,
) -> Option<&'a PyClassDef> {
    if let Some(candidates) = pybevy_by_bevy_name.get(type_name)
        && let Some(best) = find_best_module_match(candidates, pybevy_module)
    {
        return Some(best);
    }

    // Try stripping generic parameters (e.g., "AudioPlayer<AudioSource>" -> "AudioPlayer")
    if let Some(base_name) = type_name.split('<').next()
        && base_name != type_name
        && let Some(candidates) = pybevy_by_bevy_name.get(base_name)
        && let Some(best) = find_best_module_match(candidates, pybevy_module)
    {
        return Some(best);
    }

    if let Some(candidates) = pybevy_by_python_name.get(type_name)
        && let Some(best) = find_best_module_match(candidates, pybevy_module)
    {
        return Some(best);
    }

    if let Some(base_name) = type_name.split('<').next()
        && base_name != type_name
        && let Some(candidates) = pybevy_by_python_name.get(base_name)
        && let Some(best) = find_best_module_match(candidates, pybevy_module)
    {
        return Some(best);
    }

    None
}

/// From a list of candidate classes, find the one that best matches the target module
fn find_best_module_match<'a>(
    candidates: &[&'a PyClassDef],
    target_module: Option<&str>,
) -> Option<&'a PyClassDef> {
    if candidates.is_empty() {
        return None;
    }

    if candidates.len() == 1 {
        return Some(candidates[0]);
    }

    if let Some(target) = target_module {
        for c in candidates {
            if let Some(ref module_path) = c.module_path
                && module_path == target
            {
                return Some(*c);
            }
        }
    }

    Some(candidates[0])
}

fn compare_crate(
    crate_name: &str,
    bevy_crate: &BevyCrate,
    pybevy_by_bevy_name: &HashMap<String, Vec<&PyClassDef>>,
    pybevy_by_python_name: &HashMap<&str, Vec<&PyClassDef>>,
    config: &BevyConfig,
) -> CrateCoverage {
    let pybevy_module: Option<String> = config
        .crate_mappings
        .iter()
        .filter(|(_, value)| *value == crate_name)
        .map(|(module, _)| module.clone())
        .min();

    let mut coverage = CrateCoverage {
        crate_name: crate_name.to_string(),
        pybevy_module: pybevy_module.clone(),
        ..Default::default()
    };

    for (type_name, bevy_item) in &bevy_crate.items {
        if !matches!(bevy_item.kind, BevyItemKind::Struct | BevyItemKind::Enum) {
            continue;
        }

        // Skip generic base types (e.g., Extrusion<P>) - these are for method inheritance only
        if bevy_item.is_generic_base {
            continue;
        }

        if config.is_type_excluded_in_crate(type_name, Some(crate_name)) {
            continue;
        }

        coverage.bevy_type_count += 1;

        let pybevy_class = find_pybevy_class(
            type_name,
            pybevy_module.as_deref(),
            pybevy_by_bevy_name,
            pybevy_by_python_name,
        );

        let pybevy_class =
            pybevy_class.filter(|&_pyclass| !config.is_false_match(type_name, crate_name));

        let type_coverage = if let Some(pyclass) = pybevy_class {
            coverage.matched_count += 1;
            coverage.pybevy_type_count += 1;
            let mut tc = compare_type(bevy_item, pyclass, config, pybevy_by_bevy_name, bevy_crate);

            // Check module placement: does the PyBevy module match the expected Bevy crate?
            if let Some(ref expected_module) = pybevy_module
                && let Some(ref actual_path) = pyclass.module_path
            {
                // Extract the pybevy module name from the full path
                // e.g., "crates.pybevy_render.src" -> "render"
                let actual_module = extract_pybevy_module(actual_path);
                if let Some(ref actual) = actual_module
                    && actual != expected_module
                    && !config.is_module_placement_override(type_name, crate_name, actual)
                {
                    tc.module_mismatch = Some(super::report::ModuleMismatch {
                        expected_module: expected_module.clone(),
                        actual_module: actual.clone(),
                        bevy_crate: crate_name.to_string(),
                    });
                }
            }

            tc
        } else {
            coverage.missing_count += 1;
            TypeCoverage {
                bevy_name: type_name.clone(),
                bevy_path: bevy_item.full_path.clone(),
                pybevy_name: None,
                is_implemented: false,
                methods: Vec::new(),
                extra_methods: Vec::new(),
                fields: Vec::new(),
                variants: Vec::new(),
                enum_representation_matches: None,
                constructor_warnings: Vec::new(),
                extends_warnings: Vec::new(),
                bevy_method_count: bevy_item.methods.len(),
                pybevy_method_count: 0,
                matched_method_count: 0,
                extra_method_count: 0,
                bevy_field_count: bevy_item.fields.len(),
                matched_field_count: 0,
                constructor_settable_field_count: 0,
                bevy_variant_count: bevy_item.variants.len(),
                pybevy_variant_count: 0,
                matched_variant_count: 0,
                extra_variant_count: 0,
                module_mismatch: None,
            }
        };

        coverage.types.push(type_coverage);
    }

    coverage
        .types
        .sort_by(|a, b| match (a.is_implemented, b.is_implemented) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.bevy_name.cmp(&b.bevy_name),
        });

    coverage
}

/// Check if a trait method should be skipped because PyBevy has an equivalent
fn should_skip_trait_method(method_name: &str, pyclass: &PyClassDef, config: &BevyConfig) -> bool {
    match method_name {
        // Default::default() - skip if:
        // 1. It's an engine-managed type (not user-constructable), OR
        // 2. PyBevy has a constructor with no required params, OR
        // 3. It's an enum (Python enums don't have Default)
        "default" => {
            if config
                .excluded_types
                .engine_managed_types
                .iter()
                .any(|t| t == &pyclass.python_name)
            {
                return true;
            }
            if pyclass.is_enum {
                return true;
            }
            if let Some(ref constructor) = pyclass.constructor {
                constructor
                    .parameters
                    .iter()
                    .all(|p| p.default_value.is_some() || p.is_optional)
            } else {
                false
            }
        }

        // new() - skip if PyBevy has a constructor
        // Bevy's `fn new(...)` becomes Python's `__init__(...)`
        "new" => pyclass.constructor.is_some(),

        // PartialEq::eq - skip if PyBevy class has #[pyclass(eq)], __eq__ method, or it's an enum
        // Python enums are automatically comparable
        "eq" => pyclass.eq || pyclass.is_enum || pyclass.methods.iter().any(|m| m.name == "__eq__"),

        // From/Into trait impls - skip these, Python uses different conversion patterns
        "from" | "into" => true,

        "try_from" | "try_into" => true,

        _ => false,
    }
}

fn compare_type(
    bevy_item: &BevyItem,
    pyclass: &PyClassDef,
    config: &BevyConfig,
    pybevy_types: &HashMap<String, Vec<&PyClassDef>>,
    bevy_crate: &BevyCrate,
) -> TypeCoverage {
    let pybevy_methods: HashMap<&str, &MethodDef> = pyclass
        .methods
        .iter()
        .map(|m| (m.name.as_str(), m))
        .chain(pyclass.static_methods.iter().map(|m| (m.name.as_str(), m)))
        .collect();

    let pybevy_properties: HashMap<&str, &PropertyDef> = pyclass
        .properties
        .iter()
        .map(|p| (p.name.as_str(), p))
        .collect();

    let mut method_coverages = Vec::new();
    let mut matched_method_count = 0;
    let mut seen_methods: HashSet<&str> = HashSet::new();

    // Methods: inherent + generic base (get_methods_for_item), then trait defaults below
    let all_bevy_methods = bevy_crate.get_methods_for_item(bevy_item);
    let mut all_methods: Vec<(&BevyMethod, Option<&str>)> = all_bevy_methods
        .into_iter()
        .map(|m| (m, m.from_trait.as_deref()))
        .collect();

    // Add methods from implemented traits (for default trait methods)
    for trait_impl in &bevy_item.trait_impls {
        // e.g., "bevy_audio::AudioSinkPlayback" -> "AudioSinkPlayback"
        let trait_short_name = trait_impl.rsplit("::").next().unwrap_or(trait_impl);

        if let Some(trait_item) = bevy_crate.items.get(trait_short_name)
            && trait_item.kind == BevyItemKind::Trait
        {
            for trait_method in &trait_item.methods {
                if !all_methods.iter().any(|(m, _)| m.name == trait_method.name) {
                    all_methods.push((trait_method, Some(trait_impl.as_str())));
                }
            }
        }
    }

    for (bevy_method, from_trait) in &all_methods {
        // Skip duplicate methods (from multiple trait impls)
        if seen_methods.contains(bevy_method.name.as_str()) {
            continue;
        }
        seen_methods.insert(&bevy_method.name);
        if config.is_method_excluded(&bevy_method.name) {
            continue;
        }

        // Skip methods from excluded traits (check both the tuple's from_trait and method's from_trait)
        let effective_trait = from_trait.or(bevy_method.from_trait.as_deref());
        if let Some(trait_name) = effective_trait
            && !trait_name.is_empty()
            && config.is_trait_excluded(trait_name)
        {
            continue;
        }

        if bevy_method.name.starts_with('_') {
            continue;
        }

        if should_skip_trait_method(&bevy_method.name, pyclass, config) {
            continue;
        }

        if config.is_intentional_missing_method(&bevy_item.name, &bevy_method.name) {
            continue;
        }

        // Also check renamed methods (e.g., Bevy "with" -> PyBevy "with_")
        let renamed_name = config.get_rename(&bevy_item.name, &bevy_method.name);
        let pybevy_method = pybevy_methods
            .get(bevy_method.name.as_str())
            .or_else(|| renamed_name.and_then(|n| pybevy_methods.get(n)));
        let pybevy_property = pybevy_properties
            .get(bevy_method.name.as_str())
            .or_else(|| renamed_name.and_then(|n| pybevy_properties.get(n)));

        let is_runtime_injected =
            config.is_runtime_injected_method(&bevy_item.name, &bevy_method.name);
        let is_implemented =
            pybevy_method.is_some() || pybevy_property.is_some() || is_runtime_injected;

        let (signature_matches, differences, pybevy_param_count) =
            if let Some(py_method) = pybevy_method {
                let diffs = compare_method_signature(bevy_method, py_method, config);
                if !diffs.is_empty() {
                    if let Some(spec) =
                        config.get_intentional_signature_diff(&bevy_item.name, &bevy_method.name)
                    {
                        let violations = validate_signature_diff_spec(spec, py_method);
                        if violations.is_empty() {
                            // Intentional and valid: suppress
                            (true, Vec::new(), Some(py_method.parameters.len()))
                        } else {
                            // Expected types don't match: regression detected
                            (false, violations, Some(py_method.parameters.len()))
                        }
                    } else {
                        (false, diffs, Some(py_method.parameters.len()))
                    }
                } else {
                    (true, diffs, Some(py_method.parameters.len()))
                }
            } else if pybevy_property.is_some() {
                // Properties match as getters (no params expected)
                if bevy_method.parameters.is_empty() {
                    // Bevy method has no params - property getter is compatible
                    // (even if Bevy takes owned self for builder pattern, PyBevy getter works)
                    (true, Vec::new(), Some(0))
                } else {
                    // Bevy method has params but PyBevy exposes as property - real mismatch
                    let diffs = vec![SignatureDiff::ParamCountMismatch {
                        bevy: bevy_method.parameters.len(),
                        pybevy: 0,
                    }];
                    (false, diffs, Some(0))
                }
            } else if is_runtime_injected {
                // The runtime surface snapshot owns signature validation for
                // methods installed outside the wrapper's canonical crate.
                (true, Vec::new(), None)
            } else {
                (false, Vec::new(), None)
            };

        if is_implemented {
            matched_method_count += 1;
        }

        let missing_required_types = if !is_implemented {
            extract_missing_required_types(bevy_method, pybevy_types)
        } else {
            Vec::new()
        };

        let method_coverage = MethodCoverage {
            bevy_name: bevy_method.name.clone(),
            pybevy_name: None,
            bevy_signature: bevy_method.signature.clone(),
            is_implemented,
            signature_matches,
            differences,
            bevy_param_count: bevy_method.parameters.len(),
            pybevy_param_count,
            missing_required_types,
        };

        method_coverages.push(method_coverage);
    }

    method_coverages.sort_by(|a, b| {
        match (a.is_implemented, b.is_implemented) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => {
                // Both implemented: sort by signature match (mismatches first for visibility)
                match (a.signature_matches, b.signature_matches) {
                    (false, true) => std::cmp::Ordering::Less,
                    (true, false) => std::cmp::Ordering::Greater,
                    _ => a.bevy_name.cmp(&b.bevy_name),
                }
            }
            _ => a.bevy_name.cmp(&b.bevy_name),
        }
    });

    // A Bevy field is "implemented" if PyBevy has either:
    // 1. A property with the same name
    // 2. A getter method with the same name

    let constructor_params: HashSet<&str> = pyclass
        .constructor
        .as_ref()
        .map(|c| c.parameters.iter().map(|p| p.name.as_str()).collect())
        .unwrap_or_default();

    let field_coverages: Vec<FieldCoverage> = bevy_item
        .fields
        .iter()
        .filter(|f| !config.is_intentional_missing_field(&bevy_item.name, &f.name))
        .filter(|f| config.get_rename(&bevy_item.name, &f.name).is_none())
        .map(|f| {
            let is_property = pybevy_properties.contains_key(f.name.as_str());
            let is_getter_method = pybevy_methods.contains_key(f.name.as_str());
            let is_implemented = is_property || is_getter_method;

            let in_constructor = constructor_params.contains(f.name.as_str())
                || config.is_intentional_not_in_constructor(&bevy_item.name, &f.name);

            FieldCoverage {
                bevy_name: f.name.clone(),
                bevy_type: f.field_type.clone(),
                pybevy_name: if is_implemented {
                    Some(f.name.clone())
                } else {
                    None
                },
                is_implemented,
                in_constructor,
            }
        })
        .collect();

    // Methods processed after filtering exclusions, duplicates, trait methods, etc.
    let bevy_method_count = method_coverages.len();
    let bevy_field_count = field_coverages.len();
    let matched_field_count = field_coverages.iter().filter(|f| f.is_implemented).count();
    let constructor_settable_field_count =
        field_coverages.iter().filter(|f| f.in_constructor).count();

    let extra_method_coverages = find_extra_methods(bevy_item, pyclass, config, bevy_crate);
    let extra_method_count = extra_method_coverages.len();

    let (
        variant_coverages,
        bevy_variant_count,
        pybevy_variant_count,
        matched_variant_count,
        extra_variant_count,
    ) = compare_variants(bevy_item, pyclass, config);

    let constructor_warnings = check_constructor_optionality(bevy_item, pyclass, config);

    let extends_warnings = check_extends_consistency(bevy_item, pyclass, config);

    TypeCoverage {
        bevy_name: bevy_item.name.clone(),
        bevy_path: bevy_item.full_path.clone(),
        pybevy_name: Some(pyclass.python_name.clone()),
        is_implemented: true,
        methods: method_coverages,
        extra_methods: extra_method_coverages,
        fields: field_coverages,
        variants: variant_coverages,
        enum_representation_matches: (bevy_item.kind == BevyItemKind::Enum)
            .then_some(pyclass.is_enum),
        constructor_warnings,
        extends_warnings,
        bevy_method_count,
        pybevy_method_count: pybevy_methods.len()
            + config
                .runtime_injected_methods(&bevy_item.name)
                .iter()
                .filter(|method| !pybevy_methods.contains_key(method.name()))
                .count(),
        matched_method_count,
        extra_method_count,
        bevy_field_count,
        matched_field_count,
        constructor_settable_field_count,
        bevy_variant_count,
        pybevy_variant_count,
        matched_variant_count,
        extra_variant_count,
        module_mismatch: None, // Set by compare_crate after this returns
    }
}

/// Find methods in PyBevy that don't exist in Bevy
fn find_extra_methods(
    bevy_item: &BevyItem,
    pyclass: &PyClassDef,
    config: &BevyConfig,
    bevy_crate: &BevyCrate,
) -> Vec<ExtraMethodCoverage> {
    let mut extra_methods = Vec::new();

    let all_bevy_methods = bevy_crate.get_methods_for_item(bevy_item);
    let mut bevy_method_names: HashSet<&str> =
        all_bevy_methods.iter().map(|m| m.name.as_str()).collect();

    for trait_impl in &bevy_item.trait_impls {
        let trait_short_name = trait_impl.rsplit("::").next().unwrap_or(trait_impl);
        if let Some(trait_item) = bevy_crate.items.get(trait_short_name)
            && trait_item.kind == BevyItemKind::Trait
        {
            for trait_method in &trait_item.methods {
                bevy_method_names.insert(&trait_method.name);
            }
        }
    }

    // Also add fields as potential matches (since properties map to fields)
    let bevy_field_names: HashSet<&str> =
        bevy_item.fields.iter().map(|f| f.name.as_str()).collect();

    // Also add constants as potential matches (PyBevy exposes constants as static methods)
    let bevy_constant_names: HashSet<&str> = bevy_item
        .constants
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();

    // Methods that are allowed to be "extra" (Python-specific patterns)
    let allowed_extra = [
        "__repr__",
        "__str__",
        "__eq__",
        "__ne__",
        "__hash__",
        "__richcmp__",
        "__bool__",
        "__len__",
        "__iter__",
        "__next__",
        "__getitem__",
        "__setitem__",
        "__delitem__",
        "__contains__",
        "__add__",
        "__sub__",
        "__mul__",
        "__truediv__",
        "__floordiv__",
        "__mod__",
        "__neg__",
        "__pos__",
        "__abs__",
        "__invert__",
        "__and__",
        "__or__",
        "__xor__",
        "__lshift__",
        "__rshift__",
        "__iadd__",
        "__isub__",
        "__imul__",
        "__itruediv__",
        "__radd__",
        "__rsub__",
        "__rmul__",
        "__rtruediv__",
        "__copy__",
        "__deepcopy__",
        "__float__",
        "__int__",
        "__index__",
        "handle", // Common for handle wrapper types
    ];

    for method in &pyclass.methods {
        let name = method.name.as_str();

        if allowed_extra.contains(&name) {
            continue;
        }

        if config.is_method_excluded(name) {
            continue;
        }

        if config.is_intentional_extra_method(&bevy_item.name, name) {
            continue;
        }

        if config.is_deref_target_method(&bevy_item.name, name) {
            continue;
        }

        if config.is_rename_target(&bevy_item.name, name) {
            continue;
        }

        if let Some(field_name) = name
            .strip_prefix("get_")
            .or_else(|| name.strip_prefix("set_"))
            && bevy_field_names.contains(field_name)
        {
            continue;
        }

        if !bevy_method_names.contains(name) && !bevy_field_names.contains(name) {
            extra_methods.push(ExtraMethodCoverage {
                pybevy_name: name.to_string(),
                is_static: method.is_static,
                is_property: false,
            });
        }
    }

    for method in &pyclass.static_methods {
        let name = method.name.as_str();

        if allowed_extra.contains(&name) {
            continue;
        }

        if config.is_method_excluded(name) {
            continue;
        }

        if config.is_intentional_extra_method(&bevy_item.name, name) {
            continue;
        }

        if config.is_deref_target_method(&bevy_item.name, name) {
            continue;
        }

        // Static methods might map to Bevy constants (e.g., IDENTITY)
        if bevy_constant_names.contains(name) {
            continue;
        }

        if !bevy_method_names.contains(name) {
            extra_methods.push(ExtraMethodCoverage {
                pybevy_name: name.to_string(),
                is_static: true,
                is_property: false,
            });
        }
    }

    for prop in &pyclass.properties {
        let name = prop.name.as_str();

        if bevy_field_names.contains(name) {
            continue;
        }

        if bevy_method_names.contains(name) {
            continue;
        }

        if let Some(field_name) = name
            .strip_prefix("get_")
            .or_else(|| name.strip_prefix("set_"))
            && bevy_field_names.contains(field_name)
        {
            continue;
        }

        if config.is_method_excluded(name) {
            continue;
        }

        if config.is_intentional_extra_method(&bevy_item.name, name) {
            continue;
        }

        if config.is_deref_target_method(&bevy_item.name, name) {
            continue;
        }

        extra_methods.push(ExtraMethodCoverage {
            pybevy_name: name.to_string(),
            is_static: false,
            is_property: true,
        });
    }

    extra_methods.sort_by(|a, b| a.pybevy_name.cmp(&b.pybevy_name));
    extra_methods
}

/// Check if PyBevy constructor parameters match Bevy field optionality
/// Returns warnings for params that are optional in PyBevy but mandatory in Bevy
///
/// Note: If the Bevy type implements Default, we skip these warnings since
/// having defaults that match Default::default() is the recommended pattern.
fn check_constructor_optionality(
    bevy_item: &BevyItem,
    pyclass: &PyClassDef,
    config: &BevyConfig,
) -> Vec<ConstructorWarning> {
    let mut warnings = Vec::new();

    if config.should_ignore_constructor_warnings(&bevy_item.name) {
        return warnings;
    }

    // If Bevy type implements Default, skip constructor warnings
    // Having defaults that match Default::default() is the recommended pattern
    if bevy_item
        .trait_impls
        .iter()
        .any(|t| t == "Default" || t.ends_with("::Default"))
    {
        return warnings;
    }

    let Some(ref constructor) = pyclass.constructor else {
        return warnings;
    };

    if bevy_item.fields.is_empty() {
        return warnings;
    }

    let bevy_fields: HashMap<&str, &str> = bevy_item
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.field_type.as_str()))
        .collect();

    for param in &constructor.parameters {
        if config.should_ignore_constructor_param(&bevy_item.name, &param.name) {
            continue;
        }

        // Is this param optional in PyBevy? (has default value OR type is Option)
        let has_default = param.default_value.is_some();
        let is_option_type = param.is_optional
            || param
                .param_type
                .as_ref()
                .is_some_and(|t| t.contains("Option"));

        if !has_default && !is_option_type {
            continue; // Parameter is required in PyBevy, no issue
        }

        if let Some(bevy_type) = bevy_fields.get(param.name.as_str()) {
            // Is the Bevy field mandatory (not Option)?
            let bevy_is_mandatory =
                !bevy_type.contains("Option<") && !bevy_type.contains("option::Option<");

            if bevy_is_mandatory {
                let simplified_type = simplify_type_for_display(bevy_type);
                warnings.push(ConstructorWarning {
                    param_name: param.name.clone(),
                    bevy_type: simplified_type.clone(),
                    message: format!(
                        "parameter '{}' has default but Bevy field is mandatory ({})",
                        param.name, simplified_type
                    ),
                });
            }
        }
    }

    warnings
}

/// Mapping of Bevy traits to expected PyBevy base classes
const TRAIT_BASE_MAPPINGS: &[(&str, &str)] = &[
    ("Component", "Component"),
    ("Resource", "Resource"),
    ("Asset", "Asset"),
    ("Plugin", "Plugin"),
];

/// Check if PyBevy extends matches Bevy trait implementations
/// Returns warnings for:
/// 1. Bevy implements Component/Resource/Asset/Plugin but PyBevy doesn't extend matching base
/// 2. PyBevy extends Component/Resource/Asset/Plugin but Bevy doesn't implement that trait
/// 3. Bevy implements multiple traits (e.g., Component + Resource) but PyBevy chose one
///    (this is informational since Python only supports single inheritance)
fn check_extends_consistency(
    bevy_item: &BevyItem,
    pyclass: &PyClassDef,
    config: &BevyConfig,
) -> Vec<ExtendsWarning> {
    // Check if warnings are suppressed for this type
    if config.should_ignore_extends_warnings(&bevy_item.name) {
        return Vec::new();
    }

    let mut warnings = Vec::new();

    // Normalize PyBevy extends (handle qualified paths like "pybevy_core :: PyAsset",
    // then strip "Py" prefix if present)
    let extends_owned: Option<String> = pyclass.extends.as_ref().map(|e| {
        // Extract last segment from qualified paths (e.g., "pybevy_core :: PyAsset" -> "PyAsset")
        let last_segment = e.rsplit("::").next().unwrap_or(e).trim();
        // Strip "Py" prefix
        if let Some(stripped) = last_segment.strip_prefix("Py")
            && !stripped.is_empty()
        {
            return stripped.to_string();
        }
        last_segment.to_string()
    });
    let pybevy_extends = extends_owned.as_deref();

    let mut bevy_traits: Vec<&str> = Vec::new();
    let mut matched_trait: Option<&str> = None;

    for (trait_name, base_name) in TRAIT_BASE_MAPPINGS {
        let bevy_has_trait = bevy_item.trait_impls.iter().any(|t| {
            t == *trait_name
                || t.ends_with(&format!("::{}", trait_name))
                || t.contains(&format!("{}::{}", trait_name, trait_name))
        });

        if bevy_has_trait {
            bevy_traits.push(trait_name);

            // Check if PyBevy extends this base
            if pybevy_extends.is_some_and(|e| e == *base_name) {
                matched_trait = Some(trait_name);
            }
        }
    }

    for (trait_name, base_name) in TRAIT_BASE_MAPPINGS {
        let bevy_has_trait = bevy_traits.contains(trait_name);
        let pybevy_has_base = pybevy_extends.is_some_and(|e| e == *base_name);

        if bevy_has_trait && !pybevy_has_base {
            // Check if this is a multi-trait situation where PyBevy chose a different one
            if bevy_traits.len() > 1
                && let Some(matched_trait) = matched_trait
            {
                // Bevy has multiple traits and PyBevy extends one of them - informational only
                warnings.push(ExtendsWarning {
                    kind: ExtendsWarningKind::AlternativeExtends {
                        bevy_trait: trait_name.to_string(),
                        pybevy_extends: pybevy_extends.unwrap_or("").to_string(),
                        matched_trait: matched_trait.to_string(),
                    },
                    message: format!(
                        "Bevy implements {} but PyBevy extends Py{} (Python single inheritance)",
                        trait_name, matched_trait
                    ),
                });
            } else {
                // This is a real missing extends warning
                warnings.push(ExtendsWarning {
                    kind: ExtendsWarningKind::MissingExtends {
                        bevy_trait: trait_name.to_string(),
                        expected_base: format!("Py{}", base_name),
                    },
                    message: format!(
                        "Bevy type implements {} but PyBevy class doesn't extend Py{}",
                        trait_name, base_name
                    ),
                });
            }
        }

        // Skip if Bevy has no detected traits at all (parser limitation for generic types)
        if pybevy_has_base && !bevy_has_trait && !bevy_item.trait_impls.is_empty() {
            warnings.push(ExtendsWarning {
                kind: ExtendsWarningKind::UnexpectedExtends {
                    pybevy_extends: pybevy_extends.unwrap_or("").to_string(),
                    expected_trait: trait_name.to_string(),
                },
                message: format!(
                    "PyBevy class extends Py{} but Bevy type doesn't implement {}",
                    base_name, trait_name
                ),
            });
        }
    }

    let bevy_has_partial_eq = bevy_item
        .trait_impls
        .iter()
        .any(|t| t == "PartialEq" || t.ends_with("::PartialEq") || t.contains("PartialEq<"));

    if bevy_has_partial_eq && !pyclass.eq && !pyclass.methods.iter().any(|m| m.name == "__eq__") {
        warnings.push(ExtendsWarning {
            kind: ExtendsWarningKind::MissingEq,
            message: format!(
                "Bevy type '{}' derives PartialEq but PyBevy #[pyclass] is missing `eq` attribute",
                bevy_item.name
            ),
        });
    }

    warnings
}

/// Compare enum variants between Bevy and PyBevy
/// Returns (variant_coverages, bevy_count, pybevy_count, matched_count, extra_count)
fn compare_variants(
    bevy_item: &BevyItem,
    pyclass: &PyClassDef,
    config: &BevyConfig,
) -> (Vec<VariantCoverage>, usize, usize, usize, usize) {
    use crate::{bevy_parser::BevyVariantKind, model::EnumVariantKind};

    // Structs have no Bevy variant topology to compare. For a Bevy enum backed by
    // an ordinary PyBevy struct, continue below with an empty PyBevy variant set:
    // every Bevy variant is then reported missing instead of silently ignored.
    if bevy_item.kind != BevyItemKind::Enum {
        return (Vec::new(), 0, 0, 0, 0);
    }

    let mut coverages = Vec::new();
    let mut matched_count = 0;

    let pybevy_variants: HashMap<&str, &crate::model::EnumVariantDef> = pyclass
        .enum_variants
        .iter()
        .map(|v| (v.name.as_str(), v))
        .collect();

    let mut skipped_bevy_variants = 0;
    for bevy_variant in &bevy_item.variants {
        if config.is_variant_intentionally_missing(&bevy_item.name, &bevy_variant.name) {
            skipped_bevy_variants += 1;
            continue;
        }
        let bevy_kind_str = match &bevy_variant.kind {
            BevyVariantKind::Unit => "Unit".to_string(),
            BevyVariantKind::Tuple(fields) => format!("Tuple({})", fields.len()),
            BevyVariantKind::Struct(fields) => format!("Struct({})", fields.len()),
        };

        // Check for rename: look up the Bevy name in renames config
        let lookup_name = config
            .get_rename(&bevy_item.name, &bevy_variant.name)
            .unwrap_or(&bevy_variant.name);

        if let Some(py_variant) = pybevy_variants.get(lookup_name) {
            let pybevy_kind_str = match &py_variant.kind {
                EnumVariantKind::Unit => "Unit".to_string(),
                EnumVariantKind::EmptyTuple => "EmptyTuple".to_string(),
                EnumVariantKind::Tuple(fields) => format!("Tuple({})", fields.len()),
                EnumVariantKind::Struct(fields) => format!("Struct({})", fields.len()),
            };

            let kind_matches = enum_variant_kinds_match(
                &bevy_variant.kind,
                &py_variant.kind,
                pyclass.bevy_tuple_variants.contains(&py_variant.name),
            );

            if kind_matches {
                matched_count += 1;
            }
            coverages.push(VariantCoverage {
                bevy_name: bevy_variant.name.clone(),
                bevy_kind: bevy_kind_str,
                is_implemented: true,
                pybevy_kind: Some(pybevy_kind_str),
                kind_matches,
                is_extra: false,
            });
        } else {
            coverages.push(VariantCoverage {
                bevy_name: bevy_variant.name.clone(),
                bevy_kind: bevy_kind_str,
                is_implemented: false,
                pybevy_kind: None,
                kind_matches: false,
                is_extra: false,
            });
        }
    }

    // Also build a set including rename targets so they're not flagged as extra
    let bevy_variant_names: HashSet<&str> =
        bevy_item.variants.iter().map(|v| v.name.as_str()).collect();
    let mut extra_count = 0;

    for py_variant in &pyclass.enum_variants {
        if config.is_rename_target(&bevy_item.name, &py_variant.name)
            || config.is_variant_intentionally_extra(&bevy_item.name, &py_variant.name)
        {
            continue;
        }
        if !bevy_variant_names.contains(py_variant.name.as_str()) {
            let pybevy_kind_str = match &py_variant.kind {
                EnumVariantKind::Unit => "Unit".to_string(),
                EnumVariantKind::EmptyTuple => "EmptyTuple".to_string(),
                EnumVariantKind::Tuple(fields) => format!("Tuple({})", fields.len()),
                EnumVariantKind::Struct(fields) => format!("Struct({})", fields.len()),
            };

            extra_count += 1;
            coverages.push(VariantCoverage {
                bevy_name: py_variant.name.clone(),
                bevy_kind: String::new(), // Not in Bevy
                is_implemented: false,
                pybevy_kind: Some(pybevy_kind_str),
                kind_matches: false,
                is_extra: true,
            });
        }
    }

    let bevy_count = bevy_item.variants.len() - skipped_bevy_variants;
    let pybevy_count = pyclass.enum_variants.len();

    (
        coverages,
        bevy_count,
        pybevy_count,
        matched_count,
        extra_count,
    )
}

fn enum_variant_kinds_match(
    bevy: &crate::bevy_parser::BevyVariantKind,
    pybevy: &crate::model::EnumVariantKind,
    named_bevy_tuple: bool,
) -> bool {
    use crate::{bevy_parser::BevyVariantKind, model::EnumVariantKind};

    match (bevy, pybevy) {
        (BevyVariantKind::Unit, EnumVariantKind::Unit | EnumVariantKind::EmptyTuple) => true,
        (BevyVariantKind::Tuple(bevy_fields), EnumVariantKind::Tuple(pybevy_fields)) => {
            bevy_fields.len() == pybevy_fields.len()
                && bevy_fields
                    .iter()
                    .zip(pybevy_fields)
                    .all(|(bevy, pybevy)| types_compatible(bevy, pybevy))
        }
        (BevyVariantKind::Tuple(bevy_fields), EnumVariantKind::Struct(pybevy_fields))
            if named_bevy_tuple =>
        {
            bevy_fields.len() == pybevy_fields.len()
                && bevy_fields
                    .iter()
                    .zip(pybevy_fields)
                    .all(|(bevy, (_, pybevy))| types_compatible(bevy, pybevy))
        }
        // PyBevy gives Bevy's unnamed one-field tuple payload the public name `value`.
        (BevyVariantKind::Tuple(bevy_fields), EnumVariantKind::Struct(pybevy_fields))
            if bevy_fields.len() == 1 && pybevy_fields.len() == 1 =>
        {
            pybevy_fields[0].0 == "value" && types_compatible(&bevy_fields[0], &pybevy_fields[0].1)
        }
        (BevyVariantKind::Struct(bevy_fields), EnumVariantKind::Struct(pybevy_fields)) => {
            bevy_fields.len() == pybevy_fields.len()
                && bevy_fields.iter().zip(pybevy_fields).all(
                    |((bevy_name, bevy_type), (pybevy_name, pybevy_type))| {
                        bevy_name == pybevy_name && types_compatible(bevy_type, pybevy_type)
                    },
                )
        }
        _ => false,
    }
}

/// Extract types from a method signature that PyBevy doesn't have yet
fn extract_missing_required_types(
    bevy_method: &BevyMethod,
    pybevy_types: &HashMap<String, Vec<&PyClassDef>>,
) -> Vec<String> {
    let mut required_types = HashSet::new();

    for param in &bevy_method.parameters {
        extract_types_from_string(&param.param_type, &mut required_types);
    }

    if let Some(ref return_type) = bevy_method.return_type {
        extract_types_from_string(return_type, &mut required_types);
    }

    let mut missing: Vec<String> = required_types
        .into_iter()
        .filter(|t| {
            // Skip primitive types and common std types
            if is_primitive_or_std_type(t) {
                return false;
            }
            // Check if PyBevy has this type
            !pybevy_types.contains_key(t)
        })
        .collect();

    missing.sort();
    missing
}

/// Extract type names from a type string (handles generics, references, etc.)
fn extract_types_from_string(type_str: &str, types: &mut HashSet<String>) {
    let cleaned = type_str
        .replace("&mut ", "")
        .replace("&'_ ", "")
        .replace("& ", "")
        .replace("mut ", "");

    for part in cleaned.split(&['<', '>', ',', '(', ')', '[', ']', ';', ' '][..]) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Extract the last segment of a path (e.g., "bevy_math::Isometry3d" -> "Isometry3d")
        let type_name = part.split("::").last().unwrap_or(part);

        // Skip numbers, lifetimes, and keywords
        if type_name
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(true)
        {
            continue;
        }
        if type_name.starts_with('\'') {
            continue;
        }

        if !type_name.is_empty() && type_name.chars().next().unwrap().is_uppercase() {
            types.insert(type_name.to_string());
        }
    }
}

/// Check if a type is a primitive or common std type that doesn't need PyBevy implementation
fn is_primitive_or_std_type(type_name: &str) -> bool {
    matches!(
        type_name,
        // Rust primitives
        "bool" | "f32" | "f64" | "i8" | "i16" | "i32" | "i64" | "i128"
            | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "isize"
            | "str" | "String" | "char"
            // Common std types
            | "Option" | "Result" | "Vec" | "Box" | "Arc" | "Rc"
            | "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet"
            | "Duration" | "Instant"
            // Self references
            | "Self"
            // Common glam types that PyBevy has
            | "Vec2" | "Vec3" | "Vec4" | "Quat" | "Mat3" | "Mat4"
            | "IVec2" | "IVec3" | "IVec4" | "UVec2" | "UVec3" | "UVec4"
            | "DVec2" | "DVec3" | "DVec4"
            // Common Bevy types that PyBevy likely has
            | "Entity" | "Commands" | "World" | "Query"
            | "Transform" | "GlobalTransform"
            | "Handle" | "AssetId"
    )
}

/// Compare a Bevy method signature against a PyBevy method
fn parameter_names_match(left: &str, right: &str) -> bool {
    left == right
        || left.strip_prefix('_').unwrap_or(left) == right.strip_prefix('_').unwrap_or(right)
}

fn compare_method_signature(
    bevy: &BevyMethod,
    pybevy: &MethodDef,
    _config: &BevyConfig,
) -> Vec<SignatureDiff> {
    let mut differences = Vec::new();

    if bevy.parameters.len() != pybevy.parameters.len() {
        differences.push(SignatureDiff::ParamCountMismatch {
            bevy: bevy.parameters.len(),
            pybevy: pybevy.parameters.len(),
        });

        for bevy_param in &bevy.parameters {
            if !pybevy
                .parameters
                .iter()
                .any(|param| parameter_names_match(&bevy_param.name, &param.name))
            {
                differences.push(SignatureDiff::MissingParam {
                    name: bevy_param.name.clone(),
                    param_type: bevy_param.param_type.clone(),
                });
            }
        }

        for pybevy_param in &pybevy.parameters {
            if !bevy
                .parameters
                .iter()
                .any(|param| parameter_names_match(&param.name, &pybevy_param.name))
            {
                differences.push(SignatureDiff::ExtraParam {
                    name: pybevy_param.name.clone(),
                });
            }
        }
    } else {
        for (i, (bevy_param, pybevy_param)) in bevy
            .parameters
            .iter()
            .zip(pybevy.parameters.iter())
            .enumerate()
        {
            if !parameter_names_match(&bevy_param.name, &pybevy_param.name) {
                differences.push(SignatureDiff::ParamNameMismatch {
                    index: i,
                    bevy: bevy_param.name.clone(),
                    pybevy: pybevy_param.name.clone(),
                });
            }

            if let Some(ref pybevy_type) = pybevy_param.param_type
                && !types_compatible(&bevy_param.param_type, pybevy_type)
            {
                differences.push(SignatureDiff::ParamTypeMismatch {
                    param_name: bevy_param.name.clone(),
                    bevy_type: bevy_param.param_type.clone(),
                    pybevy_type: pybevy_type.clone(),
                });
            }
        }
    }

    let bevy_self = match bevy.self_kind {
        SelfKind::None => "static",
        SelfKind::Ref => "&self",
        SelfKind::RefMut => "&mut self",
        SelfKind::Owned => "self",
    };

    let pybevy_self = match pybevy.self_mutability {
        SelfMutability::None => "static",
        SelfMutability::Ref => "&self",
        SelfMutability::RefMut => "&mut self",
        SelfMutability::Owned => "self",
    };

    // Only flag if one is static and other isn't, or mutability is significantly different
    let bevy_is_static = bevy.self_kind == SelfKind::None;
    let pybevy_is_static = pybevy.self_mutability == SelfMutability::None || pybevy.is_static;

    if bevy_is_static != pybevy_is_static {
        differences.push(SignatureDiff::SelfMutabilityMismatch {
            bevy: bevy_self.to_string(),
            pybevy: pybevy_self.to_string(),
        });
    }

    if let (Some(bevy_ret), Some(pybevy_ret)) = (&bevy.return_type, &pybevy.return_type)
        && !types_compatible(bevy_ret, pybevy_ret)
    {
        differences.push(SignatureDiff::ReturnTypeMismatch {
            bevy_type: bevy_ret.clone(),
            pybevy_type: pybevy_ret.clone(),
        });
    }

    differences
}

/// Validate that the PyBevy method matches the expected types from a SignatureDiffSpec.
/// Returns empty vec if valid (or spec has no expectations), otherwise returns violation diffs.
fn validate_signature_diff_spec(
    spec: &SignatureDiffSpec,
    py_method: &MethodDef,
) -> Vec<SignatureDiff> {
    let SignatureDiffSpec::WithExpected {
        expected_params,
        expected_return,
        ..
    } = spec
    else {
        // Simple spec: no validation needed
        return Vec::new();
    };

    let mut violations = Vec::new();

    for (param_name, expected_type) in expected_params {
        if let Some(py_param) = py_method.parameters.iter().find(|p| &p.name == param_name) {
            if let Some(ref actual_type) = py_param.param_type
                && !actual_type.contains(expected_type.as_str())
            {
                violations.push(SignatureDiff::ExpectedTypeMismatch {
                    context: format!("param '{}'", param_name),
                    expected: expected_type.clone(),
                    actual: actual_type.clone(),
                });
            }
        } else {
            violations.push(SignatureDiff::ExpectedTypeMismatch {
                context: format!("param '{}'", param_name),
                expected: expected_type.clone(),
                actual: "(param not found)".to_string(),
            });
        }
    }

    if let Some(expected_ret) = expected_return
        && let Some(ref actual_ret) = py_method.return_type
        && !actual_ret.contains(expected_ret.as_str())
    {
        violations.push(SignatureDiff::ExpectedTypeMismatch {
            context: "return".to_string(),
            expected: expected_ret.clone(),
            actual: actual_ret.clone(),
        });
    }

    violations
}

/// Check if a Bevy type is compatible with a PyBevy type
fn types_compatible(bevy_type: &str, pybevy_type: &str) -> bool {
    let bevy_normalized = normalize_bevy_type(bevy_type);
    let pybevy_normalized = normalize_pybevy_type(pybevy_type);

    // Direct match
    if bevy_normalized == pybevy_normalized {
        return true;
    }

    // Any type on either side is compatible (impl Trait, dynamic typing)
    if bevy_normalized == "Any" || pybevy_normalized == "Any" {
        return true;
    }

    // Self type is compatible with the type itself (usually handled elsewhere)
    if bevy_normalized == "Self" || pybevy_normalized == "Self" {
        return true;
    }

    // PyBevy often wraps returns in PyResult, Py<>, PyRefMut, etc.
    // These are implementation details, not API mismatches
    let pybevy_unwrapped = unwrap_pyo3_type(&pybevy_normalized);
    if bevy_normalized == pybevy_unwrapped {
        return true;
    }

    // Check if PyBevy type is a Py-prefixed version
    if pybevy_unwrapped.starts_with("Py") && pybevy_unwrapped.len() > 2 {
        let without_py = &pybevy_unwrapped[2..];
        if bevy_normalized == without_py || bevy_normalized.ends_with(without_py) {
            return true;
        }
    }

    // Check inverse: Bevy type might map to PyFoo
    if !bevy_normalized.starts_with("Py") {
        let with_py = format!("Py{}", bevy_normalized);
        if with_py == pybevy_unwrapped {
            return true;
        }
    }

    // Handle Dir3 -> Vec3 conversion (PyBevy exposes direction as Vec3)
    if bevy_normalized == "Dir3" && (pybevy_unwrapped == "Vec3" || pybevy_unwrapped == "PyVec3") {
        return true;
    }

    // Handle associated type "Vec" which could be Vec2, Vec3, PyVec2, PyVec3
    // This comes from Self::Translation, Self::HalfSize normalization
    if bevy_normalized == "Vec"
        && (pybevy_unwrapped == "Vec2"
            || pybevy_unwrapped == "Vec3"
            || pybevy_unwrapped == "PyVec2"
            || pybevy_unwrapped == "PyVec3")
    {
        return true;
    }

    // Handle "Rotation" which could be Rot2, Quat, etc.
    if bevy_normalized == "Rotation"
        && (pybevy_unwrapped == "Rot2"
            || pybevy_unwrapped == "Quat"
            || pybevy_unwrapped == "PyRot2"
            || pybevy_unwrapped == "PyQuat")
    {
        return true;
    }

    // Handle slice vs Vec: &[T] is compatible with Vec<T> or list[T]
    if bevy_normalized.starts_with("list[") && pybevy_normalized.starts_with("list[") {
        // Both are list types, compare inner
        let bevy_inner = &bevy_normalized[5..bevy_normalized.len() - 1];
        let pybevy_inner = &pybevy_normalized[5..pybevy_normalized.len() - 1];
        if types_compatible(bevy_inner, pybevy_inner) {
            return true;
        }
    }

    // Handle Result<T, E> vs PyResult<T> - both are error-returning patterns
    // The bevy_normalized might be just T if Result was unwrapped, or "Result" if not
    // PyResult<T> unwraps to T, so if bevy returns Result<T, E> and pybevy returns PyResult<T>,
    // we should compare the inner T types

    // Handle Handle<T> vs PyHandle - PyBevy uses a single PyHandle type for all asset handles
    if bevy_normalized.starts_with("Handle") && pybevy_normalized == "Handle" {
        return true;
    }
    if bevy_normalized.starts_with("Handle") && pybevy_normalized == "PyHandle" {
        return true;
    }

    // Handle Vec2 vs tuple (f32, f32) - PyBevy often accepts tuples for convenience
    if bevy_normalized == "Vec2"
        && pybevy_normalized.starts_with('(')
        && pybevy_normalized.ends_with(')')
    {
        let cleaned = pybevy_normalized.replace([' ', '(', ')'], "");
        if cleaned == "f32,f32" || cleaned == "float,float" {
            return true;
        }
    }

    // Handle Vec<u8> / &[u8] vs PyBytes - byte arrays in Rust map to bytes in Python
    // Bevy: Vec<u8>, &[u8] -> normalized to list[int]
    // PyBevy: &Bound<'_, PyBytes>, PyBytes, &PyBytes -> bytes
    if (bevy_normalized == "list[int]"
        || bevy_type.contains("[u8]")
        || bevy_type.contains("Vec<u8>"))
        && (pybevy_type.contains("PyBytes") || pybevy_normalized.contains("bytes"))
    {
        return true;
    }

    false
}

/// Unwrap PyO3 wrapper types like PyResult<T>, Py<T>, PyRefMut<T>
fn unwrap_pyo3_type(ty: &str) -> String {
    let ty = ty.trim();

    // PyResult<T> -> T
    if ty.starts_with("PyResult")
        && let Some(inner) = extract_generic_arg(ty)
    {
        return unwrap_pyo3_type(&inner);
    }

    // Py<T> -> T
    if (ty.starts_with("Py <") || ty.starts_with("Py<"))
        && let Some(inner) = extract_generic_arg(ty)
    {
        return unwrap_pyo3_type(&inner);
    }

    // PyRefMut<'_, T> -> T
    if ty.starts_with("PyRefMut")
        && let Some(inner) = extract_generic_arg(ty)
    {
        // Handle lifetime: PyRefMut<'py, Self> -> Self
        let inner = inner.trim();
        if let Some(comma_pos) = inner.find(',') {
            let after_comma = inner[comma_pos + 1..].trim();
            return unwrap_pyo3_type(after_comma);
        }
        return unwrap_pyo3_type(inner);
    }

    // PyRef<'_, T> -> T
    if ty.starts_with("PyRef")
        && !ty.starts_with("PyRefMut")
        && let Some(inner) = extract_generic_arg(ty)
    {
        let inner = inner.trim();
        if let Some(comma_pos) = inner.find(',') {
            let after_comma = inner[comma_pos + 1..].trim();
            return unwrap_pyo3_type(after_comma);
        }
        return unwrap_pyo3_type(inner);
    }

    // Bound<'_, T> -> T
    if ty.starts_with("Bound") || ty.starts_with("& Bound") {
        if let Some(inner) = extract_generic_arg(ty) {
            let inner = inner.trim();
            if let Some(comma_pos) = inner.find(',') {
                let after_comma = inner[comma_pos + 1..].trim();
                return unwrap_pyo3_type(after_comma);
            }
        }
        // Bound<'_, PyAny> means dynamic type - compatible with anything
        return "Any".to_string();
    }

    ty.to_string()
}

/// Extract the generic argument from Type<Arg>
fn extract_generic_arg(ty: &str) -> Option<String> {
    let start = ty.find('<')? + 1;
    let end = ty.rfind('>')?;
    if end > start {
        Some(ty[start..end].trim().to_string())
    } else {
        None
    }
}

/// Simplify a full Rust type path for display
/// e.g., "core::option::Option<alloc::string::String>" -> "Option<String>"
pub fn simplify_type_for_display(ty: &str) -> String {
    let ty = ty.trim();

    // Handle Option<T>
    if ty.starts_with("core::option::Option<") || ty.starts_with("Option<") {
        let start = ty.find('<').unwrap_or(0) + 1;
        let end = ty.rfind('>').unwrap_or(ty.len());
        if end > start {
            let inner = &ty[start..end];
            return format!("Option<{}>", simplify_type_for_display(inner));
        }
    }

    // Handle tuples (X, Y)
    if ty.starts_with('(') && ty.ends_with(')') {
        let inner = &ty[1..ty.len() - 1];
        let parts: Vec<&str> = split_generic_args(inner);
        let simplified: Vec<String> = parts.iter().map(|p| simplify_type_for_display(p)).collect();
        return format!("({})", simplified.join(", "));
    }

    // Handle generic types like NonZero<u32>
    if let Some(angle_pos) = ty.find('<') {
        let base = &ty[..angle_pos];
        let rest = &ty[angle_pos..];
        let simplified_base = simplify_path(base);
        if let Some(inner_start) = rest.find('<') {
            let inner_end = rest.rfind('>').unwrap_or(rest.len());
            if inner_end > inner_start + 1 {
                let inner = &rest[inner_start + 1..inner_end];
                return format!("{}<{}>", simplified_base, simplify_type_for_display(inner));
            }
        }
        return format!("{}{}", simplified_base, rest);
    }

    simplify_path(ty)
}

/// Extract the last segment from a path (e.g., "bevy_window::WindowPosition" -> "WindowPosition")
fn simplify_path(path: &str) -> String {
    path.split("::").last().unwrap_or(path).to_string()
}

/// Split generic arguments respecting nested angle brackets
fn split_generic_args(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;

    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < s.len() {
        parts.push(s[start..].trim());
    }
    parts
}

/// Normalize a Bevy/Rust type for comparison
fn normalize_bevy_type(ty: &str) -> String {
    let ty = ty.trim();

    // Handle Self::AssociatedType patterns
    // These are common in Bevy's bounding volume traits
    if let Some(assoc_type) = ty.strip_prefix("Self::") {
        // strip "Self::"
        return match assoc_type {
            // BoundingVolume trait associated types
            "Translation" | "Position" => "Vec".to_string(), // Could be Vec2 or Vec3
            "HalfSize" => "Vec".to_string(),                 // Could be Vec2 or Vec3
            "Rotation" => "Rotation".to_string(),            // Rot2 or Quat
            // Just use the associated type name as-is for unknown ones
            _ => assoc_type.to_string(),
        };
    }

    // Handle impl Trait patterns - treat as Any since PyBevy uses dynamic typing
    if ty.starts_with("impl ") {
        return "Any".to_string();
    }

    // Handle Result<T, E> -> T (unwrap success type)
    if (ty.starts_with("Result<") || ty.starts_with("core::result::Result<"))
        && let Some(inner) = extract_generic_arg(ty)
    {
        // Result<T, E> -> get T (before the comma)
        let inner = inner.trim();
        if let Some(comma_pos) = inner.find(',') {
            let success_type = inner[..comma_pos].trim();
            return normalize_bevy_type(success_type);
        }
        return normalize_bevy_type(inner);
    }

    // Handle Option<T> -> T | None
    if (ty.starts_with("Option<") || ty.starts_with("core::option::Option<"))
        && let Some(inner) = extract_generic_arg(ty)
    {
        return format!("{} | None", normalize_bevy_type(&inner));
    }

    // Handle reference types, including explicit lifetimes: &T / &'a T -> T.
    let ty = strip_rust_reference(ty);

    // Handle Handle<T> before path extraction (because Handle<full::path::Type> would lose the Handle part)
    if ty.contains("Handle<") {
        return "Handle".to_string();
    }

    // Handle slice types: &[T] -> list[T]
    if ty.starts_with('[') && ty.ends_with(']') && !ty.contains(';') {
        let inner = &ty[1..ty.len() - 1];
        return format!("list[{}]", normalize_bevy_type(inner));
    }

    // Handle array types [T; N] - normalize the inner type too
    // e.g., [glam::f32::vec2::Vec2; 2] -> [Vec2;2]
    if ty.starts_with('[') && ty.ends_with(']') && ty.contains(';') {
        // Find the semicolon position (careful: could have nested brackets)
        if let Some(semi_pos) = ty.rfind(';') {
            let inner_type = &ty[1..semi_pos];
            let size = &ty[semi_pos + 1..ty.len() - 1];
            let normalized_inner = normalize_bevy_type(inner_type.trim());
            return format!("[{};{}]", normalized_inner, size.trim());
        }
    }

    // Extract last path segment: bevy_transform::components::Transform -> Transform
    let ty = ty.split("::").last().unwrap_or(ty);

    // Handle common type mappings
    match ty {
        "f32" | "f64" => "float".to_string(),
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
            "int".to_string()
        }
        "bool" => "bool".to_string(),
        "String" | "&str" | "str" => "str".to_string(),
        "()" => "None".to_string(),
        "Self" => "Self".to_string(),
        _ => {
            // Handle Handle<T> -> Handle (strip generic parameter, PyBevy uses single Handle type)
            if ty.starts_with("Handle<") && ty.ends_with('>') {
                return "Handle".to_string();
            }
            // Handle Vec<T> -> list[T]
            if ty.starts_with("Vec<") && ty.ends_with('>') {
                let inner = &ty[4..ty.len() - 1];
                format!("list[{}]", normalize_bevy_type(inner))
            }
            // Handle glam types
            else if ty.contains("Vec3A") || ty == "Vec3A" {
                "Vec3".to_string()
            } else if ty.contains("Vec3") || ty == "Vec3" {
                "Vec3".to_string()
            } else if ty.contains("Vec2") || ty == "Vec2" {
                "Vec2".to_string()
            } else if ty.contains("Quat") || ty == "Quat" {
                "Quat".to_string()
            } else if ty.contains("Mat4") || ty == "Mat4" {
                "Mat4".to_string()
            } else if ty.contains("Mat3") || ty == "Mat3" {
                "Mat3".to_string()
            }
            // Strip generic parameters for basic comparison
            else if let Some(base) = ty.split('<').next() {
                base.to_string()
            } else {
                ty.to_string()
            }
        }
    }
}

/// Normalize a PyBevy/Python type for comparison
fn normalize_pybevy_type(ty: &str) -> String {
    let ty = ty.trim();

    // Normalize spacing: remove extra spaces around :: and ; in arrays
    let ty = ty
        .replace(" :: ", "::")
        .replace(" ; ", "; ")
        .replace("; ", ";")
        .replace(" >", ">")
        .replace("< ", "<");

    // Handle reference types, including the spaced form emitted by syn.
    let ty = strip_rust_reference(&ty);

    // First unwrap PyO3 wrappers
    let ty = unwrap_pyo3_type(ty);

    // Extract last path segment: crate::math::vec3::PyVec3 -> PyVec3
    let ty = if ty.contains("::") {
        ty.split("::").last().unwrap_or(&ty).to_string()
    } else {
        ty
    };
    let ty = ty.as_str();

    // Handle union types: "X | None" -> keep as is for Option comparison
    if ty.contains(" | ") {
        return ty.to_string();
    }

    // Handle Option<T> -> T | None (same as Bevy normalization)
    // Note: spacing may vary (Option<f32> or Option < f32 >)
    if (ty.starts_with("Option<") || ty.starts_with("Option <"))
        && ty.ends_with('>')
        && let Some(inner) = extract_generic_arg(ty)
    {
        return format!("{} | None", normalize_pybevy_type(&inner));
    }

    // Handle list types
    if ty.starts_with("list[") && ty.ends_with(']') {
        let inner = &ty[5..ty.len() - 1];
        return format!("list[{}]", normalize_pybevy_type(inner));
    }

    // Handle Vec<T> -> list[T] (Rust Vec in PyBevy signatures)
    // Handle both "Vec<T>" and "Vec < T >" (with spaces)
    if (ty.starts_with("Vec<") || ty.starts_with("Vec <"))
        && ty.ends_with('>')
        && let Some(inner) = extract_generic_arg(ty)
    {
        return format!("list[{}]", normalize_pybevy_type(&inner));
    }

    // Handle array returns like [[f32;3];4] which represent matrices (check before regular arrays)
    if ty.starts_with("[[") && ty.contains("f32") {
        let normalized = ty.replace(' ', "");
        if normalized.contains("[f32;3];4") {
            return "Mat4".to_string();
        }
        if normalized.contains("[f32;3];3") {
            return "Mat3".to_string();
        }
    }

    // Handle array types [T; N] - normalize the inner type too
    // e.g., [PyVec2; 2] -> [Vec2;2]
    if ty.starts_with('[') && ty.ends_with(']') && ty.contains(';') {
        // Find the semicolon position
        if let Some(semi_pos) = ty.rfind(';') {
            let inner_type = &ty[1..semi_pos];
            let size = &ty[semi_pos + 1..ty.len() - 1];
            let normalized_inner = normalize_pybevy_type(inner_type.trim());
            return format!("[{};{}]", normalized_inner, size.trim());
        }
    }

    // Normalize common Python types and Rust primitives (PyBevy may use Rust types internally)
    match ty {
        "f32" | "f64" | "float" => "float".to_string(),
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" | "int" => {
            "int".to_string()
        }
        "bool" => "bool".to_string(),
        "str" | "String" | "&str" => "str".to_string(),
        "()" | "None" => "None".to_string(),
        "Any" | "PyAny" => "Any".to_string(),
        "Self" => "Self".to_string(),
        _ => {
            // Handle glam types (same as Bevy normalization)
            if ty.contains("Vec3") || ty == "Vec3" || ty == "PyVec3" {
                return "Vec3".to_string();
            }
            if ty.contains("Vec2") || ty == "Vec2" || ty == "PyVec2" {
                return "Vec2".to_string();
            }
            if ty.contains("Vec4") || ty == "Vec4" || ty == "PyVec4" {
                return "Vec4".to_string();
            }
            if ty.contains("Quat") || ty == "Quat" || ty == "PyQuat" {
                return "Quat".to_string();
            }
            if ty.contains("Mat4") || ty == "Mat4" || ty == "PyMat4" {
                return "Mat4".to_string();
            }
            if ty.contains("Mat3") || ty == "Mat3" || ty == "PyMat3" {
                return "Mat3".to_string();
            }

            // Strip Py prefix for comparison
            if ty.starts_with("Py")
                && ty.len() > 2
                && ty.chars().nth(2).map(|c| c.is_uppercase()).unwrap_or(false)
            {
                ty[2..].to_string()
            } else {
                ty.to_string()
            }
        }
    }
}

fn strip_rust_reference(ty: &str) -> &str {
    let Some(rest) = ty.strip_prefix('&') else {
        return ty;
    };
    let mut rest = rest.trim_start();
    if rest.starts_with('\'') {
        rest = rest
            .split_once(char::is_whitespace)
            .map_or(rest, |(_, ty)| ty);
    }
    rest.strip_prefix("mut ").unwrap_or(rest).trim_start()
}
