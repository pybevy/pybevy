use crate::model::SourceLocation;

/// Diagnostic severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

/// Diagnostic code for categorizing issues
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticCode {
    // Critical errors (E-codes)
    /// E001: Class in Rust not found in .pyi
    E001,
    /// E002: Method in Rust not found in .pyi
    E002,
    /// E003: Method in .pyi not found in Rust
    E003,
    /// E004: Constructor parameter name mismatch
    E004,
    /// E005: Constructor parameter order mismatch
    E005,
    /// E006: Return type incompatible
    E006,
    /// E007: Missing property in .pyi
    E007,
    /// E008: Constructor signature mismatch
    E008,
    /// E009: batch stub fields don't match bridge view_fields
    E009,
    /// E010: a configured validation entry no longer matches anything, either
    /// an exception matching no diagnostic or a declaration matching no class
    E010,
    /// E012: configured entry no longer matches any upstream Bevy or PyBevy item
    E012,
    /// E013: in-scope constructor has no resolved audited origin/pinned source
    E013,
    /// E014: constructor parameter roles violate the origin's calling policy
    E014,
    /// E015: convenience default precedes a required native constructor input
    E015,
    /// E016: explicitly non-constructible wrapper regained a public initializer
    E016,
    /// E017: pyenum stub declares a Python enum-family base
    E017,

    // Warnings (W-codes)
    /// W001: Complex type should return Py<T>
    W001,
    /// W002: Getter using &mut self
    W002,
    /// W003: Setter missing set_ prefix
    W003,
    /// W004: Factory method not returning PyResult<Py<Self>>
    W004,
    /// W005: Missing Option wrapper for default None
    W005,
    /// W006: Storage, message, or enum payload result is mutable without explicit semantics
    W006,
    /// W007: Property has getter but no setter (read-only)
    W007,
    /// W008: Enum has both variant and classattr alias (redundant)
    W008,
    /// W009: Component has eligible fields not exposed in view_fields/batch_only_fields
    W009,
    /// W010: Builder returning Self takes a mutable receiver (mutates the caller's value)
    W010,
    /// W012: an excluded Bevy type is implemented in the matching PyBevy module
    W012,
    /// W011: bevy -> Python conversion substitutes a value for unmapped variants
    W011,
    /// W013: value enum declared as plain pyclass instead of #[pyenum]
    W013,
    /// W014: shared-borrow proxy type takes a mutable borrow of itself
    W014,
    /// W015: payload-free immutable value enum omits upstream-supported hashing
    W015,
    /// W016: pyclass declares no `module = "pybevy...."`, so `__module__` is
    /// `builtins`
    W016,
    /// W017: field-derived constructor parameter accepts `None` but the Bevy
    /// field is not an `Option`
    W017,
    /// W018: stub spells a builtin tuple/list/number where the Bevy field or
    /// payload type has a PyBevy wrapper
    W018,
    /// W019: stub spells a builtin tuple/list/number where the Bevy field or
    /// payload type is a Bevy struct or enum that PyBevy does not wrap
    W019,
    /// W020: public value class copies a Bevy struct's fields instead of
    /// holding storage or using a generated wrapper
    W020,
    /// W021: wrapper declares Python hashing but the resolved upstream type
    /// does not implement Hash
    W021,
    /// W022: stub declares a writable attribute where the Rust runtime is
    /// read-only (frozen class or no corresponding setter)
    W022,
    /// W023: nested PyBevy wrapper field exposed through a plain struct-field
    /// getter, which detaches a clone in borrowed (component) mode
    W023,
    /// W024: non-Copy upstream associated constant exposed as a ClassVar /
    /// class attribute instead of a factory staticmethod
    W024,
    /// W025: wrapper explicitly claims a Bevy type that cannot be resolved in
    /// the pinned Bevy (missing, or ambiguous without a deciding module or
    /// explicit mapping)
    W025,
    /// W026: upstream tuple-payload enum variant exposed with named fields;
    /// tuple payloads must stay positional
    W026,
    /// W029: wrapper's or native system set's Python registration module
    /// (`module = "pybevy.X"`, `register_native_system_set!(.., module = "X")`)
    /// differs from the canonical Bevy placement derived from
    /// `[bevy.crate_mappings]`
    W029,
    /// W030: pybevy.prelude diverges from the pinned Bevy prelude (missing
    /// implemented names, or unreviewed PyBevy-specific additions)
    W030,
    /// W031: enum exposes a PyBevy variant name absent from the resolved
    /// upstream Bevy enum and not explained by a configured rename
    W031,
    /// W032: GC-visible pyclass holds Python references in struct fields
    /// but declares no `__traverse__`
    W032,
    /// W033: stub `__match_args__` does not match the variant's payload fields
    W033,
    /// W034: stub constructor default disagrees with the Rust `#[new]` default
    W034,
    /// W035: stub `Handle[T]` argument names a different asset type than the
    /// upstream field/parameter
    W035,
    /// W036: optionality drift: a stub advertises/omits `None` acceptance
    /// (constructor parameter or property) that the Rust signature or the
    /// upstream field does not support
    W036,
    /// W037: Bevy type implements `Default` but the Python constructor has
    /// required inputs, so `Type()` cannot build the default value
    W037,
    /// W038: wrapper exposes a method or property that the resolved Bevy type
    /// does not have
    W038,
    /// W039: method return type differs from the resolved Bevy method
    W039,
    /// W040: Bevy enum wrapped as an ordinary class instead of an enum adapter
    W040,
    /// W041: wrapper lacks a method of a configured contract trait the Bevy
    /// type implements
    W041,

    // Test coverage info (T-codes)
    /// T001: Class has no test coverage
    T001,
    /// T002: Constructor not tested
    T002,
    /// T003: Member not tested
    T003,
}

