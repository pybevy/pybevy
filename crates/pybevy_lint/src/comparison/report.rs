use std::collections::HashMap;

/// Coverage report for the entire comparison
#[derive(Debug, Default)]
pub struct CoverageReport {
    /// Per-crate coverage statistics
    pub crates: HashMap<String, CrateCoverage>,

    /// Overall statistics
    pub total_bevy_types: usize,
    pub total_pybevy_types: usize,
    pub matched_types: usize,
    pub missing_types: usize,

    pub total_bevy_methods: usize,
    pub total_pybevy_methods: usize,
    pub matched_methods: usize,
    pub missing_methods: usize,
    /// Implemented methods whose parameters or return type differ from Bevy.
    pub signature_mismatches: usize,
    /// Extra methods in PyBevy that don't exist in Bevy
    pub extra_methods: usize,

    /// Methods only counting implemented types (more meaningful metric)
    pub implemented_type_bevy_methods: usize,
    pub implemented_type_matched_methods: usize,

    /// Field statistics (only for implemented types)
    pub total_bevy_fields: usize,
    pub matched_fields: usize,
    pub missing_fields: usize,

    /// Variant statistics (only for implemented enum types)
    pub total_bevy_variants: usize,
    pub matched_variants: usize,
    pub missing_variants: usize,
    pub mismatched_variants: usize,
    pub extra_variants: usize,
    /// Bevy enums represented as ordinary PyBevy classes instead of enum adapters.
    pub enum_representation_mismatches: usize,
}

/// Coverage statistics for a single Bevy crate
#[derive(Debug, Default)]
pub struct CrateCoverage {
    pub crate_name: String,
    pub pybevy_module: Option<String>,

    /// Types in this crate
    pub types: Vec<TypeCoverage>,

    /// Statistics
    pub bevy_type_count: usize,
    pub pybevy_type_count: usize,
    pub matched_count: usize,
    pub missing_count: usize,
}

/// Coverage information for a single type
#[derive(Debug)]
pub struct TypeCoverage {
    /// Bevy type name
    pub bevy_name: String,

    /// Full Bevy path
    pub bevy_path: String,

    /// Corresponding PyBevy class name (if exists)
    pub pybevy_name: Option<String>,

    /// Whether the type is implemented in PyBevy
    pub is_implemented: bool,

    /// Method coverage (if type is implemented)
    pub methods: Vec<MethodCoverage>,

    /// Extra methods in PyBevy that don't exist in Bevy
    pub extra_methods: Vec<ExtraMethodCoverage>,

    /// Field coverage
    pub fields: Vec<FieldCoverage>,

    /// Variant coverage (for enums)
    pub variants: Vec<VariantCoverage>,

    /// `Some(false)` when the matched Bevy item is an enum but PyBevy declares a struct.
    pub enum_representation_matches: Option<bool>,

    /// Constructor warnings (optional params that should be required)
    pub constructor_warnings: Vec<ConstructorWarning>,

    /// Extends/trait implementation warnings
    pub extends_warnings: Vec<ExtendsWarning>,

    /// Module placement mismatch (e.g., type in pybevy_render but Bevy has it in bevy_pbr)
    pub module_mismatch: Option<ModuleMismatch>,

    /// Statistics
    pub bevy_method_count: usize,
    pub pybevy_method_count: usize,
    pub matched_method_count: usize,
    pub extra_method_count: usize,

    /// Field statistics
    pub bevy_field_count: usize,
    pub matched_field_count: usize,
    /// Number of fields settable via constructor params
    pub constructor_settable_field_count: usize,

    /// Variant statistics (for enums)
    pub bevy_variant_count: usize,
    pub pybevy_variant_count: usize,
    pub matched_variant_count: usize,
    pub extra_variant_count: usize,
}

