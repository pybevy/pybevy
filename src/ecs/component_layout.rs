use std::ffi::c_void;

use pybevy_bytecodevm::bytecode::FieldType;
// Re-export backend-agnostic types from pybevy_core
pub use pybevy_core::component_layout::{
    ComponentLayout, ComponentSerializationError, ComponentStorageType, PrimitiveType,
    PrimitiveValue,
};
use pybevy_core::{
    component_fields::declared_annotations,
    domain::{IntegerInput, check_for},
};
use pybevy_math::{
    mat2::PyMat2, mat3::PyMat3, mat4::PyMat4, quat::PyQuat, vec2::PyVec2, vec3::PyVec3,
    vec4::PyVec4,
};
use pyo3::{
    PyTypeInfo,
    exceptions::{PyRuntimeError, PyTypeError},
    intern,
    prelude::*,
    types::{PyBool, PyDict, PyFloat, PyInt, PyString, PyType},
};

use crate::ecs::{component::PyComponent, custom_component::PyCustomComponent};

fn component_annotations<'py>(cls: &Bound<'py, PyType>) -> PyResult<Bound<'py, PyDict>> {
    let annotations = declared_annotations(cls)?;
    if !annotations
        .values()
        .iter()
        .any(|hint| hint.is_instance_of::<PyString>())
    {
        return Ok(annotations);
    }
    let hints = cls
        .py()
        .import("typing")?
        .call_method1("get_type_hints", (cls,));
    let Ok(hints) = hints else {
        return Ok(annotations);
    };
    let hints = hints.cast_into::<PyDict>()?;
    let resolved = PyDict::new(cls.py());
    for (name, hint) in annotations.iter() {
        resolved.set_item(&name, hints.get_item(&name)?.unwrap_or(hint))?;
    }
    Ok(resolved)
}

/// PyO3-specific extension methods for PrimitiveType
pub trait PrimitiveTypeExt {
    /// Try to parse a Python type annotation into a PrimitiveType
    fn from_python_type(ty: &Bound<'_, PyAny>) -> PyResult<Option<PrimitiveType>>;

    /// Convert to numpy dtype string
    fn to_numpy_dtype(self) -> &'static str;

    /// Convert to FieldType for bytecode VM; `None` for matrices, which require scalar expansion
    fn to_field_type(self) -> Option<FieldType>;
}

impl PrimitiveTypeExt for PrimitiveType {
    fn from_python_type(ty: &Bound<'_, PyAny>) -> PyResult<Option<PrimitiveType>> {
        let py = ty.py();
        if ty.is(PyFloat::type_object(py)) {
            return Ok(Some(PrimitiveType::F64));
        }
        if ty.is(PyInt::type_object(py)) {
            return Ok(Some(PrimitiveType::I64));
        }
        if ty.is(PyBool::type_object(py)) {
            return Ok(Some(PrimitiveType::Bool));
        }
        if ty.is(PyVec3::type_object(py)) {
            return Ok(Some(PrimitiveType::Vec3));
        }
        if ty.is(PyVec2::type_object(py)) {
            return Ok(Some(PrimitiveType::Vec2));
        }
        if ty.is(PyVec4::type_object(py)) {
            return Ok(Some(PrimitiveType::Vec4));
        }
        if ty.is(PyQuat::type_object(py)) {
            return Ok(Some(PrimitiveType::Quat));
        }
        if ty.is(PyMat2::type_object(py)) {
            return Ok(Some(PrimitiveType::Mat2));
        }
        if ty.is(PyMat3::type_object(py)) {
            return Ok(Some(PrimitiveType::Mat3));
        }
        if ty.is(PyMat4::type_object(py)) {
            return Ok(Some(PrimitiveType::Mat4));
        }

        let ty_str_bound = ty.str()?;
        let ty_str = ty_str_bound.to_str()?;

        // Handle typing module types
        let ty_str = if ty_str.starts_with("<class '") && ty_str.ends_with("'>") {
            &ty_str[8..ty_str.len() - 2]
        } else {
            ty_str
        };

        Ok(PrimitiveType::from_annotation_name(ty_str))
    }

