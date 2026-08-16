use std::{error::Error, ffi::c_void, fmt};

use bevy::math::{Vec2, Vec3};

use super::component_wrapper::WrapperSize;

/// Primitive types that can be stored in wrapper components.
///
/// Interpreter-neutral: adapters use this enum to describe field types in
/// custom components.
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
    /// Resolve the canonical annotation names accepted by both Python backends.
    pub fn from_annotation_name(name: &str) -> Option<Self> {
        match name {
            "float" => Some(Self::F64),
            "int" => Some(Self::I64),
            "bool" => Some(Self::Bool),
            "Vec3" | "builtins.Vec3" => Some(Self::Vec3),
            "Vec2" | "builtins.Vec2" => Some(Self::Vec2),
            _ => None,
        }
    }

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

/// A component-field value already coerced from Python to a plain Rust value, ready
/// to write into wrapper bytes. One variant per [`PrimitiveType`].
///
/// Interpreter-neutral: adapters extract their interpreter's value into this enum
/// and then call [`Self::write_to_ptr`].
/// Splitting extraction from the write lets a caller finish ALL Python interaction
/// before it resolves the destination pointer - extraction can re-enter Python
/// (`__float__`/`__index__`/`__bool__`) and structurally mutate the World, relocating
/// the component, so a pointer held across it would dangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PrimitiveValue {
    F32(f32),
    F64(f64),
    I32(i32),
    I64(i64),
    U32(u32),
    U64(u64),
    Bool(bool),
    Vec3(Vec3),
    Vec2(Vec2),
}

impl PrimitiveValue {
    /// The `PrimitiveType` this value corresponds to.
    pub const fn field_type(&self) -> PrimitiveType {
        match self {
            PrimitiveValue::F32(_) => PrimitiveType::F32,
            PrimitiveValue::F64(_) => PrimitiveType::F64,
            PrimitiveValue::I32(_) => PrimitiveType::I32,
            PrimitiveValue::I64(_) => PrimitiveType::I64,
            PrimitiveValue::U32(_) => PrimitiveType::U32,
            PrimitiveValue::U64(_) => PrimitiveType::U64,
            PrimitiveValue::Bool(_) => PrimitiveType::Bool,
            PrimitiveValue::Vec3(_) => PrimitiveType::Vec3,
            PrimitiveValue::Vec2(_) => PrimitiveType::Vec2,
        }
    }

    /// Write this value into wrapper bytes at `dst`. Does no Python interaction, so the
    /// caller may resolve `dst` immediately before the call. Vectors are written
    /// lane-by-lane; all writes are `write_unaligned` (matching the bytecode VM), so no
    /// alignment of `dst` is assumed even though the wrapper layout currently guarantees it.
    ///
    /// # Safety
    /// `dst` must point to a valid, writable region of at least
    /// `self.field_type().size_bytes()` bytes.
    #[inline]
    pub unsafe fn write_to_ptr(self, dst: *mut u8) {
        // SAFETY: the caller guarantees a writable region large enough for this value.
        unsafe {
            match self {
                PrimitiveValue::F32(v) => (dst as *mut f32).write_unaligned(v),
                PrimitiveValue::F64(v) => (dst as *mut f64).write_unaligned(v),
                PrimitiveValue::I32(v) => (dst as *mut i32).write_unaligned(v),
                PrimitiveValue::I64(v) => (dst as *mut i64).write_unaligned(v),
                PrimitiveValue::U32(v) => (dst as *mut u32).write_unaligned(v),
                PrimitiveValue::U64(v) => (dst as *mut u64).write_unaligned(v),
                PrimitiveValue::Bool(v) => dst.write_unaligned(v as u8),
                PrimitiveValue::Vec3(v) => {
                    (dst as *mut f32).write_unaligned(v.x);
                    (dst as *mut f32).add(1).write_unaligned(v.y);
                    (dst as *mut f32).add(2).write_unaligned(v.z);
                }
                PrimitiveValue::Vec2(v) => {
                    (dst as *mut f32).write_unaligned(v.x);
                    (dst as *mut f32).add(1).write_unaligned(v.y);
                }
            }
        }
    }