/// Module placement mismatch warning
#[derive(Debug, Clone)]
pub struct ModuleMismatch {
    /// Expected PyBevy module (derived from Bevy crate via crate_mappings)
    pub expected_module: String,
    /// Actual PyBevy module where the type is defined
    pub actual_module: String,
    /// Bevy crate name
    pub bevy_crate: String,
}

/// Warning about constructor parameter optionality mismatch
#[derive(Debug, Clone)]
pub struct ConstructorWarning {
    /// The parameter name
    pub param_name: String,
    /// The Bevy field type (mandatory, non-Option)
    pub bevy_type: String,
    /// Warning message
    pub message: String,
}

/// Warning about extends/trait implementation mismatch
#[derive(Debug, Clone)]
pub struct ExtendsWarning {
    /// The kind of warning
    pub kind: ExtendsWarningKind,
    /// Warning message
    pub message: String,
}

/// The kind of extends/trait mismatch
#[derive(Debug, Clone)]
pub enum ExtendsWarningKind {
    /// Bevy type implements trait but PyBevy doesn't extend matching base class
    MissingExtends {
        /// The Bevy trait (e.g., "Component", "Resource")
        bevy_trait: String,
        /// The expected PyBevy base class (e.g., "PyComponent", "Component")
        expected_base: String,
    },
    /// PyBevy extends a base class but Bevy type doesn't implement matching trait
    UnexpectedExtends {
        /// What PyBevy extends (e.g., "PyComponent", "Component")
        pybevy_extends: String,
        /// The expected Bevy trait (e.g., "Component", "Resource")
        expected_trait: String,
    },
    /// Bevy type derives PartialEq but PyBevy pyclass is missing `eq` attribute
    MissingEq,
    /// Bevy implements multiple traits (e.g., Component + Resource) but Python only supports
    /// single inheritance, so PyBevy chose one. This is informational, not a real warning.
    AlternativeExtends {
        /// The Bevy trait that PyBevy doesn't extend
        bevy_trait: String,
        /// What PyBevy actually extends
        pybevy_extends: String,
        /// The Bevy trait that PyBevy does extend
        matched_trait: String,
    },
}

/// Coverage information for a method
#[derive(Debug)]
pub struct MethodCoverage {
    /// Method name in Bevy
    pub bevy_name: String,

    /// Method name in PyBevy (if different)
    pub pybevy_name: Option<String>,

    /// Full Bevy signature
    pub bevy_signature: String,

    /// Whether the method is implemented
    pub is_implemented: bool,

    /// Whether signatures match (if implemented)
    pub signature_matches: bool,

    /// Signature differences (if any)
    pub differences: Vec<SignatureDiff>,

    /// Bevy parameter count
    pub bevy_param_count: usize,

    /// PyBevy parameter count (if implemented)
    pub pybevy_param_count: Option<usize>,

    /// Types required to implement this method that PyBevy doesn't have yet
    pub missing_required_types: Vec<String>,
}

/// Coverage information for an extra method (exists in PyBevy but not in Bevy)
#[derive(Debug)]
pub struct ExtraMethodCoverage {
    /// Method name in PyBevy
    pub pybevy_name: String,
    /// Whether the method is a static method
    pub is_static: bool,
    /// Whether the method is a property getter
    pub is_property: bool,
}

/// A specific signature difference
#[derive(Debug, Clone)]
pub enum SignatureDiff {
    /// Parameter count mismatch
    ParamCountMismatch { bevy: usize, pybevy: usize },

    /// Parameter name differs
    ParamNameMismatch {
        index: usize,
        bevy: String,
        pybevy: String,
    },

    /// Parameter type differs
    ParamTypeMismatch {
        param_name: String,
        bevy_type: String,
        pybevy_type: String,
    },

    /// Return type differs
    ReturnTypeMismatch {
        bevy_type: String,
        pybevy_type: String,
    },

    /// Self mutability differs (e.g., &self vs &mut self)
    SelfMutabilityMismatch { bevy: String, pybevy: String },

