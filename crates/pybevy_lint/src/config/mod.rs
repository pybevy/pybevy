use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;

/// Parsed representation of an intentional extra method signature
/// Supports formats like: "method_name", "method_name()", "method_name() -> ReturnType"
#[derive(Debug, Clone)]
pub struct ParsedExtraMethod {
    pub name: String,
    pub params: Option<String>,
    pub return_type: Option<String>,
    pub raw: String,
}

impl ParsedExtraMethod {
    /// Parse a method signature string
    /// Examples:
    /// - "to_vec3" -> name only
    /// - "to_vec3()" -> name with empty params
    /// - "to_vec3() -> Vec3" -> name with return type
    /// - "from_components(x: f32, y: f32) -> Self" -> full signature
    pub fn parse(s: &str) -> Self {
        let raw = s.to_string();
        let s = s.trim();

        let (before_arrow, return_type) = if let Some(idx) = s.find("->") {
            let ret = s[idx + 2..].trim().to_string();
            (s[..idx].trim(), Some(ret))
        } else {
            (s, None)
        };

        if let Some(paren_start) = before_arrow.find('(') {
            let name = before_arrow[..paren_start].trim().to_string();
            let params_end = before_arrow.rfind(')').unwrap_or(before_arrow.len());
            let params = before_arrow[paren_start + 1..params_end].trim();
            // Use Some("") for empty params to preserve () in signature
            let params = Some(params.to_string());
            Self {
                name,
                params,
                return_type,
                raw,
            }
        } else {
            Self {
                name: before_arrow.to_string(),
                params: None,
                return_type,
                raw,
            }
        }
    }

    /// Format as a display signature
    pub fn signature(&self) -> String {
        let mut sig = self.name.clone();
        if self.params.is_some() || self.return_type.is_some() {
            sig.push('(');
            if let Some(ref params) = self.params {
                sig.push_str(params);
            }
            sig.push(')');
        }
        if let Some(ref ret) = self.return_type {
            sig.push_str(" -> ");
            sig.push_str(ret);
        }
        sig
    }
}

/// Top-level configuration structure
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub bevy: BevyConfig,

    #[serde(default)]
    pub validation: ValidationConfig,

    #[serde(default)]
    pub test_coverage: TestCoverageConfig,
}

/// Exact, reviewed exceptions to Rust binding versus stub validation.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationConfig {
    #[serde(default)]
    pub exceptions: Vec<ValidationException>,
    #[serde(default)]
    pub disabled: Vec<DisabledDiagnostic>,
}

/// A diagnostic code turned off wholesale, with a durable reason.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisabledDiagnostic {
    /// Diagnostic code, such as `W007`.
    pub code: String,
    /// Durable explanation for why the whole code is advisory-only here.
    pub reason: String,
}

/// One diagnostic intentionally omitted from the public stub contract.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidationException {
    /// Diagnostic code, such as `E001` or `E008`.
    pub code: String,
    /// Parser-origin path in `<module>.<Class>` form.
    pub path: String,
    /// Durable explanation for why the mismatch is intentional.
    pub reason: String,
}

/// Test coverage configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestCoverageConfig {
    /// Path to test directory
    #[serde(default = "default_test_path")]
    pub test_path: String,

    /// Classes excluded from test coverage checks
    #[serde(default)]
    pub excluded_classes: Vec<String>,

    /// Wildcard patterns for excluded classes
    #[serde(default)]
    pub excluded_patterns: Vec<String>,

    /// Per-class member exclusions
    #[serde(default)]
    pub type_ignores: HashMap<String, TestCoverageTypeIgnores>,
}

fn default_test_path() -> String {
    "tests".to_string()
}

impl Default for TestCoverageConfig {
    fn default() -> Self {
        Self {
            test_path: default_test_path(),
            excluded_classes: Vec::new(),
            excluded_patterns: Vec::new(),
            type_ignores: HashMap::new(),
        }
    }
}