    /// Read one primitive value from wrapper bytes.
    ///
    /// # Safety
    /// `src` must point to a valid, readable region of at least
    /// `field_type.size_bytes()` bytes containing the representation used by
    /// wrapper storage.
    #[inline]
    pub unsafe fn read_from_ptr(field_type: PrimitiveType, src: *const u8) -> Self {
        // SAFETY: the caller guarantees a readable region large enough for the
        // requested primitive. Unaligned reads match wrapper serialization.
        unsafe {
            match field_type {
                PrimitiveType::F32 => Self::F32((src as *const f32).read_unaligned()),
                PrimitiveType::F64 => Self::F64((src as *const f64).read_unaligned()),
                PrimitiveType::I32 => Self::I32((src as *const i32).read_unaligned()),
                PrimitiveType::I64 => Self::I64((src as *const i64).read_unaligned()),
                PrimitiveType::U32 => Self::U32((src as *const u32).read_unaligned()),
                PrimitiveType::U64 => Self::U64((src as *const u64).read_unaligned()),
                PrimitiveType::Bool => Self::Bool(src.read_unaligned() != 0),
                PrimitiveType::Vec3 => Self::Vec3(Vec3::new(
                    (src as *const f32).read_unaligned(),
                    (src as *const f32).add(1).read_unaligned(),
                    (src as *const f32).add(2).read_unaligned(),
                )),
                PrimitiveType::Vec2 => Self::Vec2(Vec2::new(
                    (src as *const f32).read_unaligned(),
                    (src as *const f32).add(1).read_unaligned(),
                )),
            }
        }
    }

    fn write_to_slice(self, dst: &mut [u8]) {
        assert_eq!(dst.len(), self.field_type().size_bytes());
        match self {
            PrimitiveValue::F32(v) => dst.copy_from_slice(&v.to_le_bytes()),
            PrimitiveValue::F64(v) => dst.copy_from_slice(&v.to_le_bytes()),
            PrimitiveValue::I32(v) => dst.copy_from_slice(&v.to_le_bytes()),
            PrimitiveValue::I64(v) => dst.copy_from_slice(&v.to_le_bytes()),
            PrimitiveValue::U32(v) => dst.copy_from_slice(&v.to_le_bytes()),
            PrimitiveValue::U64(v) => dst.copy_from_slice(&v.to_le_bytes()),
            PrimitiveValue::Bool(v) => dst[0] = u8::from(v),
            PrimitiveValue::Vec3(v) => {
                dst[0..4].copy_from_slice(&v.x.to_le_bytes());
                dst[4..8].copy_from_slice(&v.y.to_le_bytes());
                dst[8..12].copy_from_slice(&v.z.to_le_bytes());
            }
            PrimitiveValue::Vec2(v) => {
                dst[0..4].copy_from_slice(&v.x.to_le_bytes());
                dst[4..8].copy_from_slice(&v.y.to_le_bytes());
            }
        }
    }
}

/// A neutral layout invariant rejected before wrapper bytes are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentLayoutError {
    MisalignedField {
        field: String,
        offset: usize,
        alignment: usize,
    },
    FieldOutOfBounds {
        field: String,
        offset: usize,
        size: usize,
        buffer_size: usize,
    },
    FieldTypeMismatch {
        field: String,
        expected: PrimitiveType,
        actual: PrimitiveType,
    },
    FieldCountMismatch {
        expected: usize,
        actual: usize,
    },
    FieldNameMismatch {
        index: usize,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for ComponentLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MisalignedField {
                field,
                offset,
                alignment,
            } => write!(
                f,
                "field '{field}' at offset {offset} is not aligned to {alignment} bytes"
            ),
            Self::FieldOutOfBounds {
                field,
                offset,
                size,
                buffer_size,
            } => write!(
                f,
                "field '{field}' at offset {offset} with size {size} exceeds wrapper buffer size {buffer_size}"
            ),
            Self::FieldTypeMismatch {
                field,
                expected,
                actual,
            } => write!(
                f,
                "field '{field}' extractor returned {actual:?}, expected {expected:?}"
            ),
            Self::FieldCountMismatch { expected, actual } => write!(
                f,
                "saved component has {actual} fields, registered schema expects {expected}"
            ),
            Self::FieldNameMismatch {
                index,
                expected,
                actual,
            } => write!(
                f,
                "saved field {index} is named '{actual}', registered schema expects '{expected}'"
            ),
        }
    }
}

impl Error for ComponentLayoutError {}

/// Serialization can fail in the backend extractor or in the neutral layout core.
#[derive(Debug)]
pub enum ComponentSerializationError<E> {
    Extract(E),
    Layout(ComponentLayoutError),
}