    /// Extra parameter in PyBevy not in Bevy
    ExtraParam { name: String },

    /// Missing parameter in PyBevy that's in Bevy
    MissingParam { name: String, param_type: String },

    /// Expected PyBevy type doesn't match (regression in intentional sig diff)
    ExpectedTypeMismatch {
        context: String,
        expected: String,
        actual: String,
    },
}

impl std::fmt::Display for SignatureDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignatureDiff::ParamCountMismatch { bevy, pybevy } => {
                write!(f, "param count: Bevy={}, PyBevy={}", bevy, pybevy)
            }
            SignatureDiff::ParamNameMismatch {
                index,
                bevy,
                pybevy,
            } => {
                write!(
                    f,
                    "param {} name: Bevy='{}', PyBevy='{}'",
                    index, bevy, pybevy
                )
            }
            SignatureDiff::ParamTypeMismatch {
                param_name,
                bevy_type,
                pybevy_type,
            } => {
                write!(
                    f,
                    "'{}' type: Bevy={}, PyBevy={}",
                    param_name, bevy_type, pybevy_type
                )
            }
            SignatureDiff::ReturnTypeMismatch {
                bevy_type,
                pybevy_type,
            } => {
                write!(f, "return: Bevy={}, PyBevy={}", bevy_type, pybevy_type)
            }
            SignatureDiff::SelfMutabilityMismatch { bevy, pybevy } => {
                write!(f, "self: Bevy={}, PyBevy={}", bevy, pybevy)
            }
            SignatureDiff::ExtraParam { name } => {
                write!(f, "extra param '{}'", name)
            }
            SignatureDiff::MissingParam { name, param_type } => {
                write!(f, "missing param '{}': {}", name, param_type)
            }
            SignatureDiff::ExpectedTypeMismatch {
                context,
                expected,
                actual,
            } => {
                write!(
                    f,
                    "REGRESSION {}: expected '{}', got '{}'",
                    context, expected, actual
                )
            }
        }
    }
}

/// Coverage information for a field/property
#[derive(Debug)]
pub struct FieldCoverage {
    pub bevy_name: String,
    pub bevy_type: String,
    pub pybevy_name: Option<String>,
    /// Whether the field is readable (via property or getter method)
    pub is_implemented: bool,
    /// Whether the field can be set via constructor parameter
    pub in_constructor: bool,
}

/// Coverage information for an enum variant
#[derive(Debug)]
pub struct VariantCoverage {
    /// Variant name in Bevy
    pub bevy_name: String,
    /// Variant kind in Bevy (Unit, Tuple, Struct)
    pub bevy_kind: String,
    /// Whether the variant exists in PyBevy
    pub is_implemented: bool,
    /// If implemented, the kind in PyBevy
    pub pybevy_kind: Option<String>,
    /// Whether the variant kinds match
    pub kind_matches: bool,
    /// True if this variant exists in PyBevy but not in Bevy
    pub is_extra: bool,
}

