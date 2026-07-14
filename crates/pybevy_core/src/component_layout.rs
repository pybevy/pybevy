use std::ffi::c_void;

use bevy::math::{Vec2, Vec3};

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

/// A component-field value already coerced from Python to a plain Rust value, ready
/// to write into wrapper bytes. One variant per [`PrimitiveType`].
///
/// Backend-agnostic: both backends extract their interpreter's value into this enum
/// (PyO3 via `.extract()`, RustPython via `try_*`) and then call [`Self::write_to_ptr`].
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

    #[test]
    fn primitive_value_write_to_ptr_roundtrips() {
        // Each variant writes exactly its bytes at the destination and reads back equal.
        let mut buf = [0u8; 16];
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
            buf = [0u8; 16];
            let dst = buf.as_mut_ptr();
            // SAFETY: buf is 16 bytes, larger than any primitive's size.
            unsafe { v.write_to_ptr(dst) };
            let read_back = unsafe {
                match v {
                    PrimitiveValue::F32(_) => {
                        PrimitiveValue::F32((dst as *const f32).read_unaligned())
                    }
                    PrimitiveValue::F64(_) => {
                        PrimitiveValue::F64((dst as *const f64).read_unaligned())
                    }
                    PrimitiveValue::I32(_) => {
                        PrimitiveValue::I32((dst as *const i32).read_unaligned())
                    }
                    PrimitiveValue::I64(_) => {
                        PrimitiveValue::I64((dst as *const i64).read_unaligned())
                    }
                    PrimitiveValue::U32(_) => {
                        PrimitiveValue::U32((dst as *const u32).read_unaligned())
                    }
                    PrimitiveValue::U64(_) => {
                        PrimitiveValue::U64((dst as *const u64).read_unaligned())
                    }
                    PrimitiveValue::Bool(_) => PrimitiveValue::Bool(*dst != 0),
                    PrimitiveValue::Vec3(_) => {
                        let x = (dst as *const f32).read_unaligned();
                        let y = (dst as *const f32).add(1).read_unaligned();
                        let z = (dst as *const f32).add(2).read_unaligned();
                        PrimitiveValue::Vec3(Vec3::new(x, y, z))
                    }
                    PrimitiveValue::Vec2(_) => {
                        let x = (dst as *const f32).read_unaligned();
                        let y = (dst as *const f32).add(1).read_unaligned();
                        PrimitiveValue::Vec2(Vec2::new(x, y))
                    }
                }
            };
            assert_eq!(&read_back, v);
        }
    }

    #[test]
    fn primitive_value_write_to_ptr_is_offset_unaligned_safe() {
        // Write at an odd (unaligned) offset: write_unaligned must not UB or corrupt.
        let mut buf = [0u8; 16];
        // SAFETY: writing an f64 at offset 1 stays within the 16-byte buffer.
        unsafe { PrimitiveValue::F64(6.5).write_to_ptr(buf.as_mut_ptr().add(1)) };
        let got = unsafe { (buf.as_ptr().add(1) as *const f64).read_unaligned() };
        assert_eq!(got, 6.5);
    }
}
