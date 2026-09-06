use std::{collections::HashSet, path::PathBuf};

/// Source location information
#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

/// Unified representation of a Python-exposed class
#[derive(Debug, Clone, Default)]
pub struct PyClassDef {
    /// Python-visible name (from #[pyclass(name = "...")])
    pub python_name: String,
    /// Rust struct/enum name
    pub rust_name: String,
    /// Module path (e.g., "camera", "sprite", "input.keyboard")
    pub module_path: Option<String>,
    /// Parent class if extends = ...
    pub extends: Option<String>,
    /// Is frozen (immutable)
    pub frozen: bool,
    /// Implements eq
    pub eq: bool,
    /// Is subclass (can be inherited)
    pub subclass: bool,
    /// Source location
    pub location: Option<SourceLocation>,
    /// Constructor (#[new])
    pub constructor: Option<MethodDef>,
    /// Properties (#[getter]/#[setter] pairs)
    pub properties: Vec<PropertyDef>,
    /// Regular methods
    pub methods: Vec<MethodDef>,
    /// Static methods
    pub static_methods: Vec<MethodDef>,
    /// Class attributes (#[classattr])
    pub class_attrs: Vec<ClassAttrDef>,
    /// Custom macro info if using `#[pyenum]`.
    pub macro_info: Option<MacroInfo>,
    /// Component bridge info from `#[pycomponent(..., bridge)]` (if any)
    pub bridge_info: Option<ComponentBridgeInfo>,
    /// Storage macro attribute on the struct, independent of the `bridge` keyword
    pub storage_macro: Option<String>,
    /// Wildcard match arms in bevy -> Python conversions that substitute a value
    pub silent_fallbacks: Vec<SilentFallbackInfo>,
    /// Whether this is an enum (vs struct)
    pub is_enum: bool,
    /// Enum variants (if is_enum)
    pub enum_variants: Vec<EnumVariantDef>,
    /// Named Python variants explicitly mapped by declaration order to Bevy tuple variants.
    pub bevy_tuple_variants: HashSet<String>,
    /// Public classes nested directly inside this class.
    pub nested_types: Vec<String>,
}

/// Method definition
#[derive(Debug, Clone, Default)]
pub struct MethodDef {
    /// Python-visible method name
    pub name: String,
    /// Rust method name (may differ)
    pub rust_name: String,
    /// Method parameters
    pub parameters: Vec<ParameterDef>,
    /// Return type
    pub return_type: Option<String>,
    /// Is static method
    pub is_static: bool,
    /// Is class method
    pub is_class_method: bool,
    /// Self mutability
    pub self_mutability: SelfMutability,
    /// Source location
    pub location: Option<SourceLocation>,
    /// Original signature string for display
    pub signature_str: Option<String>,
    /// Has *args parameter (Python variadic positional)
    pub has_varargs: bool,
    /// Has **kwargs parameter (Python variadic keyword)
    pub has_kwargs: bool,
    /// How a value derived from storage is exposed to Python.
    pub result_classification: GetterResultClassification,
    /// Whether the implementation reads from its receiver's stored value.
    pub result_derived_from_self: bool,
}

/// Parameter definition
#[derive(Debug, Clone, Default)]
pub struct ParameterDef {
    /// Parameter name
    pub name: String,
    /// Parameter type
    pub param_type: Option<String>,
    /// Default value (as string)
    pub default_value: Option<String>,
    /// Is optional (Option<T>)
    pub is_optional: bool,
    /// Python calling convention for this parameter.
    pub kind: ParameterKind,
}

/// Python parameter calling convention.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ParameterKind {
    PositionalOnly,
    #[default]
    PositionalOrKeyword,
    KeywordOnly,
}

impl ParameterKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PositionalOnly => "positional_only",
            Self::PositionalOrKeyword => "positional_or_keyword",
            Self::KeywordOnly => "keyword_only",
        }
    }
}