impl DiagnosticCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticCode::E001 => "E001",
            DiagnosticCode::E002 => "E002",
            DiagnosticCode::E003 => "E003",
            DiagnosticCode::E004 => "E004",
            DiagnosticCode::E005 => "E005",
            DiagnosticCode::E006 => "E006",
            DiagnosticCode::E007 => "E007",
            DiagnosticCode::E008 => "E008",
            DiagnosticCode::E009 => "E009",
            DiagnosticCode::E010 => "E010",
            DiagnosticCode::E012 => "E012",
            DiagnosticCode::E013 => "E013",
            DiagnosticCode::E014 => "E014",
            DiagnosticCode::E015 => "E015",
            DiagnosticCode::E016 => "E016",
            DiagnosticCode::E017 => "E017",
            DiagnosticCode::W001 => "W001",
            DiagnosticCode::W002 => "W002",
            DiagnosticCode::W003 => "W003",
            DiagnosticCode::W004 => "W004",
            DiagnosticCode::W005 => "W005",
            DiagnosticCode::W006 => "W006",
            DiagnosticCode::W007 => "W007",
            DiagnosticCode::W008 => "W008",
            DiagnosticCode::W009 => "W009",
            DiagnosticCode::W010 => "W010",
            DiagnosticCode::W011 => "W011",
            DiagnosticCode::W012 => "W012",
            DiagnosticCode::W013 => "W013",
            DiagnosticCode::W014 => "W014",
            DiagnosticCode::W015 => "W015",
            DiagnosticCode::W016 => "W016",
            DiagnosticCode::W017 => "W017",
            DiagnosticCode::W018 => "W018",
            DiagnosticCode::W019 => "W019",
            DiagnosticCode::W020 => "W020",
            DiagnosticCode::W021 => "W021",
            DiagnosticCode::W022 => "W022",
            DiagnosticCode::W023 => "W023",
            DiagnosticCode::W024 => "W024",
            DiagnosticCode::W025 => "W025",
            DiagnosticCode::W026 => "W026",
            DiagnosticCode::W029 => "W029",
            DiagnosticCode::W030 => "W030",
            DiagnosticCode::W031 => "W031",
            DiagnosticCode::W032 => "W032",
            DiagnosticCode::W033 => "W033",
            DiagnosticCode::W034 => "W034",
            DiagnosticCode::W035 => "W035",
            DiagnosticCode::W036 => "W036",
            DiagnosticCode::W037 => "W037",
            DiagnosticCode::W038 => "W038",
            DiagnosticCode::W039 => "W039",
            DiagnosticCode::W040 => "W040",
            DiagnosticCode::W041 => "W041",
            DiagnosticCode::T001 => "T001",
            DiagnosticCode::T002 => "T002",
            DiagnosticCode::T003 => "T003",
        }
    }

    pub fn severity(&self) -> DiagnosticSeverity {
        match self {
            DiagnosticCode::E001
            | DiagnosticCode::E002
            | DiagnosticCode::E003
            | DiagnosticCode::E004
            | DiagnosticCode::E005
            | DiagnosticCode::E006
            | DiagnosticCode::E007
            | DiagnosticCode::E008
            | DiagnosticCode::E009
            | DiagnosticCode::E010 => DiagnosticSeverity::Error,
            DiagnosticCode::E012 => DiagnosticSeverity::Error,
            DiagnosticCode::E013
            | DiagnosticCode::E014
            | DiagnosticCode::E015
            | DiagnosticCode::E016
            | DiagnosticCode::E017 => DiagnosticSeverity::Error,

            DiagnosticCode::W001
            | DiagnosticCode::W002
            | DiagnosticCode::W003
            | DiagnosticCode::W004
            | DiagnosticCode::W005
            | DiagnosticCode::W006
            | DiagnosticCode::W007
            | DiagnosticCode::W008
            | DiagnosticCode::W009
            | DiagnosticCode::W010
            | DiagnosticCode::W011
            | DiagnosticCode::W012
            | DiagnosticCode::W013
            | DiagnosticCode::W014
            | DiagnosticCode::W015
            | DiagnosticCode::W016
            | DiagnosticCode::W017
            | DiagnosticCode::W018
            | DiagnosticCode::W019
            | DiagnosticCode::W020
            | DiagnosticCode::W021
            | DiagnosticCode::W022
            | DiagnosticCode::W023
            | DiagnosticCode::W024
            | DiagnosticCode::W025
            | DiagnosticCode::W026
            | DiagnosticCode::W029
            | DiagnosticCode::W030
            | DiagnosticCode::W031
            | DiagnosticCode::W032
            | DiagnosticCode::W033
            | DiagnosticCode::W034
            | DiagnosticCode::W035
            | DiagnosticCode::W036
            | DiagnosticCode::W037
            | DiagnosticCode::W038
            | DiagnosticCode::W039
            | DiagnosticCode::W040
            | DiagnosticCode::W041 => DiagnosticSeverity::Warning,

            DiagnosticCode::T001 | DiagnosticCode::T002 | DiagnosticCode::T003 => {
                DiagnosticSeverity::Info
            }
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            DiagnosticCode::E001 => "class in Rust not found in Python stub",
            DiagnosticCode::E002 => "method in Rust not found in Python stub",
            DiagnosticCode::E003 => "method in Python stub not found in Rust",
            DiagnosticCode::E004 => "constructor parameter name mismatch",
            DiagnosticCode::E005 => "constructor parameter order mismatch",
            DiagnosticCode::E006 => "return type incompatible",
            DiagnosticCode::E007 => "missing property in Python stub",
            DiagnosticCode::E008 => "constructor signature mismatch",
            DiagnosticCode::E009 => "batch stub fields don't match bridge view_fields",
            DiagnosticCode::E010 => "stale or invalid validation exception",
            DiagnosticCode::E012 => "stale linter configuration entry",
            DiagnosticCode::E013 => "constructor has no resolved audited origin",
            DiagnosticCode::E014 => {
                "constructor parameter roles violate the origin's calling policy"
            }
            DiagnosticCode::E015 => {
                "convenience default precedes a required native constructor input"
            }
            DiagnosticCode::E016 => "non-constructible wrapper regained a public initializer",
            DiagnosticCode::E017 => "pyenum stub declares a Python enum-family base",
            DiagnosticCode::W001 => "complex type should return Py<T>",
            DiagnosticCode::W002 => "getter using &mut self instead of &self",
            DiagnosticCode::W003 => "setter missing set_ prefix",
            DiagnosticCode::W004 => "factory method should return PyResult<Py<Self>>",
            DiagnosticCode::W005 => "parameter with None default should be Option<T>",
            DiagnosticCode::W006 => {
                "storage, message, or enum payload mutable result has no mutation semantics"
            }
            DiagnosticCode::W007 => "property has getter but no setter (read-only)",
            DiagnosticCode::W008 => {
                "enum has both variant and classattr alias (redundant constructor)"
            }
            DiagnosticCode::W009 => {
                "component has eligible fields not exposed in view_fields/batch_only_fields"
            }
            DiagnosticCode::W010 => {
                "builder returning Self takes a mutable receiver (mutates the caller's value)"
            }
            DiagnosticCode::W011 => "conversion substitutes a value for unmapped bevy variants",
            DiagnosticCode::W013 => "value enum uses plain pyclass instead of pyenum",
            DiagnosticCode::W012 => "excluded Bevy type is implemented in PyBevy",
            DiagnosticCode::W014 => {
                "shared-borrow proxy takes a mutable borrow of itself (re-entrant borrow panics)"
            }
            DiagnosticCode::W015 => {
                "payload-free immutable value enum omits upstream-supported hashing"
            }
            DiagnosticCode::W016 => "pyclass declares no module, so __module__ is builtins",
            DiagnosticCode::W017 => {
                "field-derived constructor parameter accepts None but the Bevy field is not optional"
            }
            DiagnosticCode::W018 => {
                "stub uses a tuple/builtin where the Bevy field or payload type has a PyBevy wrapper"
            }
            DiagnosticCode::W019 => {
                "stub uses a tuple/builtin where the Bevy field or payload type is an unwrapped Bevy struct or enum"
            }
            DiagnosticCode::W020 => {
                "value class hand-mirrors Bevy struct fields instead of using storage or a generated wrapper"
            }
            DiagnosticCode::W021 => {
                "wrapper declares Python hashing but the upstream type does not implement Hash"
            }
            DiagnosticCode::W022 => {
                "stub declares a writable attribute where the Rust runtime is read-only"
            }
            DiagnosticCode::W023 => {
                "nested PyBevy wrapper field uses a plain field getter, detaching a clone in borrowed mode"
            }
            DiagnosticCode::W024 => {
                "non-Copy upstream constant exposed as a class attribute instead of a factory"
            }
            DiagnosticCode::W025 => {
                "wrapper claims a Bevy type that cannot be resolved in the pinned Bevy"
            }
            DiagnosticCode::W026 => {
                "upstream tuple-payload enum variant exposed with named fields instead of positionally"
            }
            DiagnosticCode::W029 => {
                "Python registration module differs from the canonical Bevy placement"
            }
            DiagnosticCode::W030 => "pybevy.prelude diverges from the pinned Bevy prelude",
            DiagnosticCode::W031 => {
                "enum exposes a variant name absent from the upstream Bevy enum"
            }
            DiagnosticCode::W032 => "pyclass holds Python references without __traverse__",
            DiagnosticCode::W033 => "__match_args__ does not match the variant payload fields",
            DiagnosticCode::W034 => "stub constructor default disagrees with the Rust default",
            DiagnosticCode::W035 => "stub Handle argument does not match the upstream field type",
            DiagnosticCode::W036 => {
                "stub None acceptance does not match the Rust signature or upstream field"
            }
            DiagnosticCode::W037 => "Bevy Default type cannot be constructed without arguments",
            DiagnosticCode::W038 => "wrapper exposes a member the Bevy type does not have",
            DiagnosticCode::W039 => "method return type differs from Bevy",
            DiagnosticCode::W040 => "Bevy enum wrapped as an ordinary class",
            DiagnosticCode::W041 => "wrapper lacks a Bevy contract trait method",
            DiagnosticCode::T001 => "class has no test coverage",
            DiagnosticCode::T002 => "constructor not tested",
            DiagnosticCode::T003 => "member not tested",
        }
    }
}

