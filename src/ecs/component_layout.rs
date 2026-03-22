use std::ffi::c_void;

use bevy::math::{Vec2, Vec3};
use pybevy_bytecodevm::bytecode::FieldType;
use pybevy_math::{vec2::PyVec2, vec3::PyVec3};
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyDict, PyType},
};

use super::component_wrapper::WrapperSize;

/// Primitive types that can be stored in wrapper components
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

    /// Try to parse a Python type annotation into a PrimitiveType
    pub fn from_python_type(ty: &Bound<'_, PyAny>) -> PyResult<Option<Self>> {
        let ty_str_bound = ty.str()?;
        let ty_str = ty_str_bound.to_str()?;

        // Handle typing module types
        let ty_str = if ty_str.starts_with("<class '") && ty_str.ends_with("'>") {
            &ty_str[8..ty_str.len() - 2]
        } else {
            ty_str
        };

        match ty_str {
            "float" => Ok(Some(PrimitiveType::F64)), // Python float is f64
            "int" => Ok(Some(PrimitiveType::I64)),   // Python int is i64
            "bool" => Ok(Some(PrimitiveType::Bool)),
            // PyO3 classes show as "builtins.ClassName"
            "Vec3" | "builtins.Vec3" => Ok(Some(PrimitiveType::Vec3)),
            "Vec2" | "builtins.Vec2" => Ok(Some(PrimitiveType::Vec2)),
            _ => {
                // Check if it's a typing annotation that's not supported
                if ty_str.contains("typing.") || ty_str.contains("list") || ty_str.contains("dict")
                {
                    Ok(None)
                } else {
                    // Unknown type - might be a custom class
                    Ok(None)
                }
            }
        }
    }

    /// Convert to numpy dtype string
    pub fn to_numpy_dtype(&self) -> &'static str {
        match self {
            PrimitiveType::F32 => "f4",
            PrimitiveType::F64 => "f8",
            PrimitiveType::I32 => "i4",
            PrimitiveType::I64 => "i8",
            PrimitiveType::U32 => "u4",
            PrimitiveType::U64 => "u8",
            PrimitiveType::Bool => "u1",
            PrimitiveType::Vec3 => "f4", // individual element dtype (used with 2D arrays)
            PrimitiveType::Vec2 => "f4",
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

    /// Convert to FieldType for bytecode VM
    pub fn to_field_type(&self) -> FieldType {
        match self {
            PrimitiveType::F32 => FieldType::F32,
            PrimitiveType::F64 => FieldType::F64,
            PrimitiveType::I32 => FieldType::I32,
            PrimitiveType::I64 => FieldType::I64,
            PrimitiveType::U32 => FieldType::U32,
            PrimitiveType::U64 => FieldType::U64,
            PrimitiveType::Bool => FieldType::Bool,
            PrimitiveType::Vec3 => FieldType::Vec3,
            PrimitiveType::Vec2 => FieldType::Vec2,
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

/// Layout metadata for a wrapper component
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

unsafe impl Send for ComponentLayout {}
unsafe impl Sync for ComponentLayout {}

impl ComponentLayout {
    /// Compute layout from a Python class's __annotations__
    pub fn from_annotations(cls: &Bound<'_, PyType>) -> PyResult<Self> {
        let name = cls.name()?.to_string();

        // Get __annotations__ dict
        let annotations_bound = cls.getattr("__annotations__").map_err(|_| {
            PyTypeError::new_err(format!("Component class '{}' has no __annotations__", name))
        })?;
        let annotations = annotations_bound.cast::<PyDict>()?;

        if annotations.is_empty() {
            return Err(PyTypeError::new_err(format!(
                "Component class '{}' has no fields",
                name
            )));
        }

        // Parse each field annotation
        let mut fields = Vec::new();
        let mut current_offset = 0usize;

        for (field_name, field_type) in annotations.iter() {
            let field_name_bound = field_name.str()?;
            let field_name_str = field_name_bound.to_str()?.to_string();

            // Try to parse as primitive type
            let primitive_type = PrimitiveType::from_python_type(&field_type)?;

            match primitive_type {
                Some(prim_type) => {
                    // Align current offset to field's alignment requirement
                    let alignment = prim_type.alignment();
                    current_offset = (current_offset + alignment - 1) / alignment * alignment;

                    fields.push(FieldInfo {
                        name: field_name_str.clone(),
                        offset: current_offset,
                        field_type: prim_type,
                    });

                    current_offset += prim_type.size_bytes();
                }
                None => {
                    // Non-primitive type found - cannot use wrapper storage
                    let type_str_bound = field_type.str()?;
                    let type_str = type_str_bound.to_str()?;
                    return Err(PyTypeError::new_err(format!(
                        "Component '{}' field '{}' has non-primitive type '{}'. \
                         Only float, int, and bool are supported for wrapper storage.",
                        name, field_name_str, type_str
                    )));
                }
            }
        }

        let data_size = current_offset;

        // Select wrapper size
        let wrapper_size = WrapperSize::for_size(data_size).ok_or_else(|| {
            PyTypeError::new_err(format!(
                "Component '{}' is too large ({} bytes). \
                 Maximum size for wrapper storage is 1024 bytes.",
                name, data_size
            ))
        })?;

        Ok(ComponentLayout {
            py_type_ptr: cls.as_ptr() as *const c_void,
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

impl ComponentStorageType {
    /// Determine storage type from a Python class
    pub fn from_python_class(cls: &Bound<'_, PyType>) -> PyResult<Self> {
        // Check for explicit storage mode in class attributes
        if let Ok(storage_attr) = cls.getattr("__pybevy_storage__") {
            let storage_str_bound = storage_attr.str()?;
            let storage_str = storage_str_bound.to_str()?;
            match storage_str {
                "pyobject" => return Ok(ComponentStorageType::PyObject),
                "wrapper" => {
                    // Continue to layout analysis
                }
                _ => {
                    return Err(PyTypeError::new_err(format!(
                        "Invalid storage type '{}'. Use 'wrapper' or 'pyobject'.",
                        storage_str
                    )));
                }
            }
        }

        // Try to compute layout - if successful, use wrapper storage
        match ComponentLayout::from_annotations(cls) {
            Ok(layout) => Ok(ComponentStorageType::Wrapper(layout.wrapper_size)),
            Err(_) => {
                // Layout computation failed (non-primitive types) - use PyObject storage
                Ok(ComponentStorageType::PyObject)
            }
        }
    }
}

/// Serialize a Python object to wrapper bytes according to the layout
///
/// # Arguments
/// * `py` - Python GIL token
/// * `obj` - Python object to serialize
/// * `layout` - Component layout describing field offsets and types
///
/// # Returns
/// A Vec<u8> containing the serialized data
///
/// # Errors
/// Returns PyErr if serialization fails (e.g., field not found, wrong type)
pub fn serialize_to_wrapper(obj: &Bound<'_, PyAny>, layout: &ComponentLayout) -> PyResult<Vec<u8>> {
    let mut buffer = vec![0u8; layout.wrapper_size.size_bytes()];

    for field in &layout.fields {
        let value = obj.getattr(field.name.as_str())?;

        // Extract primitive value and write to buffer at the correct offset
        match field.field_type {
            PrimitiveType::F32 => {
                let val: f32 = value.extract()?;
                let bytes = val.to_le_bytes();
                buffer[field.offset..field.offset + 4].copy_from_slice(&bytes);
            }
            PrimitiveType::F64 => {
                let val: f64 = value.extract()?;
                let bytes = val.to_le_bytes();
                buffer[field.offset..field.offset + 8].copy_from_slice(&bytes);
            }
            PrimitiveType::I32 => {
                let val: i32 = value.extract()?;
                let bytes = val.to_le_bytes();
                buffer[field.offset..field.offset + 4].copy_from_slice(&bytes);
            }
            PrimitiveType::I64 => {
                let val: i64 = value.extract()?;
                let bytes = val.to_le_bytes();
                buffer[field.offset..field.offset + 8].copy_from_slice(&bytes);
            }
            PrimitiveType::U32 => {
                let val: u32 = value.extract()?;
                let bytes = val.to_le_bytes();
                buffer[field.offset..field.offset + 4].copy_from_slice(&bytes);
            }
            PrimitiveType::U64 => {
                let val: u64 = value.extract()?;
                let bytes = val.to_le_bytes();
                buffer[field.offset..field.offset + 8].copy_from_slice(&bytes);
            }
            PrimitiveType::Bool => {
                let val: bool = value.extract()?;
                buffer[field.offset] = if val { 1 } else { 0 };
            }
            PrimitiveType::Vec3 => {
                let py_vec3: PyRef<PyVec3> = value.extract()?;
                let v: Vec3 = (&*py_vec3).into();
                buffer[field.offset..field.offset + 4].copy_from_slice(&v.x.to_le_bytes());
                buffer[field.offset + 4..field.offset + 8].copy_from_slice(&v.y.to_le_bytes());
                buffer[field.offset + 8..field.offset + 12].copy_from_slice(&v.z.to_le_bytes());
            }
            PrimitiveType::Vec2 => {
                let py_vec2: PyRef<PyVec2> = value.extract()?;
                let v: Vec2 = (&*py_vec2).into();
                buffer[field.offset..field.offset + 4].copy_from_slice(&v.x.to_le_bytes());
                buffer[field.offset + 4..field.offset + 8].copy_from_slice(&v.y.to_le_bytes());
            }
        }
    }

    Ok(buffer)
}