impl CoverageReport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate overall coverage percentage
    pub fn type_coverage_percent(&self) -> f64 {
        if self.total_bevy_types == 0 {
            return 100.0;
        }
        (self.matched_types as f64 / self.total_bevy_types as f64) * 100.0
    }

    /// Calculate method coverage percentage (all Bevy types)
    pub fn method_coverage_percent(&self) -> f64 {
        if self.total_bevy_methods == 0 {
            return 100.0;
        }
        (self.matched_methods as f64 / self.total_bevy_methods as f64) * 100.0
    }

    /// Calculate method coverage percentage (only implemented types)
    /// This is a more meaningful metric since it shows coverage within what PyBevy targets
    pub fn implemented_method_coverage_percent(&self) -> f64 {
        if self.implemented_type_bevy_methods == 0 {
            return 100.0;
        }
        (self.implemented_type_matched_methods as f64 / self.implemented_type_bevy_methods as f64)
            * 100.0
    }

    /// Calculate field coverage percentage (only implemented types)
    pub fn field_coverage_percent(&self) -> f64 {
        if self.total_bevy_fields == 0 {
            return 100.0;
        }
        (self.matched_fields as f64 / self.total_bevy_fields as f64) * 100.0
    }

    /// Calculate variant coverage percentage (only implemented enum types)
    pub fn variant_coverage_percent(&self) -> f64 {
        if self.total_bevy_variants == 0 {
            return 100.0;
        }
        (self.matched_variants as f64 / self.total_bevy_variants as f64) * 100.0
    }

    /// Get list of missing types
    pub fn missing_types(&self) -> Vec<(&str, &str)> {
        let mut missing = Vec::new();
        for (crate_name, crate_coverage) in &self.crates {
            for type_coverage in &crate_coverage.types {
                if !type_coverage.is_implemented {
                    missing.push((crate_name.as_str(), type_coverage.bevy_name.as_str()));
                }
            }
        }
        missing
    }

    /// Get list of implemented types with missing methods
    pub fn types_with_missing_methods(&self) -> Vec<(&str, &TypeCoverage)> {
        let mut result = Vec::new();
        for (crate_name, crate_coverage) in &self.crates {
            for type_coverage in &crate_coverage.types {
                if type_coverage.is_implemented {
                    let missing_methods: Vec<_> = type_coverage
                        .methods
                        .iter()
                        .filter(|m| !m.is_implemented)
                        .collect();
                    if !missing_methods.is_empty() {
                        result.push((crate_name.as_str(), type_coverage));
                    }
                }
            }
        }
        result
    }
}

impl CrateCoverage {
    pub fn coverage_percent(&self) -> f64 {
        if self.bevy_type_count == 0 {
            return 100.0;
        }
        (self.matched_count as f64 / self.bevy_type_count as f64) * 100.0
    }
}

impl TypeCoverage {
    pub fn method_coverage_percent(&self) -> f64 {
        if self.bevy_method_count == 0 {
            return 100.0;
        }
        (self.matched_method_count as f64 / self.bevy_method_count as f64) * 100.0
    }

    pub fn field_coverage_percent(&self) -> f64 {
        if self.bevy_field_count == 0 {
            return 100.0;
        }
        (self.matched_field_count as f64 / self.bevy_field_count as f64) * 100.0
    }

    pub fn missing_methods(&self) -> Vec<&MethodCoverage> {
        self.methods.iter().filter(|m| !m.is_implemented).collect()
    }

    pub fn missing_fields(&self) -> Vec<&FieldCoverage> {
        self.fields.iter().filter(|f| !f.is_implemented).collect()
    }

    /// Returns fields that are readable but not settable via constructor
    pub fn fields_not_in_constructor(&self) -> Vec<&FieldCoverage> {
        self.fields
            .iter()
            .filter(|f| f.is_implemented && !f.in_constructor)
            .collect()
    }

    pub fn variant_coverage_percent(&self) -> f64 {
        if self.bevy_variant_count == 0 {
            return 100.0;
        }
        (self.matched_variant_count as f64 / self.bevy_variant_count as f64) * 100.0
    }

    pub fn missing_variants(&self) -> Vec<&VariantCoverage> {
        self.variants
            .iter()
            .filter(|v| !v.is_implemented && !v.is_extra)
            .collect()
    }

    pub fn extra_variants(&self) -> Vec<&VariantCoverage> {
        self.variants.iter().filter(|v| v.is_extra).collect()
    }

    pub fn mismatched_variants(&self) -> Vec<&VariantCoverage> {
        self.variants
            .iter()
            .filter(|v| v.is_implemented && !v.kind_matches)
            .collect()
    }

    pub fn extra_methods(&self) -> &[ExtraMethodCoverage] {
        &self.extra_methods
    }
}
