use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A parsed Bevy crate's public API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BevyCrate {
    pub name: String,
    pub items: HashMap<String, BevyItem>,
}

impl BevyCrate {
    /// Find the generic base type for a specialized type.
    /// E.g., for "Time<Virtual>", finds "Time<T>" if it exists.
    pub fn find_generic_base(&self, specialized_name: &str) -> Option<&BevyItem> {
        // Extract base name: "Time<Virtual>" -> "Time"
        let base_name = specialized_name.split('<').next()?;

        // Look for a generic base with matching base name
        self.items
            .values()
            .filter(|item| {
                item.is_generic_base
                    && item.name.split('<').next() == Some(base_name)
                    && item.name.contains('<')
            })
            // rustdoc may emit several lifetime-specialized forms. The primary
            // inherent impl is the one carrying the complete method set.
            .max_by_key(|item| item.methods.len())
    }

    /// Get all methods for a type, including inherited methods from generic bases.
    /// For "Time<Virtual>", this returns methods from both Time<Virtual> and Time<T>.
    pub fn get_all_methods(&self, type_name: &str) -> Vec<&BevyMethod> {
        let mut methods = Vec::new();

        if let Some(item) = self.items.get(type_name) {
            methods.extend(item.methods.iter());
        }

        // If this is a specialized type, also include methods from generic base
        if type_name.contains('<')
            && !type_name.contains("::")
            && let Some(generic_base) = self.find_generic_base(type_name)
        {
            methods.extend(generic_base.methods.iter());
        }

        methods
    }

    /// Get all methods for a BevyItem, including inherited methods from generic bases.
    /// This is used when you already have the item reference (e.g., in comparison).
    pub fn get_methods_for_item<'a>(&'a self, item: &'a BevyItem) -> Vec<&'a BevyMethod> {
        let mut methods: Vec<&BevyMethod> = item.methods.iter().collect();

        // If this is a specialized type, also include methods from generic base
        if item.name.contains('<') && !item.name.contains("::") {
            if let Some(generic_base) = self.find_generic_base(&item.name) {
                methods.extend(generic_base.methods.iter());
            }
        } else if !item.name.contains('<') {
            // cargo-public-api can emit a bare exported type (`AssetPath`) plus
            // its inherent impl under a lifetime-specialized item
            // (`AssetPath<'a>`). Treat that impl as the bare type's method set.
            if let Some(generic_base) = self.find_generic_base(&item.name) {
                methods.extend(generic_base.methods.iter());
            }
        }

        methods
    }
}

/// A public item from Bevy's API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BevyItem {
    /// Full path (e.g., "bevy_transform::components::Transform")
    pub full_path: String,

    /// Short name (e.g., "Transform")
    pub name: String,

    /// Module path (e.g., "bevy_transform::components")
    pub module: String,

    /// What kind of item this is
    pub kind: BevyItemKind,

    /// Methods (for structs/enums/traits)
    pub methods: Vec<BevyMethod>,

    /// Fields (for structs)
    pub fields: Vec<BevyField>,

    /// Enum variants (for enums)
    pub variants: Vec<BevyEnumVariant>,

    /// Trait implementations
    pub trait_impls: Vec<String>,

    /// Associated types
    pub associated_types: Vec<(String, String)>,

    /// Constants
    pub constants: Vec<(String, String)>,

    /// Whether this is a generic base type (e.g., Time<T>, Assets<A>)
    /// Generic bases have their methods inherited by specialized types
    #[serde(default)]
    pub is_generic_base: bool,
}

/// The kind of Bevy item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BevyItemKind {
    Struct,
    Enum,
    Trait,
    TypeAlias,
    Constant,
    Function,
    Module,
}

/// A method on a Bevy type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BevyMethod {
    /// Method name
    pub name: String,

    /// Full signature
    pub signature: String,

    /// Parameters (excluding self)
    pub parameters: Vec<BevyParameter>,

    /// Return type
    pub return_type: Option<String>,

    /// Whether it takes &self, &mut self, self, or is static
    pub self_kind: SelfKind,

    /// Is this a const fn
    pub is_const: bool,

    /// Is this an unsafe fn
    pub is_unsafe: bool,

    /// Is this an async fn
    pub is_async: bool,

    /// Which trait this method comes from (None for inherent methods)
    pub from_trait: Option<String>,
}

/// How `self` is passed to a method
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SelfKind {
    /// No self parameter (static/associated function)
    #[default]
    None,
    /// `self` (owned)
    Owned,
    /// `&self` (borrowed)
    Ref,
    /// `&mut self` (mutably borrowed)
    RefMut,
}

/// A parameter in a Bevy method
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BevyParameter {
    pub name: String,
    pub param_type: String,
}

/// A field in a Bevy struct
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BevyField {
    pub name: String,
    pub field_type: String,
    pub is_public: bool,
}

/// An enum variant in Bevy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BevyEnumVariant {
    pub name: String,
    pub kind: BevyVariantKind,
}

/// The kind of enum variant
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BevyVariantKind {
    /// Unit variant: `Variant` or `Variant = N`
    Unit,
    /// Tuple variant: `Variant(T)` or `Variant(T, U)`
    Tuple(Vec<String>),
    /// Struct variant: `Variant { field: T }`
    Struct(Vec<(String, String)>),
}

impl BevyItem {
    pub fn new(full_path: String, kind: BevyItemKind) -> Self {
        let parts: Vec<&str> = full_path.rsplitn(2, "::").collect();
        let name = parts.first().unwrap_or(&"").to_string();
        let module = parts.get(1).unwrap_or(&"").to_string();

        Self {
            full_path,
            name,
            module,
            kind,
            methods: Vec::new(),
            fields: Vec::new(),
            variants: Vec::new(),
            trait_impls: Vec::new(),
            associated_types: Vec::new(),
            constants: Vec::new(),
            is_generic_base: false,
        }
    }

    /// Create a new item marked as a generic base type
    pub fn new_generic_base(full_path: String, kind: BevyItemKind) -> Self {
        let mut item = Self::new(full_path, kind);
        item.is_generic_base = true;
        item
    }
}

impl std::fmt::Display for BevyItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BevyItemKind::Struct => write!(f, "struct"),
            BevyItemKind::Enum => write!(f, "enum"),
            BevyItemKind::Trait => write!(f, "trait"),
            BevyItemKind::TypeAlias => write!(f, "type"),
            BevyItemKind::Constant => write!(f, "const"),
            BevyItemKind::Function => write!(f, "fn"),
            BevyItemKind::Module => write!(f, "mod"),
        }
    }
}