    fn to_numpy_dtype(self) -> &'static str {
        match self {
            PrimitiveType::F32 => "f4",
            PrimitiveType::F64 => "f8",
            PrimitiveType::I32 => "i4",
            PrimitiveType::I64 => "i8",
            PrimitiveType::U32 => "u4",
            PrimitiveType::U64 => "u8",
            PrimitiveType::Bool => "?",
            // individual element dtype (used with 2D arrays)
            PrimitiveType::Vec3
            | PrimitiveType::Vec2
            | PrimitiveType::Vec4
            | PrimitiveType::Quat
            | PrimitiveType::Mat2
            | PrimitiveType::Mat3
            | PrimitiveType::Mat4 => "f4",
        }
    }

    fn to_field_type(self) -> Option<FieldType> {
        Some(match self {
            PrimitiveType::F32 => FieldType::F32,
            PrimitiveType::F64 => FieldType::F64,
            PrimitiveType::I32 => FieldType::I32,
            PrimitiveType::I64 => FieldType::I64,
            PrimitiveType::U32 => FieldType::U32,
            PrimitiveType::U64 => FieldType::U64,
            PrimitiveType::Bool => FieldType::Bool,
            PrimitiveType::Vec3 => FieldType::Vec3,
            PrimitiveType::Vec2 => FieldType::Vec2,
            PrimitiveType::Vec4 | PrimitiveType::Quat => FieldType::Vec4,
            PrimitiveType::Mat2 | PrimitiveType::Mat3 | PrimitiveType::Mat4 => return None,
        })
    }
}

/// Coerce one PyO3 value into the neutral primitive representation.
pub(crate) fn extract_primitive_from_py(
    field_type: PrimitiveType,
    value: &Bound<'_, PyAny>,
) -> PyResult<PrimitiveValue> {
    Ok(match field_type {
        PrimitiveType::F32 => PrimitiveValue::F32(value.extract()?),
        PrimitiveType::F64 => PrimitiveValue::F64(value.extract()?),
        PrimitiveType::I32 => PrimitiveValue::I32(
            check_for::<PyCustomComponent, _>("value", value.extract::<IntegerInput<i32>>()?, &[])?
                .stored(),
        ),
        PrimitiveType::I64 => PrimitiveValue::I64(
            check_for::<PyCustomComponent, _>("value", value.extract::<IntegerInput<i64>>()?, &[])?
                .stored(),
        ),
        PrimitiveType::U32 => PrimitiveValue::U32(
            check_for::<PyCustomComponent, _>("value", value.extract::<IntegerInput<u32>>()?, &[])?
                .stored(),
        ),
        PrimitiveType::U64 => PrimitiveValue::U64(
            check_for::<PyCustomComponent, _>("value", value.extract::<IntegerInput<u64>>()?, &[])?
                .stored(),
        ),
        PrimitiveType::Bool => PrimitiveValue::Bool(value.extract()?),
        PrimitiveType::Vec3 => {
            let py_vec3: PyRef<PyVec3> = value.extract()?;
            PrimitiveValue::Vec3((&*py_vec3).try_into()?)
        }
        PrimitiveType::Vec2 => {
            let py_vec2: PyRef<PyVec2> = value.extract()?;
            PrimitiveValue::Vec2((&*py_vec2).try_into()?)
        }
        PrimitiveType::Vec4 => {
            let py_vec4: PyRef<PyVec4> = value.extract()?;
            PrimitiveValue::Vec4((&*py_vec4).try_into()?)
        }
        PrimitiveType::Quat => {
            let py_quat: PyRef<PyQuat> = value.extract()?;
            PrimitiveValue::Quat((&*py_quat).try_into()?)
        }
        PrimitiveType::Mat2 => {
            let py_mat2: PyRef<PyMat2> = value.extract()?;
            PrimitiveValue::Mat2((&*py_mat2).try_into()?)
        }
        PrimitiveType::Mat3 => {
            let py_mat3: PyRef<PyMat3> = value.extract()?;
            PrimitiveValue::Mat3((&*py_mat3).try_into()?)
        }
        PrimitiveType::Mat4 => {
            let py_mat4: PyRef<PyMat4> = value.extract()?;
            PrimitiveValue::Mat4((&*py_mat4).try_into()?)
        }
    })
}