impl TestCoverageConfig {
    /// Check if a class should be excluded from coverage checking
    pub fn is_excluded(&self, class_path: &str, class_name: &str) -> bool {
        if self.excluded_classes.iter().any(|excluded| {
            excluded == class_path || (!excluded.contains('.') && excluded == class_name)
        }) {
            return true;
        }
        for pattern in &self.excluded_patterns {
            if matches_pattern(pattern, class_path) || matches_pattern(pattern, class_name) {
                return true;
            }
        }
        false
    }

    /// Check if a specific member should be excluded for a class
    pub fn is_member_excluded(
        &self,
        class_path: &str,
        class_name: &str,
        member_name: &str,
    ) -> bool {
        if let Some(ignores) = self
            .type_ignores
            .get(class_path)
            .or_else(|| self.type_ignores.get(class_name))
        {
            return ignores.excluded_members.contains(&member_name.to_string());
        }
        false
    }
}

/// Per-class test coverage configuration
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestCoverageTypeIgnores {
    /// Members excluded from coverage checks
    #[serde(default)]
    pub excluded_members: Vec<String>,
}

/// Bevy-specific configuration
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BevyConfig {
    /// Path to Bevy source
    #[serde(default)]
    pub path: Option<String>,

    /// PyBevy module -> Bevy crate mappings
    #[serde(default)]
    pub crate_mappings: HashMap<String, String>,

    /// Excluded crates configuration
    #[serde(default)]
    pub excluded_crates: ExcludedCrates,

    /// Excluded types configuration
    #[serde(default)]
    pub excluded_types: ExcludedTypes,

    /// Type name mappings (PyBevy -> Bevy)
    #[serde(default)]
    pub type_mappings: HashMap<String, String>,

    /// Dependency crates whose types a Bevy crate re-exports (e.g. `pub use
    /// glam::*` in bevy_math). Re-exported types are absent from the
    /// re-exporting crate's own public API, so the listed crates are parsed
    /// separately and wrapped types are merged in before comparison.
    #[serde(default)]
    pub crate_type_sources: HashMap<String, Vec<String>>,

    /// Excluded methods configuration
    #[serde(default)]
    pub excluded_methods: ExcludedMethods,

    /// Excluded traits configuration (methods from these traits are skipped)
    #[serde(default)]
    pub excluded_traits: ExcludedTraits,

    /// Per-type ignore configuration
    #[serde(default)]
    pub type_ignores: HashMap<String, TypeIgnores>,

    /// Explicit field mapping decisions used by `emit-mappings`.
    #[serde(default)]
    pub mapping_overrides: HashMap<String, HashMap<String, FieldMappingOverride>>,

    /// Wrapped components that Bevy's type registry cannot reflect at runtime.
    #[serde(default)]
    pub reflection_unavailable_types: Vec<String>,

    /// Types that are false matches across modules.
    /// When a Bevy type from one crate matches a PyBevy class from a different module
    /// by name only, and the types are actually different (e.g., bevy_ui::State is a
    /// focus tracking struct, while PyBevy's State is an ECS state machine), list
    /// them here as "crate::Type" (e.g., "ui::State") to suppress the false match.
    /// The Bevy type will be treated as unimplemented instead of falsely matched.
    #[serde(default)]
    pub false_match_types: Vec<String>,

    /// Module placement overrides: suppress module mismatch warnings for types
    /// intentionally placed in a different PyBevy module than Bevy's crate.
    /// Format: "bevy_crate::Type" = "pybevy_module" (e.g., "math::Circle" = "mesh")
    #[serde(default)]
    pub module_placement_overrides: HashMap<String, String>,
}

/// Explicit mapping or exclusion for one Bevy field.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldMappingOverride {
    /// Python property name when it does not follow a convention.
    #[serde(default)]
    pub pybevy_name: Option<String>,
    /// Required normalizer when inference is not sufficient.
    #[serde(default)]
    pub normalizer: Option<String>,
    /// Non-empty reason when this field is intentionally excluded.
    #[serde(default)]
    pub exclude_reason: Option<String>,
}

