use std::ffi::c_void;

use super::component_wrapper::WrapperSize;

/// Primitive types that can be stored in wrapper components.
///
/// Backend-agnostic: both PyO3 and RustPython backends use this enum
/// to describe field types in custom components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    F32,
    F64,
    I32,
    I64,
    U32,
    U64,
    Bool,
    Vec3,
    Vec2,
}

impl PrimitiveType {
    /// Size in bytes of this primitive type
    pub const fn size_bytes(&self) -> usize {
        match self {
            PrimitiveType::F32 => 4,
            PrimitiveType::F64 => 8,
            PrimitiveType::I32 => 4,
            PrimitiveType::I64 => 8,
            PrimitiveType::U32 => 4,
            PrimitiveType::U64 => 8,
            PrimitiveType::Bool => 1,
            PrimitiveType::Vec3 => 12, // 3 × f32
            PrimitiveType::Vec2 => 8,  // 2 × f32
        }
    }

    /// Alignment requirement of this primitive type
    pub const fn alignment(&self) -> usize {
        match self {
            PrimitiveType::F32 => 4,
            PrimitiveType::F64 => 8,
            PrimitiveType::I32 => 4,
            PrimitiveType::I64 => 8,
            PrimitiveType::U32 => 4,
            PrimitiveType::U64 => 8,
            PrimitiveType::Bool => 1,
            PrimitiveType::Vec3 => 4, // f32 alignment
            PrimitiveType::Vec2 => 4, // f32 alignment
        }
    }

    /// Whether this type is a composite (multi-element) type like Vec3/Vec2
    pub const fn is_composite(&self) -> bool {
        matches!(self, PrimitiveType::Vec3 | PrimitiveType::Vec2)
    }

    /// Number of elements for composite types (1 for scalars)
    pub const fn element_count(&self) -> usize {
        match self {
            PrimitiveType::Vec3 => 3,
            PrimitiveType::Vec2 => 2,
            _ => 1,
        }
    }
}

/// Information about a single field in a component
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub offset: usize,
    pub field_type: PrimitiveType,
}

/// Layout metadata for a wrapper component.
///
/// Backend-agnostic: describes the memory layout of a custom component's fields.
/// Each backend (PyO3, RustPython) provides its own constructor to build this
/// from Python class metadata.
#[derive(Debug, Clone)]
pub struct ComponentLayout {
    /// Pointer to the Python type object (for type identity)
    pub py_type_ptr: *const c_void,
    /// Component type name
    pub name: String,
    /// Field information
    pub fields: Vec<FieldInfo>,
    /// Total size of the data (may be less than wrapper_size)
    pub data_size: usize,
    /// Wrapper size used to store this component
    pub wrapper_size: WrapperSize,
}

// SAFETY: py_type_ptr is an opaque identity key, never dereferenced; type
// objects live for the interpreter lifetime
unsafe impl Send for ComponentLayout {}
// SAFETY: see the Send impl above
unsafe impl Sync for ComponentLayout {}

impl ComponentLayout {
    /// Create a new ComponentLayout from pre-computed field information.
    pub fn new(
        py_type_ptr: *const c_void,
        name: String,
        fields: Vec<FieldInfo>,
        data_size: usize,
        wrapper_size: WrapperSize,
    ) -> Self {
        Self {
            py_type_ptr,
            name,
            fields,
            data_size,
            wrapper_size,
        }
    }

    /// Compute layout from field names and types.
    /// Returns `Err(data_size)` if the data is too large for any wrapper size,
    /// so callers can report the offending byte count.
    pub fn from_fields(
        py_type_ptr: *const c_void,
        name: String,
        field_types: &[(String, PrimitiveType)],
    ) -> Result<Self, usize> {
        let mut fields = Vec::new();
        let mut current_offset = 0usize;

        for (field_name, prim_type) in field_types {
            let alignment = prim_type.alignment();
            current_offset = current_offset.div_ceil(alignment) * alignment;

            fields.push(FieldInfo {
                name: field_name.clone(),
                offset: current_offset,
                field_type: *prim_type,
            });

            current_offset += prim_type.size_bytes();
        }

        let data_size = current_offset;
        let wrapper_size = WrapperSize::for_size(data_size).ok_or(data_size)?;

        Ok(Self {
            py_type_ptr,
            name,
            fields,
            data_size,
            wrapper_size,
        })
    }

    /// Get field information by name
    pub fn get_field(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.iter().find(|f| f.name == name)
    }

    /// Get list of field names (for error messages)
    pub fn field_names(&self) -> Vec<&str> {
        self.fields.iter().map(|f| f.name.as_str()).collect()
    }
}