/// Convert one neutral primitive into an owned, independent Python value.
pub(crate) fn primitive_to_py(py: Python<'_>, value: PrimitiveValue) -> PyResult<Bound<'_, PyAny>> {
    Ok(match value {
        PrimitiveValue::F32(value) => value.into_pyobject(py)?.into_any(),
        PrimitiveValue::F64(value) => value.into_pyobject(py)?.into_any(),
        PrimitiveValue::I32(value) => value.into_pyobject(py)?.into_any(),
        PrimitiveValue::I64(value) => value.into_pyobject(py)?.into_any(),
        PrimitiveValue::U32(value) => value.into_pyobject(py)?.into_any(),
        PrimitiveValue::U64(value) => value.into_pyobject(py)?.into_any(),
        PrimitiveValue::Bool(value) => PyBool::new(py, value).to_owned().into_any(),
        PrimitiveValue::Vec3(value) => Bound::new(py, PyVec3::from(value))?.into_any(),
        PrimitiveValue::Vec2(value) => Bound::new(py, PyVec2::from(value))?.into_any(),
        PrimitiveValue::Vec4(value) => Bound::new(py, PyVec4::from(value))?.into_any(),
        PrimitiveValue::Quat(value) => Bound::new(py, PyQuat::from(value))?.into_any(),
        PrimitiveValue::Mat2(value) => Bound::new(py, PyMat2::from(value))?.into_any(),
        PrimitiveValue::Mat3(value) => Bound::new(py, PyMat3::from(value))?.into_any(),
        PrimitiveValue::Mat4(value) => Bound::new(py, PyMat4::from(value))?.into_any(),
    })
}

/// PyO3-specific extension methods for ComponentLayout
pub trait ComponentLayoutExt {
    /// Compute layout from a Python class's __annotations__
    fn from_annotations(cls: &Bound<'_, PyType>) -> PyResult<ComponentLayout>;
}

impl ComponentLayoutExt for ComponentLayout {
    fn from_annotations(cls: &Bound<'_, PyType>) -> PyResult<ComponentLayout> {
        let name = cls.name()?.to_string();

        let annotations = component_annotations(cls)?;

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
                         Wrapper storage supports float, int, bool, Vec2, Vec3, Vec4, Quat, \
                         Mat2, Mat3, and Mat4; use @component(storage=\"python\") for other types.",
                        name, field_name_str, type_str
                    )));
                }
            }
        }

        let py_type_ptr = cls.as_ptr() as *const c_void;

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

    /// Determine storage type from a Python class, keeping the layout that
    /// selected wrapper storage.
    fn storage_with_layout(
        cls: &Bound<'_, PyType>,
    ) -> PyResult<(ComponentStorageType, Option<ComponentLayout>)>;
}

fn empty_layout_has_undeclared_instance_state(cls: &Bound<'_, PyType>) -> PyResult<bool> {
    if !declared_annotations(cls)?.is_empty() {
        return Ok(false);
    }
    let object = cls.py().get_type::<PyAny>();
    let constructor = cls.getattr("__new__")?;
    let guarded = constructor
        .getattr_opt("__pybevy_missing_constructor_guard__")?
        .is_some_and(|marker| marker.is_truthy().unwrap_or(false));
    if !constructor.is(object.getattr("__new__")?)
        && !constructor.is(cls.py().get_type::<PyComponent>().getattr("__new__")?)
        && !guarded
    {
        return Ok(true);
    }
    for base in cls.getattr("__mro__")?.try_iter()? {
        let dictionary = base?.getattr("__dict__")?;
        if dictionary.contains("__slots__")? && dictionary.get_item("__slots__")?.is_truthy()? {
            return Ok(true);
        }
    }
    let initializer = cls.getattr("__init__")?;
    let class_dict = cls.getattr("__dict__")?;
    if initializer.is(object.getattr("__init__")?) {
        return Ok(cls.hasattr("__dataclass_fields__")? && class_dict.contains("__init__")?);
    }
    if !cls.hasattr("__dataclass_fields__")? {
        return Ok(true);
    }
    let generated_by_dataclass = (|| -> PyResult<bool> {
        let Some(code) = initializer.getattr_opt("__code__")? else {
            return Ok(false);
        };
        Ok(code.getattr("co_argcount")?.extract::<usize>()? == 1
            && code.getattr("co_kwonlyargcount")?.extract::<usize>()? == 0
            && code.getattr("co_names")?.len()? == 0
            && code.getattr("co_freevars")?.len()? == 0
            && code.getattr("co_cellvars")?.len()? == 0)
    })()
    .unwrap_or(false);
    Ok(!generated_by_dataclass)
}