/// An intentionally omitted item (method or field) with optional per-item note
/// Can be specified as either:
/// - A simple string: "method_name"
/// - An object with note: { name = "method_name", note = "Why it's omitted" }
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum IntentionalOmission {
    /// Simple name without note
    Simple(String),
    /// Name with per-item note
    WithNote { name: String, note: String },
}

impl IntentionalOmission {
    /// Get the name of the omitted item
    pub fn name(&self) -> &str {
        match self {
            IntentionalOmission::Simple(s) => s,
            IntentionalOmission::WithNote { name, .. } => name,
        }
    }

    /// Get the per-item note (if any)
    pub fn note(&self) -> Option<&str> {
        match self {
            IntentionalOmission::Simple(_) => None,
            IntentionalOmission::WithNote { note, .. } => Some(note),
        }
    }
}

/// Intentional signature difference specification.
/// Supports two forms:
/// - A simple string: "method_name" (suppresses sig diff without validation)
/// - An object with expected PyBevy types: { name = "method_name", expected_params = { "param" = "type" }, expected_return = "type" }
///   When expected types are specified, the lint validates the PyBevy side matches them,
///   catching regressions if someone accidentally changes the Python API.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SignatureDiffSpec {
    /// Simple name: just suppress the diff
    Simple(String),
    /// Name with expected PyBevy types: suppress diff but validate PyBevy side
    WithExpected {
        name: String,
        /// Expected PyBevy parameter types (param_name -> expected_type substring)
        #[serde(default)]
        expected_params: HashMap<String, String>,
        /// Expected PyBevy return type (substring match)
        #[serde(default)]
        expected_return: Option<String>,
    },
}

impl SignatureDiffSpec {
    pub fn name(&self) -> &str {
        match self {
            SignatureDiffSpec::Simple(s) => s,
            SignatureDiffSpec::WithExpected { name, .. } => name,
        }
    }
}