/// Information about a single field in a component
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    pub name: String,
    pub offset: usize,
    pub field_type: PrimitiveType,
}

/// Interpreter-neutral schema for a wrapper-stored custom component.
///
/// Unlike [`ComponentLayout`], this contains no interpreter object identity and
/// can therefore be retained by shared registries, serialization, and control
/// adapters across Python hot-reload generations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrapperComponentSchema {
    pub fields: Vec<FieldInfo>,
    pub data_size: usize,
    pub wrapper_size: WrapperSize,
}

impl WrapperComponentSchema {
    /// Get field information by name.
    pub fn get_field(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.iter().find(|field| field.name == name)
    }

    /// Get field names in their declared layout order.
    pub fn field_names(&self) -> Vec<&str> {
        self.fields
            .iter()
            .map(|field| field.name.as_str())
            .collect()
    }

    /// Read all declared values from a wrapper component.
    ///
    /// # Safety
    ///
    /// `data` must point to the data array of the wrapper selected by
    /// [`Self::wrapper_size`].
    pub unsafe fn read_values(
        &self,
        data: *const u8,
    ) -> Result<Vec<(String, PrimitiveValue)>, ComponentLayoutError> {
        let buffer_size = self.wrapper_size.size_bytes();
        let mut values = Vec::with_capacity(self.fields.len());
        for field in &self.fields {
            let size = field.field_type.size_bytes();
            let Some(end) = field.offset.checked_add(size) else {
                return Err(ComponentLayoutError::FieldOutOfBounds {
                    field: field.name.clone(),
                    offset: field.offset,
                    size,
                    buffer_size,
                });
            };
            if end > buffer_size {
                return Err(ComponentLayoutError::FieldOutOfBounds {
                    field: field.name.clone(),
                    offset: field.offset,
                    size,
                    buffer_size,
                });
            }
            // SAFETY: the checked range lies within the wrapper allocation and
            // the caller guarantees that `data` points at that allocation.
            let value =
                unsafe { PrimitiveValue::read_from_ptr(field.field_type, data.add(field.offset)) };
            values.push((field.name.clone(), value));
        }
        Ok(values)
    }

    /// Serialize an exact, ordered schema/value snapshot into wrapper bytes.
    pub fn serialize_values(
        &self,
        values: &[(String, PrimitiveValue)],
    ) -> Result<Vec<u8>, ComponentLayoutError> {
        if values.len() != self.fields.len() {
            return Err(ComponentLayoutError::FieldCountMismatch {
                expected: self.fields.len(),
                actual: values.len(),
            });
        }

        let mut buffer = vec![0u8; self.wrapper_size.size_bytes()];
        for (index, (field, (name, value))) in self.fields.iter().zip(values).enumerate() {
            if name != &field.name {
                return Err(ComponentLayoutError::FieldNameMismatch {
                    index,
                    expected: field.name.clone(),
                    actual: name.clone(),
                });
            }
            if value.field_type() != field.field_type {
                return Err(ComponentLayoutError::FieldTypeMismatch {
                    field: name.clone(),
                    expected: field.field_type,
                    actual: value.field_type(),
                });
            }
            let size = field.field_type.size_bytes();
            let Some(end) = field.offset.checked_add(size) else {
                return Err(ComponentLayoutError::FieldOutOfBounds {
                    field: field.name.clone(),
                    offset: field.offset,
                    size,
                    buffer_size: buffer.len(),
                });
            };
            let Some(dst) = buffer.get_mut(field.offset..end) else {
                return Err(ComponentLayoutError::FieldOutOfBounds {
                    field: field.name.clone(),
                    offset: field.offset,
                    size,
                    buffer_size: buffer.len(),
                });
            };
            value.write_to_slice(dst);
        }
        Ok(buffer)
    }
}

