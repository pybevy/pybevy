use bevy::{
    asset::Handle,
    pbr::{
        MeshMaterial3d, StandardMaterial,
        decal::{ForwardDecal, ForwardDecalMaterial, ForwardDecalMaterialExt},
    },
};
use pybevy_core::{
    AssetStorage, ComponentStorage, FieldStorage, FromBorrowedStorage, PyComponent, PyHandle,
    PyMaterial, ensure_asset_type, extract_handle_from_any,
};
use pybevy_macros::{pyasset, pycomponent, pyfield};
use pyo3::{PyTypeInfo, prelude::*, types::PyType};

use crate::standard_material::PyStandardMaterial;

#[pycomponent(ForwardDecal, unit, bridge)]
#[pyclass(name = "ForwardDecal", module = "pybevy.pbr", extends = PyComponent, frozen, eq, skip_from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyForwardDecal;

impl From<ForwardDecal> for PyForwardDecal {
    fn from(_: ForwardDecal) -> Self {
        PyForwardDecal
    }
}

impl From<PyForwardDecal> for ForwardDecal {
    fn from(_: PyForwardDecal) -> Self {
        ForwardDecal
    }
}

impl TryFrom<&ForwardDecal> for PyForwardDecal {
    type Error = PyErr;
    fn try_from(_: &ForwardDecal) -> PyResult<Self> {
        Ok(PyForwardDecal)
    }
}

#[pymethods]
impl PyForwardDecal {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyForwardDecal, PyComponent).into()
    }

    fn __repr__(&self) -> &'static str {
        "ForwardDecal"
    }
}

#[pyfield]
#[pyclass(
    name = "ForwardDecalMaterialExt",
    module = "pybevy.pbr",
    from_py_object
)]
#[derive(Debug)]
pub struct PyForwardDecalMaterialExt {
    storage: FieldStorage<ForwardDecalMaterialExt>,
}

#[pymethods]
impl PyForwardDecalMaterialExt {
    #[new]
    #[pyo3(signature = (depth_fade_factor = 8.0))]
    pub fn new(depth_fade_factor: f32) -> Self {
        Self::from_owned(ForwardDecalMaterialExt { depth_fade_factor })
    }

    #[getter]
    pub fn depth_fade_factor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.depth_fade_factor)
    }

    #[setter]
    pub fn set_depth_fade_factor(&mut self, depth_fade_factor: f32) -> PyResult<()> {
        self.as_mut()?.depth_fade_factor = depth_fade_factor;
        Ok(())
    }
}

#[pyasset(
    ForwardDecalMaterial<StandardMaterial>,
    bridge,
    not_loadable,
    material
)]
#[pyclass(
    name = "ForwardDecalMaterial", module = "pybevy.pbr",
    extends = PyMaterial,
    skip_from_py_object
)]
#[derive(Debug)]
pub struct PyForwardDecalMaterial {
    pub(crate) storage: AssetStorage<ForwardDecalMaterial<StandardMaterial>>,
}

#[pymethods]
impl PyForwardDecalMaterial {
    #[new]
    #[pyo3(signature = (base = None, extension = None))]
    pub fn new(
        base: Option<PyRef<'_, PyStandardMaterial>>,
        extension: Option<PyRef<'_, PyForwardDecalMaterialExt>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let base = match base {
            Some(base) => base.storage.as_ref()?.clone(),
            None => StandardMaterial::default(),
        };
        let extension = match extension {
            Some(extension) => (&*extension).try_into()?,
            None => ForwardDecalMaterialExt::default(),
        };
        Ok(Self::from_owned(ForwardDecalMaterial { base, extension }))
    }

    #[classattr]
    fn __pybevy_component_type__(py: Python<'_>) -> Bound<'_, PyType> {
        PyMeshMaterial3dForwardDecal::type_object(py)
    }

    #[getter]
    pub fn base(&self, py: Python<'_>) -> PyResult<Py<PyStandardMaterial>> {
        let storage = self
            .storage
            .borrow_asset_field(|material| &material.base, |material| &mut material.base)?;
        Py::new(py, PyStandardMaterial::from_borrowed(storage))
    }

    #[setter]
    pub fn set_base(&mut self, base: PyRef<'_, PyStandardMaterial>) -> PyResult<()> {
        let base = base.storage.as_ref()?.clone();
        self.as_mut()?.base = base;
        Ok(())
    }

    #[getter]
    pub fn extension(&self, py: Python<'_>) -> PyResult<Py<PyForwardDecalMaterialExt>> {
        let storage = self.storage.borrow_field(
            |material| &material.extension,
            |material| &mut material.extension,
        )?;
        Py::new(py, PyForwardDecalMaterialExt::from_borrowed(storage))
    }

    #[setter]
    pub fn set_extension(
        &mut self,
        extension: PyRef<'_, PyForwardDecalMaterialExt>,
    ) -> PyResult<()> {
        let extension = (&*extension).try_into()?;
        self.as_mut()?.extension = extension;
        Ok(())
    }
}

#[pycomponent(
    MeshMaterial3d<ForwardDecalMaterial<StandardMaterial>>,
    bridge
)]
#[pyclass(
    name = "MeshMaterial3dForwardDecal", module = "pybevy.pbr",
    extends = PyComponent,
    eq,
    skip_from_py_object
)]
#[derive(Debug, PartialEq)]
pub struct PyMeshMaterial3dForwardDecal {
    pub(crate) storage: ComponentStorage<MeshMaterial3d<ForwardDecalMaterial<StandardMaterial>>>,
}

fn extract_forward_decal_material_handle(
    material: &Bound<'_, PyAny>,
) -> PyResult<Handle<ForwardDecalMaterial<StandardMaterial>>> {
    let handle = extract_handle_from_any(material)?;

    ensure_asset_type::<ForwardDecalMaterial<StandardMaterial>>(&handle)?;

    (&handle).try_into()
}

#[pymethods]
impl PyMeshMaterial3dForwardDecal {
    #[new]
    pub fn new(material: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            Self::from_owned(MeshMaterial3d(extract_forward_decal_material_handle(
                material,
            )?))
            .into(),
        )
    }

    #[getter]
    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok((&self.as_ref()?.0).into())
    }

    #[setter]
    pub fn set_handle(&mut self, handle: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.0 = extract_forward_decal_material_handle(handle)?;
        Ok(())
    }
}