/// Per-type ignore configuration
#[derive(Debug, Default, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TypeIgnores {
    /// Ignore all constructor warnings for this type
    #[serde(default)]
    pub ignore_constructor_warnings: bool,

    /// Specific constructor params to ignore warnings for
    #[serde(default)]
    pub ignore_constructor_params: Vec<String>,

    /// Methods with intentional signature differences (param renames, etc.)
    /// Can be simple strings or objects with expected PyBevy types for regression detection
    #[serde(default)]
    pub intentional_signature_diffs: Vec<SignatureDiffSpec>,

    /// Methods installed at runtime outside the canonical wrapper's crate.
    ///
    /// These count as implemented for Bevy coverage. Their Python signatures
    /// must be pinned by the runtime surface contract because the Rust source
    /// parser cannot see dynamic class assignment.
    #[serde(default)]
    pub runtime_injected_methods: Vec<IntentionalOmission>,

    /// Missing methods that are intentionally not implemented
    /// Can be simple strings or objects with per-item notes
    #[serde(default)]
    pub intentional_missing_methods: Vec<IntentionalOmission>,

    /// Missing fields that are intentionally not exposed
    /// Can be simple strings or objects with per-item notes
    #[serde(default)]
    pub intentional_missing_fields: Vec<IntentionalOmission>,

    /// Extra methods that are intentionally added (not in Bevy)
    #[serde(default)]
    pub intentional_extra_methods: Vec<String>,

    /// Bevy enum variants that are intentionally not represented in Python
    #[serde(default)]
    pub intentional_missing_variants: Vec<IntentionalOmission>,

    /// Python enum variants that intentionally have no Bevy counterpart,
    /// such as one Bevy variant split into several for Python clarity
    #[serde(default)]
    pub intentional_extra_variants: Vec<String>,

    /// Methods that come from a Deref target type (e.g., AccessibilityNode -> accesskit::Node)
    /// These are legitimate Bevy API methods exposed through Deref and should not be reported as extra
    #[serde(default)]
    pub deref_target_methods: Vec<String>,

    /// Ignore extends/trait warnings for this type
    /// Use when PyBevy intentionally uses a type differently than Bevy
    /// (e.g., value type vs component, or chose Resource over Component)
    #[serde(default)]
    pub ignore_extends_warnings: bool,

    /// Properties that are intentionally read-only (getter without setter)
    /// Suppresses W007 warnings for these properties
    /// Examples: computed values, invariant-protected fields, engine-managed state
    #[serde(default)]
    pub intentional_readonly_properties: Vec<String>,

    /// Fields that are intentionally not settable via constructor
    /// Suppresses warnings for fields that can be read but not set at construction time
    /// Examples: computed fields, fields set by convenience params like "color" setting all sides
    #[serde(default)]
    pub intentional_not_in_constructor: Vec<String>,

    /// Renamed items: Bevy name -> PyBevy name
    /// Covers variant renames (None -> None_), method renames (with -> with_),
    /// and field renames (global -> global_).
    /// When a rename is configured, the item won't be reported as both missing and extra.
    #[serde(default)]
    pub renames: HashMap<String, String>,

    /// Notes explaining why items are intentionally missing/different
    /// Displayed in the linter output when showing intentional omissions
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludedCrates {
    #[serde(default)]
    pub crates: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludedTypes {
    /// Wildcard patterns (e.g., "*Builder", "Reflect*")
    #[serde(default)]
    pub patterns: Vec<String>,

    /// Specific type names
    #[serde(default)]
    pub types: Vec<String>,

    /// Engine-managed component types that don't need user-constructable defaults.
    /// These are inserted/managed by Bevy's internal systems, not user code.
    /// The linter will skip `default()` method warnings for these types.
    #[serde(default)]
    pub engine_managed_types: Vec<String>,

    /// Types that are intentionally excluded despite appearing in Bevy examples.
    /// These won't trigger audit warnings. Use for types where PyBevy has an
    /// alternative API (e.g., schedule labels replaced by Stage enum).
    #[serde(default)]
    pub audit_ignore: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludedMethods {
    /// Wildcard patterns for method names
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExcludedTraits {
    /// Trait names to exclude (methods from these traits will be skipped)
    /// Use short names (e.g., "AsRef") or full paths (e.g., "core::convert::AsRef")
    #[serde(default)]
    pub traits: Vec<String>,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))
    }

    /// Try to find and load config from standard locations
    pub fn find_and_load(start_dir: &Path) -> Result<Option<Self>> {
        let mut current = start_dir.to_path_buf();

        loop {
            let config_path = current.join(".pybevy-lint.toml");
            if config_path.exists() {
                return Ok(Some(Self::load(&config_path)?));
            }

            // Also check in crates/pybevy_lint/
            let lint_config = current.join("crates/pybevy_lint/.pybevy-lint.toml");
            if lint_config.exists() {
                return Ok(Some(Self::load(&lint_config)?));
            }

            if !current.pop() {
                break;
            }
        }

        Ok(None)
    }
}

impl BevyConfig {
    /// Return the exact configured rule that excludes a type, if any.
    ///
    /// Unlike [`Self::is_type_excluded_in_crate`], this preserves the matched
    /// rule for the reproducible upstream API inventory.
    pub fn type_exclusion_rule(&self, type_name: &str, crate_name: Option<&str>) -> Option<String> {
        let base_name = type_name.split('<').next().unwrap_or(type_name);

        for candidate in [type_name, base_name] {
            if self
                .excluded_types
                .types
                .iter()
                .any(|value| value == candidate)
            {
                return Some(format!("exact:{candidate}"));
            }
        }

        if let Some(crate_name) = crate_name {
            let short_module = crate_name.strip_prefix("bevy_").unwrap_or(crate_name);
            for candidate in [type_name, base_name] {
                for qualified in [
                    format!("{crate_name}::{candidate}"),
                    format!("{short_module}::{candidate}"),
                ] {
                    if self
                        .excluded_types
                        .types
                        .iter()
                        .any(|value| value == &qualified)
                    {
                        return Some(format!("exact:{qualified}"));
                    }
                }
            }
        }

        for pattern in &self.excluded_types.patterns {
            if matches_pattern(pattern, type_name)
                || (base_name != type_name && matches_pattern(pattern, base_name))
            {
                return Some(format!("pattern:{pattern}"));
            }
        }
        None
    }