/// A diagnostic message
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: DiagnosticCode,
    pub message: String,
    pub primary_location: Option<SourceLocation>,
    pub secondary_locations: Vec<(SourceLocation, String)>,
    pub notes: Vec<String>,
    pub suggestions: Vec<Suggestion>,
}

impl Diagnostic {
    pub fn error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Error,
            code,
            message: message.into(),
            primary_location: None,
            secondary_locations: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn warning(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Warning,
            code,
            message: message.into(),
            primary_location: None,
            secondary_locations: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn info(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: DiagnosticSeverity::Info,
            code,
            message: message.into(),
            primary_location: None,
            secondary_locations: Vec::new(),
            notes: Vec::new(),
            suggestions: Vec::new(),
        }
    }

    pub fn with_location(mut self, location: SourceLocation) -> Self {
        self.primary_location = Some(location);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: Suggestion) -> Self {
        self.suggestions.push(suggestion);
        self
    }

    /// Check if this diagnostic is an error
    pub fn is_error(&self) -> bool {
        self.severity == DiagnosticSeverity::Error
    }

    /// Check if this diagnostic is a warning
    pub fn is_warning(&self) -> bool {
        self.severity == DiagnosticSeverity::Warning
    }
}

/// A suggestion for fixing a diagnostic
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub message: String,
    pub replacement: Option<String>,
}

impl Suggestion {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            replacement: None,
        }
    }

    pub fn with_replacement(mut self, replacement: impl Into<String>) -> Self {
        self.replacement = Some(replacement.into());
        self
    }
}