/// Storage type for a custom component
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentStorageType {
    /// Component uses wrapper storage (primitive-only fields)
    Wrapper(WrapperSize),
    /// Component uses PyAny storage (contains non-primitive types or opted out)
    PyObject,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_type_sizes() {
        assert_eq!(PrimitiveType::F32.size_bytes(), 4);
        assert_eq!(PrimitiveType::F64.size_bytes(), 8);
        assert_eq!(PrimitiveType::I32.size_bytes(), 4);
        assert_eq!(PrimitiveType::I64.size_bytes(), 8);
        assert_eq!(PrimitiveType::Bool.size_bytes(), 1);
        assert_eq!(PrimitiveType::Vec3.size_bytes(), 12);
        assert_eq!(PrimitiveType::Vec2.size_bytes(), 8);
    }

    #[test]
    fn test_primitive_type_alignments() {
        assert_eq!(PrimitiveType::F32.alignment(), 4);
        assert_eq!(PrimitiveType::F64.alignment(), 8);
        assert_eq!(PrimitiveType::I32.alignment(), 4);
        assert_eq!(PrimitiveType::I64.alignment(), 8);
        assert_eq!(PrimitiveType::Bool.alignment(), 1);
    }

    #[test]
    fn test_primitive_type_u32_u64() {
        assert_eq!(PrimitiveType::U32.size_bytes(), 4);
        assert_eq!(PrimitiveType::U64.size_bytes(), 8);
        assert_eq!(PrimitiveType::U32.alignment(), 4);
        assert_eq!(PrimitiveType::U64.alignment(), 8);
    }

    #[test]
    fn test_composite_types() {
        assert!(PrimitiveType::Vec3.is_composite());
        assert!(PrimitiveType::Vec2.is_composite());
        assert!(!PrimitiveType::F32.is_composite());
        assert!(!PrimitiveType::Bool.is_composite());

        assert_eq!(PrimitiveType::Vec3.element_count(), 3);
        assert_eq!(PrimitiveType::Vec2.element_count(), 2);
        assert_eq!(PrimitiveType::F32.element_count(), 1);
    }

    #[test]
    fn test_layout_from_fields() {
        use std::ptr;

        let fields = vec![
            ("x".to_string(), PrimitiveType::F64),
            ("y".to_string(), PrimitiveType::F64),
            ("count".to_string(), PrimitiveType::I64),
            ("active".to_string(), PrimitiveType::Bool),
        ];

        let layout =
            ComponentLayout::from_fields(ptr::null(), "TestComponent".to_string(), &fields)
                .unwrap();

        assert_eq!(layout.name, "TestComponent");
        assert_eq!(layout.fields.len(), 4);

        assert_eq!(layout.fields[0].name, "x");
        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(layout.fields[0].field_type, PrimitiveType::F64);

        assert_eq!(layout.fields[1].name, "y");
        assert_eq!(layout.fields[1].offset, 8);

        assert_eq!(layout.fields[2].name, "count");
        assert_eq!(layout.fields[2].offset, 16);

        assert_eq!(layout.fields[3].name, "active");
        assert_eq!(layout.fields[3].offset, 24);

        assert_eq!(layout.data_size, 25);
        assert_eq!(layout.wrapper_size, WrapperSize::W32);
    }

    #[test]
    fn test_layout_field_lookup() {
        use std::ptr;

        let fields = vec![
            ("x".to_string(), PrimitiveType::F64),
            ("y".to_string(), PrimitiveType::F64),
        ];

        let layout =
            ComponentLayout::from_fields(ptr::null(), "Test".to_string(), &fields).unwrap();

        assert!(layout.get_field("x").is_some());
        assert!(layout.get_field("y").is_some());
        assert!(layout.get_field("z").is_none());
        assert_eq!(layout.field_names(), vec!["x", "y"]);
    }

    #[test]
    fn test_layout_alignment_mixed_types() {
        use std::ptr;

        let fields = vec![
            ("active".to_string(), PrimitiveType::Bool),
            ("x".to_string(), PrimitiveType::F64),
        ];

        let layout =
            ComponentLayout::from_fields(ptr::null(), "Test".to_string(), &fields).unwrap();

        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(layout.fields[1].offset, 8); // aligned to 8 after 1 byte
        assert_eq!(layout.data_size, 16);
    }

    #[test]
    fn test_layout_too_large() {
        use std::ptr;

        // 129 i64 fields = 1032 bytes, exceeds 1024
        let fields: Vec<_> = (0..129)
            .map(|i| (format!("f{}", i), PrimitiveType::I64))
            .collect();

        let result = ComponentLayout::from_fields(ptr::null(), "Big".to_string(), &fields);
        // 129 i64 fields = 1032 bytes; the error carries the offending size.
        assert_eq!(result.unwrap_err(), 1032);
    }
}