    /// Return the exact configured rule that excludes a method, if any.
    pub fn method_exclusion_rule(&self, method_name: &str) -> Option<String> {
        self.excluded_methods
            .patterns
            .iter()
            .find(|pattern| matches_pattern(pattern, method_name))
            .map(|pattern| format!("pattern:{pattern}"))
    }

    /// Return the exact configured trait rule that excludes a method, if any.
    pub fn trait_exclusion_rule(&self, trait_name: &str) -> Option<String> {
        self.excluded_traits
            .traits
            .iter()
            .find(|excluded| {
                trait_name == excluded.as_str()
                    || trait_name.ends_with(&format!("::{excluded}"))
                    || (excluded.contains("::") && trait_name.ends_with(excluded.as_str()))
                    || trait_name
                        .rsplit("::")
                        .next()
                        .is_some_and(|short_name| short_name == excluded.as_str())
            })
            .map(|excluded| format!("trait:{excluded}"))
    }

    /// Get the resolved Bevy path
    pub fn bevy_path(&self) -> Option<PathBuf> {
        self.path.as_ref().map(|p| {
            if p.starts_with("~/") {
                dirs::home_dir()
                    .map(|home| home.join(&p[2..]))
                    .unwrap_or_else(|| PathBuf::from(p))
            } else {
                PathBuf::from(p)
            }
        })
    }

    /// Check if a crate should be excluded
    pub fn is_crate_excluded(&self, crate_name: &str) -> bool {
        self.excluded_crates
            .crates
            .contains(&crate_name.to_string())
    }

    /// Check if a type should be excluded
    /// Check if a type should be excluded (without crate context)
    pub fn is_type_excluded(&self, type_name: &str) -> bool {
        self.is_type_excluded_in_crate(type_name, None)
    }

    /// Check if a type should be excluded, with optional crate context
    /// Supports module-prefixed exclusions like "input::TouchPhase" or "bevy_input::TouchPhase"
    /// Also handles generic types (e.g., "LoadOp<f32>" matches "camera::LoadOp")
    pub fn is_type_excluded_in_crate(&self, type_name: &str, crate_name: Option<&str>) -> bool {
        self.type_exclusion_rule(type_name, crate_name).is_some()
    }

    /// Check if a method should be excluded
    pub fn is_method_excluded(&self, method_name: &str) -> bool {
        self.method_exclusion_rule(method_name).is_some()
    }

    /// Check if a trait is excluded (methods from excluded traits are skipped)
    pub fn is_trait_excluded(&self, trait_name: &str) -> bool {
        for excluded in &self.excluded_traits.traits {
            // Match full path or short name
            if trait_name == excluded {
                return true;
            }
            // Match short name against end of full path (e.g., "AsRef" matches "core::convert::AsRef")
            if trait_name.ends_with(&format!("::{}", excluded)) {
                return true;
            }
            // Match excluded path against end of trait name
            if excluded.contains("::") && trait_name.ends_with(excluded) {
                return true;
            }
            // Match just the trait name part (last segment)
            if let Some(short_name) = trait_name.rsplit("::").next()
                && short_name == excluded
            {
                return true;
            }
        }
        false
    }

    /// Get all excluded crates as a set
    pub fn excluded_crates_set(&self) -> HashSet<&str> {
        self.excluded_crates
            .crates
            .iter()
            .map(|s| s.as_str())
            .collect()
    }