/// Layout metadata for a wrapper component.
///
/// Backend-agnostic: describes the memory layout of a custom component's fields.
/// Each interpreter adapter provides its own constructor to build this
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

    /// Drop interpreter identity and retain only the stable wrapper schema.
    pub fn schema(&self) -> WrapperComponentSchema {
        WrapperComponentSchema {
            fields: self.fields.clone(),
            data_size: self.data_size,
            wrapper_size: self.wrapper_size,
        }
    }

    /// Serialize backend-extracted field values into a wrapper-sized byte buffer.
    pub fn serialize_with<E>(
        &self,
        mut extract: impl FnMut(&FieldInfo) -> Result<PrimitiveValue, E>,
    ) -> Result<Vec<u8>, ComponentSerializationError<E>> {
        let mut buffer = vec![0u8; self.wrapper_size.size_bytes()];

        for field in &self.fields {
            let alignment = field.field_type.alignment();
            if field.offset % alignment != 0 {
                return Err(ComponentSerializationError::Layout(
                    ComponentLayoutError::MisalignedField {
                        field: field.name.clone(),
                        offset: field.offset,
                        alignment,
                    },
                ));
            }

            let size = field.field_type.size_bytes();
            let Some(end) = field.offset.checked_add(size) else {
                return Err(ComponentSerializationError::Layout(
                    ComponentLayoutError::FieldOutOfBounds {
                        field: field.name.clone(),
                        offset: field.offset,
                        size,
                        buffer_size: buffer.len(),
                    },
                ));
            };
            let Some(dst) = buffer.get_mut(field.offset..end) else {
                return Err(ComponentSerializationError::Layout(
                    ComponentLayoutError::FieldOutOfBounds {
                        field: field.name.clone(),
                        offset: field.offset,
                        size,
                        buffer_size: buffer.len(),
                    },
                ));
            };

            let value = extract(field).map_err(ComponentSerializationError::Extract)?;
            let actual = value.field_type();
            if actual != field.field_type {
                return Err(ComponentSerializationError::Layout(
                    ComponentLayoutError::FieldTypeMismatch {
                        field: field.name.clone(),
                        expected: field.field_type,
                        actual,
                    },
                ));
            }
            value.write_to_slice(dst);
        }

        Ok(buffer)
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
    use std::{convert::Infallible, ptr};

    use super::*;

    #[test]
    fn annotation_names_map_to_shared_primitive_types() {
        assert_eq!(
            PrimitiveType::from_annotation_name("float"),
            Some(PrimitiveType::F64)
        );
        assert_eq!(
            PrimitiveType::from_annotation_name("int"),
            Some(PrimitiveType::I64)
        );
        assert_eq!(
            PrimitiveType::from_annotation_name("bool"),
            Some(PrimitiveType::Bool)
        );
        assert_eq!(
            PrimitiveType::from_annotation_name("builtins.Vec3"),
            Some(PrimitiveType::Vec3)
        );
        assert_eq!(
            PrimitiveType::from_annotation_name("Vec2"),
            Some(PrimitiveType::Vec2)
        );
        assert_eq!(PrimitiveType::from_annotation_name("list"), None);
    }

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
    fn test_layout_alignment_composite_then_scalar() {
        let fields = vec![
            ("position".to_string(), PrimitiveType::Vec3),
            ("speed".to_string(), PrimitiveType::F64),
        ];

        let layout =
            ComponentLayout::from_fields(ptr::null(), "Test".to_string(), &fields).unwrap();

        assert_eq!(layout.fields[0].offset, 0);
        assert_eq!(layout.fields[1].offset, 16);
        assert_eq!(layout.data_size, 24);
        assert_eq!(layout.wrapper_size, WrapperSize::W32);
    }

    #[test]
    fn test_layout_too_large() {
        // 129 i64 fields = 1032 bytes, exceeds 1024
        let fields: Vec<_> = (0..129)
            .map(|i| (format!("f{}", i), PrimitiveType::I64))
            .collect();

        let result = ComponentLayout::from_fields(ptr::null(), "Big".to_string(), &fields);
        // 129 i64 fields = 1032 bytes; the error carries the offending size.
        assert_eq!(result.unwrap_err(), 1032);
    }

    #[test]
    fn primitive_value_write_to_ptr_roundtrips() {
        // Each variant writes exactly its bytes at the destination and reads back equal.
        let cases: &[PrimitiveValue] = &[
            PrimitiveValue::F32(1.5),
            PrimitiveValue::F64(-2.25),
            PrimitiveValue::I32(-7),
            PrimitiveValue::I64(1 << 40),
            PrimitiveValue::U32(4_000_000_000),
            PrimitiveValue::U64(u64::MAX),
            PrimitiveValue::Bool(true),
            PrimitiveValue::Vec3(Vec3::new(1.0, 2.0, 3.0)),
            PrimitiveValue::Vec2(Vec2::new(9.0, -8.0)),
        ];
        for v in cases {
            let mut buf = [0u8; 16];
            let dst = buf.as_mut_ptr();
            // SAFETY: buf is 16 bytes, larger than any primitive's size.
            unsafe { v.write_to_ptr(dst) };
            // SAFETY: buf is 16 bytes, larger than any primitive's size.
            let read_back = unsafe { PrimitiveValue::read_from_ptr(v.field_type(), dst) };
            assert_eq!(&read_back, v);
        }
    }

    #[test]
    fn primitive_value_write_to_ptr_is_offset_unaligned_safe() {
        // Write at an odd (unaligned) offset: write_unaligned must not UB or corrupt.
        let mut buf = [0u8; 16];
        // SAFETY: writing an f64 at offset 1 stays within the 16-byte buffer.
        unsafe { PrimitiveValue::F64(6.5).write_to_ptr(buf.as_mut_ptr().add(1)) };
        // SAFETY: reading an f64 at offset 1 stays within the 16-byte buffer.
        let got = unsafe { PrimitiveValue::read_from_ptr(PrimitiveType::F64, buf.as_ptr().add(1)) };
        assert_eq!(got, PrimitiveValue::F64(6.5));
    }

    #[test]
    fn serialize_with_writes_values_and_zeroes_padding() {
        let fields = vec![
            ("active".to_string(), PrimitiveType::Bool),
            ("count".to_string(), PrimitiveType::I64),
            ("velocity".to_string(), PrimitiveType::Vec2),
        ];
        let layout =
            ComponentLayout::from_fields(ptr::null(), "Mixed".to_string(), &fields).unwrap();

        let buffer = layout
            .serialize_with(|field| {
                Ok::<_, Infallible>(match field.name.as_str() {
                    "active" => PrimitiveValue::Bool(true),
                    "count" => PrimitiveValue::I64(-42),
                    "velocity" => PrimitiveValue::Vec2(Vec2::new(1.5, -2.5)),
                    _ => unreachable!(),
                })
            })
            .unwrap();

        assert_eq!(buffer.len(), WrapperSize::W32.size_bytes());
        assert_eq!(buffer[0], 1);
        assert_eq!(&buffer[1..8], &[0; 7]);
        assert_eq!(i64::from_le_bytes(buffer[8..16].try_into().unwrap()), -42);
        assert_eq!(f32::from_le_bytes(buffer[16..20].try_into().unwrap()), 1.5);
        assert_eq!(f32::from_le_bytes(buffer[20..24].try_into().unwrap()), -2.5);
        assert_eq!(&buffer[24..], &[0; 8]);
    }

    #[test]
    fn serialize_with_preserves_backend_extraction_errors() {
        let fields = vec![("value".to_string(), PrimitiveType::I64)];
        let layout =
            ComponentLayout::from_fields(ptr::null(), "Test".to_string(), &fields).unwrap();

        let error = layout
            .serialize_with(|_| Err::<PrimitiveValue, _>("extract failed"))
            .unwrap_err();

        assert!(matches!(
            error,
            ComponentSerializationError::Extract("extract failed")
        ));
    }

    #[test]
    fn serialize_with_rejects_mismatched_extractor_types() {
        let fields = vec![("value".to_string(), PrimitiveType::I64)];
        let layout =
            ComponentLayout::from_fields(ptr::null(), "Test".to_string(), &fields).unwrap();

        let error = layout
            .serialize_with(|_| Ok::<_, Infallible>(PrimitiveValue::F64(1.0)))
            .unwrap_err();

        assert!(matches!(
            error,
            ComponentSerializationError::Layout(ComponentLayoutError::FieldTypeMismatch {
                expected: PrimitiveType::I64,
                actual: PrimitiveType::F64,
                ..
            })
        ));
    }

    #[test]
    fn serialize_with_rejects_invalid_layout_bounds() {
        let layout = ComponentLayout::new(
            ptr::null(),
            "Invalid".to_string(),
            vec![FieldInfo {
                name: "value".to_string(),
                offset: 8,
                field_type: PrimitiveType::I64,
            }],
            16,
            WrapperSize::W8,
        );

        let error = layout
            .serialize_with(|_| Ok::<_, Infallible>(PrimitiveValue::I64(1)))
            .unwrap_err();

        assert!(matches!(
            error,
            ComponentSerializationError::Layout(ComponentLayoutError::FieldOutOfBounds {
                buffer_size: 8,
                ..
            })
        ));
    }
}