impl ComponentStorageTypeExt for ComponentStorageType {
    fn from_python_class(cls: &Bound<'_, PyType>) -> PyResult<ComponentStorageType> {
        Self::storage_with_layout(cls).map(|(storage, _)| storage)
    }

    fn storage_with_layout(
        cls: &Bound<'_, PyType>,
    ) -> PyResult<(ComponentStorageType, Option<ComponentLayout>)> {
        let mut force_wrapper = false;
        // Check for explicit storage mode in class attributes
        if let Ok(Some(storage_attr)) = cls.getattr_opt(intern!(cls.py(), "__pybevy_storage__")) {
            let storage_str_bound = storage_attr.str()?;
            let storage_str = storage_str_bound.to_str()?;
            match storage_str {
                "pyobject" => return Ok((ComponentStorageType::PyObject, None)),
                "wrapper" => {
                    force_wrapper = true;
                }
                _ => {
                    return Err(PyTypeError::new_err(format!(
                        "Invalid storage type '{}'. Use 'wrapper' or 'pyobject'.",
                        storage_str
                    )));
                }
            }
        }

        if !force_wrapper && empty_layout_has_undeclared_instance_state(cls)? {
            return Ok((ComponentStorageType::PyObject, None));
        }

        let layout = ComponentLayout::from_annotations(cls)?;
        Ok((
            ComponentStorageType::Wrapper(layout.wrapper_size),
            Some(layout),
        ))
    }
}

/// Check a cached storage decision without rebuilding an unchanged wrapper layout.
pub(crate) fn cached_storage_and_layout_match(
    cls: &Bound<'_, PyType>,
    storage: ComponentStorageType,
    layout: Option<&ComponentLayout>,
) -> PyResult<bool> {
    let mut force_wrapper = false;
    if let Ok(Some(storage_attr)) = cls.getattr_opt(intern!(cls.py(), "__pybevy_storage__")) {
        let storage_str_bound = storage_attr.str()?;
        match storage_str_bound.to_str()? {
            "pyobject" => return Ok(storage == ComponentStorageType::PyObject),
            "wrapper" => force_wrapper = true,
            other => {
                return Err(PyTypeError::new_err(format!(
                    "Invalid storage type '{}'. Use 'wrapper' or 'pyobject'.",
                    other
                )));
            }
        }
    }

    if !force_wrapper && empty_layout_has_undeclared_instance_state(cls)? {
        return Ok(storage == ComponentStorageType::PyObject && layout.is_none());
    }

    let Some(layout) = layout else {
        return Ok(ComponentLayout::from_annotations(cls).is_err());
    };
    if !matches!(storage, ComponentStorageType::Wrapper(_)) {
        return Ok(false);
    }

    let annotations = component_annotations(cls)?;
    if annotations.len() != layout.fields.len() {
        return Ok(false);
    }

    for ((field_name, annotation), cached_field) in annotations.iter().zip(&layout.fields) {
        let Ok(field_name) = field_name.str() else {
            return Ok(false);
        };
        let Ok(field_name) = field_name.to_str() else {
            return Ok(false);
        };
        let Ok(Some(field_type)) = PrimitiveType::from_python_type(&annotation) else {
            return Ok(false);
        };
        if field_name != cached_field.name || field_type != cached_field.field_type {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Serialize a Python object to wrapper bytes according to the layout
pub fn serialize_to_wrapper(obj: &Bound<'_, PyAny>, layout: &ComponentLayout) -> PyResult<Vec<u8>> {
    layout
        .serialize_with(|field| {
            let value = obj.getattr(field.name.as_str())?;
            extract_primitive_from_py(field.field_type, &value)
        })
        .map_err(|error| match error {
            ComponentSerializationError::Extract(error) => error,
            ComponentSerializationError::Layout(error) => {
                PyRuntimeError::new_err(error.to_string())
            }
        })
}