    /// Map a PyBevy type name to Bevy type name
    pub fn map_type_name(&self, pybevy_name: &str) -> String {
        self.type_mappings
            .get(pybevy_name)
            .cloned()
            .unwrap_or_else(|| {
                if pybevy_name.starts_with("Py") && pybevy_name.len() > 2 {
                    pybevy_name[2..].to_string()
                } else {
                    pybevy_name.to_string()
                }
            })
    }

    /// Is this Bevy variant intentionally absent from the Python enum?
    pub fn is_variant_intentionally_missing(&self, type_name: &str, variant: &str) -> bool {
        self.type_ignores.get(type_name).is_some_and(|ignores| {
            ignores
                .intentional_missing_variants
                .iter()
                .any(|omission| omission.name() == variant)
        })
    }

    /// Is this Python variant intentionally absent from the Bevy enum?
    pub fn is_variant_intentionally_extra(&self, type_name: &str, variant: &str) -> bool {
        self.type_ignores.get(type_name).is_some_and(|ignores| {
            ignores
                .intentional_extra_variants
                .iter()
                .any(|name| name == variant)
        })
    }

    /// Get type-specific ignore configuration
    pub fn get_type_ignores(&self, type_name: &str) -> Option<&TypeIgnores> {
        self.type_ignores.get(type_name)
    }

