use bevy::{
    asset::Handle,
    pbr::{MeshMaterial3d, StandardMaterial},
};
use pybevy_core::{
    ComponentStorage, PyComponent, PyHandle, PyLogicalComponentParam, ensure_asset_type,
    extract_handle_from_any,
};
use pybevy_macros::pycomponent;
use pyo3::{prelude::*, types::PyType};

#[pycomponent(MeshMaterial3d<StandardMaterial>, bridge)]
#[pyclass(name = "MeshMaterial3d", module = "pybevy.mesh", extends = PyComponent, eq, skip_from_py_object)]
#[derive(Debug, PartialEq)]
pub struct PyMeshMaterial3d {
    pub(crate) storage: ComponentStorage<MeshMaterial3d<StandardMaterial>>,
}

fn extract_material_handle(material: &Bound<'_, PyAny>) -> PyResult<Handle<StandardMaterial>> {
    let handle = extract_handle_from_any(material)?;

    ensure_asset_type::<StandardMaterial>(&handle)?;

    (&handle).try_into()
}

#[pymethods]
impl PyMeshMaterial3d {
    #[new]
    pub fn new(material: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(MeshMaterial3d(extract_material_handle(material)?)).into())
    }

    /// Support `MeshMaterial3d[HologramMaterial]` subscript notation.
    ///
    /// Custom materials retain their Python class identity while sharing the native
    /// `MeshMaterial3d<ShaderMaterial>` storage type.
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if key.hasattr("__pybevy_component_type__")? && key.hasattr("__pybevy_logical_type_id__")? {
            return Ok(Py::new(cls.py(), PyLogicalComponentParam::from_redirect(key)?)?.into_any());
        }
        if key.hasattr("__pybevy_component_type__")? {
            return Ok(key
                .getattr("__pybevy_component_type__")?
                .cast_into::<PyType>()?
                .into_any()
                .unbind());
        }
        // Default: return MeshMaterial3d itself
        Ok(cls.clone().into_any().unbind())
    }

    #[getter]
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok((&self.as_ref()?.0).into())
    }

    #[setter]
    pub fn set_handle(&mut self, handle: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.0 = extract_material_handle(handle)?;
        Ok(())
    }
}