/// Property definition (getter/setter pair)
#[derive(Debug, Clone, Default)]
pub struct PropertyDef {
    /// Property name
    pub name: String,
    /// Property type
    pub property_type: Option<String>,
    /// Has getter
    pub has_getter: bool,
    /// Has setter
    pub has_setter: bool,
    /// Getter self mutability
    pub getter_mutability: SelfMutability,
    /// Getter location
    pub getter_location: Option<SourceLocation>,
    /// Setter location
    pub setter_location: Option<SourceLocation>,
    /// Whether getter uses borrow_field pattern (returns borrowed reference, mutations persist)
    /// False means getter returns a cloned/owned value (mutations don't persist)
    pub getter_uses_borrow: bool,
    /// Explicit storage-result classification inferred from the getter body.
    pub getter_result_classification: GetterResultClassification,
}

/// How a mutable-looking result obtained from storage relates to its source.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GetterResultClassification {
    #[default]
    Unclassified,
    /// Resolver-backed or validity-bound storage; writes reach the source.
    Live,
    /// Storage machinery enforces a read-only snapshot.
    ReadOnlySnapshot,
    /// A deliberately independent computed value.
    ComputedOwned,
}

/// Class attribute definition
#[derive(Debug, Clone, Default)]
pub struct ClassAttrDef {
    /// Attribute name
    pub name: String,
    /// Attribute type
    pub attr_type: Option<String>,
    /// Source location
    pub location: Option<SourceLocation>,
}

/// Enum variant definition
#[derive(Debug, Clone)]
pub struct EnumVariantDef {
    /// Variant name
    pub name: String,
    /// Variant kind
    pub kind: EnumVariantKind,
    /// Explicit constructor declared by a nested Python variant class.
    pub constructor: Option<MethodDef>,
}

/// Enum variant kind
#[derive(Debug, Clone)]
pub enum EnumVariantKind {
    /// Unit variant: `Variant`
    Unit,
    /// Empty tuple variant: `Variant()`
    EmptyTuple,
    /// Tuple variant with fields: `Variant(T)`
    Tuple(Vec<String>),
    /// Struct variant: `Variant { field: T }`
    Struct(Vec<(String, String)>),
}

/// Self mutability in method signature
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SelfMutability {
    /// No self parameter
    #[default]
    None,
    /// &self
    Ref,
    /// &mut self
    RefMut,
    /// self (owned)
    Owned,
}

/// Custom macro information
#[derive(Debug, Clone)]
pub enum MacroInfo {
    /// #[pyenum(Type, options)]
    BevyEnum {
        bevy_type: String,
        empty_tuple: bool,
        /// The adapter implements conversions and Python variant registration by hand.
        manual: bool,
        /// The enum declaration emits a generated struct-backed variant hierarchy.
        message: bool,
        /// The enum declaration emits a generated PyComponent-backed hierarchy.
        component: bool,
        /// The enum declaration emits a generated PyResource-backed hierarchy.
        resource: bool,
    },
}

/// Information extracted from a `#[pycomponent(..., bridge)]` attribute.
/// Stored on the PyClassDef for the corresponding Py* type.
#[derive(Debug, Clone, Default)]
pub struct ComponentBridgeInfo {
    /// Bevy type named by the storage macro's first argument.
    pub bevy_type: String,
    /// Storage macro that declared this bridge.
    pub storage_kind: BridgeStorageKind,
    /// Fields accessible via both View API and from_numpy batch spawning
    pub view_fields: Vec<String>,
    /// Fields accessible only via from_numpy batch spawning (not View API)
    pub batch_only_fields: Vec<String>,
    /// Fields accessible only via View API (not batch/from_numpy)
    pub view_only_fields: Vec<String>,
    /// Whether this component is read-only (no_insert)
    pub no_insert: bool,
    /// Whether reflection registration is intentionally disabled.
    pub no_reflect: bool,
    /// Source location of the bridge declaration
    pub location: Option<SourceLocation>,
}

/// Storage macro kind that declared a bridge.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BridgeStorageKind {
    #[default]
    Component,
    Resource,
    Asset,
    Newtype,
}

/// A catch-all arm in a bevy -> Python conversion that yields a concrete value
#[derive(Debug, Clone)]
pub struct SilentFallbackInfo {
    /// Bevy type being converted from
    pub source_type: String,
    /// Where the catch-all arm sits
    pub location: Option<SourceLocation>,
}