    /// Check if constructor warnings should be ignored for a type
    pub fn should_ignore_constructor_warnings(&self, type_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| t.ignore_constructor_warnings)
            .unwrap_or(false)
    }

    /// Check if extends/trait warnings should be ignored for a type
    pub fn should_ignore_extends_warnings(&self, type_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| t.ignore_extends_warnings)
            .unwrap_or(false)
    }

    /// Check if a specific constructor param warning should be ignored
    pub fn should_ignore_constructor_param(&self, type_name: &str, param_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| {
                t.ignore_constructor_warnings
                    || t.ignore_constructor_params
                        .contains(&param_name.to_string())
            })
            .unwrap_or(false)
    }

    /// Check if a signature difference is intentional, returning the spec if found
    pub fn get_intentional_signature_diff(
        &self,
        type_name: &str,
        method_name: &str,
    ) -> Option<&SignatureDiffSpec> {
        self.type_ignores.get(type_name).and_then(|t| {
            t.intentional_signature_diffs
                .iter()
                .find(|s| s.name() == method_name)
        })
    }

    /// Check if a signature difference is intentional (convenience bool)
    pub fn is_intentional_signature_diff(&self, type_name: &str, method_name: &str) -> bool {
        self.get_intentional_signature_diff(type_name, method_name)
            .is_some()
    }

    /// Check if a method is implemented by a reviewed runtime class injection.
    pub fn is_runtime_injected_method(&self, type_name: &str, method_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|type_ignores| {
                type_ignores
                    .runtime_injected_methods
                    .iter()
                    .any(|method| method.name() == method_name)
            })
            .unwrap_or(false)
    }

    /// Return reviewed runtime-injected methods for one wrapper type.
    pub fn runtime_injected_methods(&self, type_name: &str) -> &[IntentionalOmission] {
        self.type_ignores
            .get(type_name)
            .map(|type_ignores| type_ignores.runtime_injected_methods.as_slice())
            .unwrap_or(&[])
    }

    /// Check if a Bevy name is renamed in PyBevy (returns the PyBevy name if so)
    pub fn get_rename(&self, type_name: &str, bevy_name: &str) -> Option<&str> {
        self.type_ignores
            .get(type_name)
            .and_then(|t| t.renames.get(bevy_name).map(|s| s.as_str()))
    }

    /// Check if a PyBevy name is the target of a rename (returns the Bevy name if so)
    pub fn is_rename_target(&self, type_name: &str, pybevy_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| t.renames.values().any(|v| v == pybevy_name))
            .unwrap_or(false)
    }

    /// Check if a missing method is intentional
    pub fn is_intentional_missing_method(&self, type_name: &str, method_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| {
                t.intentional_missing_methods
                    .iter()
                    .any(|m| m.name() == method_name)
            })
            .unwrap_or(false)
    }

    /// Check if a missing field is intentional
    pub fn is_intentional_missing_field(&self, type_name: &str, field_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| {
                t.intentional_missing_fields
                    .iter()
                    .any(|f| f.name() == field_name)
            })
            .unwrap_or(false)
    }

    /// Check if a field is intentionally not in constructor
    pub fn is_intentional_not_in_constructor(&self, type_name: &str, field_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| {
                t.intentional_not_in_constructor
                    .contains(&field_name.to_string())
            })
            .unwrap_or(false)
    }

    /// Check if an extra method is intentional (allowed to be in PyBevy but not in Bevy)
    /// Parses signatures like "to_vec3() -> Vec3" and matches by method name only
    pub fn is_intentional_extra_method(&self, type_name: &str, method_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| {
                t.intentional_extra_methods.iter().any(|sig| {
                    let parsed = ParsedExtraMethod::parse(sig);
                    parsed.name == method_name
                })
            })
            .unwrap_or(false)
    }

    /// Get parsed intentional extra methods for a type (for display purposes)
    pub fn get_intentional_extra_methods(&self, type_name: &str) -> Vec<ParsedExtraMethod> {
        self.type_ignores
            .get(type_name)
            .map(|t| {
                t.intentional_extra_methods
                    .iter()
                    .map(|s| ParsedExtraMethod::parse(s))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check if a method comes from a Deref target type
    /// These are legitimate Bevy API methods exposed through Deref and should not be reported as extra
    pub fn is_deref_target_method(&self, type_name: &str, method_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| t.deref_target_methods.iter().any(|m| m == method_name))
            .unwrap_or(false)
    }

    /// Check if a type from a specific Bevy crate is a known false match.
    /// Returns true if `"crate::Type"` or `"short_module::Type"` is in false_match_types.
    pub fn is_false_match(&self, type_name: &str, crate_name: &str) -> bool {
        let short_module = crate_name.strip_prefix("bevy_").unwrap_or(crate_name);
        let prefixed_full = format!("{}::{}", crate_name, type_name);
        let prefixed_short = format!("{}::{}", short_module, type_name);

        self.false_match_types
            .iter()
            .any(|s| s == &prefixed_full || s == &prefixed_short)
    }

    /// Check if a module placement is intentionally overridden.
    pub fn is_module_placement_override(
        &self,
        type_name: &str,
        bevy_crate: &str,
        pybevy_module: &str,
    ) -> bool {
        let short_module = bevy_crate.strip_prefix("bevy_").unwrap_or(bevy_crate);
        let key_full = format!("{}::{}", bevy_crate, type_name);
        let key_short = format!("{}::{}", short_module, type_name);

        if let Some(target) = self.module_placement_overrides.get(&key_full) {
            return target == pybevy_module;
        }
        if let Some(target) = self.module_placement_overrides.get(&key_short) {
            return target == pybevy_module;
        }
        false
    }

    /// Check if a property is intentionally read-only (should suppress W007)
    pub fn is_intentional_readonly_property(&self, type_name: &str, prop_name: &str) -> bool {
        self.type_ignores
            .get(type_name)
            .map(|t| {
                t.intentional_readonly_properties
                    .iter()
                    .any(|p| p == prop_name)
            })
            .unwrap_or(false)
    }
}

/// Simple wildcard pattern matching (supports * at start or end)
fn matches_pattern(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if let Some(inner) = pattern
        .strip_prefix('*')
        .and_then(|value| value.strip_suffix('*'))
    {
        // *foo* - contains
        text.contains(inner)
    } else if let Some(suffix) = pattern.strip_prefix('*') {
        // *foo - ends with
        text.ends_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('*') {
        // foo* - starts with
        text.starts_with(prefix)
    } else {
        // exact match
        pattern == text
    }
}
