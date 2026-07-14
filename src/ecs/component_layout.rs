use bevy::math::{Vec2, Vec3};
use pybevy_bytecodevm::bytecode::FieldType;
// Re-export backend-agnostic types from pybevy_core
pub use pybevy_core::component_layout::{
    ComponentLayout, ComponentStorageType, FieldInfo, PrimitiveType, PrimitiveValue,
};
use pybevy_math::{vec2::PyVec2, vec3::PyVec3};
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyDict, PyType},
};

/// PyO3-specific extension methods for PrimitiveType
pub trait PrimitiveTypeExt {
    /// Try to parse a Python type annotation into a PrimitiveType
    ///
    /// TODO: replace string-based dispatch with PyO3 type-identity comparisons
    /// (e.g. `ty.is(<PyVec3 as PyTypeInfo>::type_object(py))`) for O(1) pointer
    /// equality instead of string allocation + matching on every field registration.
    fn from_python_type(ty: &Bound<'_, PyAny>) -> PyResult<Option<PrimitiveType>>;

    /// Convert to numpy dtype string
    fn to_numpy_dtype(self) -> &'static str;

    /// Convert to FieldType for bytecode VM
    fn to_field_type(self) -> FieldType;
}

impl PrimitiveTypeExt for PrimitiveType {
    fn from_python_type(ty: &Bound<'_, PyAny>) -> PyResult<Option<PrimitiveType>> {
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

    fn to_numpy_dtype(self) -> &'static str {
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

    fn to_field_type(self) -> FieldType {
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

/// PyO3-specific extension methods for ComponentLayout
pub trait ComponentLayoutExt {
    /// Compute layout from a Python class's __annotations__
    fn from_annotations(cls: &Bound<'_, PyType>) -> PyResult<ComponentLayout>;
}

impl ComponentLayoutExt for ComponentLayout {
    fn from_annotations(cls: &Bound<'_, PyType>) -> PyResult<ComponentLayout> {
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
        let mut field_types = Vec::new();

        for (field_name, field_type) in annotations.iter() {
            let field_name_bound = field_name.str()?;
            let field_name_str = field_name_bound.to_str()?.to_string();

            // Try to parse as primitive type
            let primitive_type = PrimitiveType::from_python_type(&field_type)?;

            match primitive_type {
                Some(prim_type) => {
                    field_types.push((field_name_str, prim_type));
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

        let py_type_ptr = cls.as_ptr() as *const std::ffi::c_void;

        ComponentLayout::from_fields(py_type_ptr, name.clone(), &field_types).map_err(|data_size| {
            PyTypeError::new_err(format!(
                "Component '{}' is too large ({} bytes). \
                 Maximum size for wrapper storage is 1024 bytes.",
                name, data_size
            ))
        })
    }
}

/// PyO3-specific extension methods for ComponentStorageType
pub trait ComponentStorageTypeExt {
    /// Determine storage type from a Python class
    fn from_python_class(cls: &Bound<'_, PyType>) -> PyResult<ComponentStorageType>;
}

impl ComponentStorageTypeExt for ComponentStorageType {
    fn from_python_class(cls: &Bound<'_, PyType>) -> PyResult<ComponentStorageType> {
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
        debug_assert!(
            field.offset % field.field_type.alignment() == 0,
            "field '{}' at offset {} is not aligned to {} bytes",
            field.name,
            field.offset,
            field.field_type.alignment()
        );
        debug_assert!(
            field.offset + field.field_type.size_bytes() <= buffer.len(),
            "field '{}' at offset {} + size {} exceeds buffer length {}",
            field.name,
            field.offset,
            field.field_type.size_bytes(),
            buffer.len()
        );

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
