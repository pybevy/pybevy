use std::hash::{Hash, Hasher};

#[allow(unused_imports)]
use bevy::asset::LoadedFolder;
use bevy::asset::{Asset, AssetId, UntypedAssetId};
use pybevy_macros::pyenum;
use pyo3::{IntoPyObjectExt, prelude::*, types::PyType};
use uuid::Uuid;

use crate::{
    LogicalTypeId, asset_index::PyAssetIndex, public_error::EXPECTED_ASSET_ID_OR_HANDLE,
    registry::global_registry,
};

#[pyenum(AssetId<LoadedFolder>, manual)]
#[pyclass(
    module = "pybevy.assets",
    name = "AssetId",
    frozen,
    subclass,
    from_py_object
)]
#[derive(Debug, Clone, Copy)]
pub struct PyAssetId {
    id: UntypedAssetId,
    logical_type_id: Option<LogicalTypeId>,
}

impl PartialEq for PyAssetId {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.logical_type_id == other.logical_type_id
    }
}

impl Eq for PyAssetId {}

impl<A: Asset> From<&AssetId<A>> for PyAssetId {
    fn from(id: &AssetId<A>) -> Self {
        Self {
            id: (*id).untyped(),
            logical_type_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterializedPyAssetId(PyAssetId);

impl From<PyAssetId> for MaterializedPyAssetId {
    fn from(value: PyAssetId) -> Self {
        Self(value)
    }
}

impl From<MaterializedPyAssetId> for PyAssetId {
    fn from(value: MaterializedPyAssetId) -> Self {
        value.0
    }
}

impl FromPyObject<'_, '_> for MaterializedPyAssetId {
    type Error = PyErr;

    fn extract(obj: Borrowed<'_, '_, PyAny>) -> PyResult<Self> {
        Ok(Self(obj.extract::<PyAssetId>()?))
    }
}

impl<'py> IntoPyObject<'py> for MaterializedPyAssetId {
    type Target = PyAssetId;
    type Output = Bound<'py, PyAssetId>;
    type Error = PyErr;

    fn into_pyobject(self, py: Python<'py>) -> PyResult<Self::Output> {
        Ok(materialize_asset_id(py, self.0)?.into_bound(py))
    }
}

impl PyAssetId {
    pub fn from_untyped(id: UntypedAssetId) -> Self {
        Self {
            id,
            logical_type_id: None,
        }
    }

    pub fn from_untyped_with_logical_type(
        id: UntypedAssetId,
        logical_type_id: Option<LogicalTypeId>,
    ) -> Self {
        Self {
            id,
            logical_type_id,
        }
    }

    pub fn untyped(&self) -> UntypedAssetId {
        self.id
    }

    pub fn asset_type_name(&self) -> Option<&'static str> {
        global_registry::get_asset_bridge_by_type_id(self.id.type_id()).map(|bridge| bridge.name())
    }

    pub fn logical_type_id(&self) -> Option<LogicalTypeId> {
        self.logical_type_id
    }
}

#[pymethods]
impl PyAssetId {
    #[new]
    pub fn new() -> PyResult<Self> {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "AssetId is an enum base; construct a nested variant",
        ))
    }

    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let _ = key;
        cls.into_py_any(cls.py())
    }

    #[staticmethod]
    pub fn uuid_from_u128(
        py: Python<'_>,
        value: u128,
        asset_type: &Bound<'_, PyType>,
    ) -> PyResult<Py<PyAssetId>> {
        let (type_id, logical_type_id) = registered_asset_identity(asset_type)?;
        materialize_asset_id(
            py,
            Self {
                id: UntypedAssetId::Uuid {
                    type_id,
                    uuid: Uuid::from_u128(value),
                },
                logical_type_id,
            },
        )
    }

    pub fn asset_type_class(&self, py: Python<'_>) -> PyResult<Py<PyType>> {
        global_registry::get_asset_bridge_by_type_id(self.id.type_id())
            .map(|bridge| bridge.py_type(py).unbind())
            .ok_or_else(|| {
                pyo3::exceptions::PyValueError::new_err(
                    "Asset type is not registered with a Python bridge",
                )
            })
    }

    fn __copy__(&self, py: Python<'_>) -> PyResult<Py<PyAssetId>> {
        materialize_asset_id(py, *self)
    }

    fn __hash__(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        self.id.hash(&mut hasher);
        self.logical_type_id.hash(&mut hasher);
        hasher.finish()
    }

    fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: pyo3::pyclass::CompareOp,
    ) -> PyResult<Py<PyAny>> {
        let py = other.py();
        let Ok(other) = other.extract::<Self>() else {
            return py.NotImplemented().into_py_any(py);
        };
        match op {
            pyo3::pyclass::CompareOp::Eq => (self == &other).into_py_any(py),
            pyo3::pyclass::CompareOp::Ne => (self != &other).into_py_any(py),
            _ => py.NotImplemented().into_py_any(py),
        }
    }

    fn __repr__(&self) -> String {
        let asset_type = self.asset_type_name().unwrap_or("Unknown");
        match self.id {
            UntypedAssetId::Index { index, .. } => {
                format!(
                    "AssetId.Index(index=AssetIndex(bits={}), asset_type={asset_type})",
                    index.to_bits()
                )
            }
            UntypedAssetId::Uuid { uuid, .. } => {
                format!(
                    "AssetId.Uuid(uuid={}, asset_type={asset_type})",
                    uuid.as_u128()
                )
            }
        }
    }
}

#[pyclass(
    module = "pybevy.assets",
    name = "Index",
    extends = PyAssetId,
    frozen
)]
pub struct PyAssetIdIndex;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyAssetIdIndex {
    #[classattr]
    const __qualname__: &'static str = "AssetId.Index";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("index",)
    }

    #[new]
    pub fn new(
        index: &PyAssetIndex,
        asset_type: &Bound<'_, PyType>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let (type_id, logical_type_id) = registered_asset_identity(asset_type)?;
        Ok(PyClassInitializer::from(PyAssetId {
            id: UntypedAssetId::Index {
                type_id,
                index: index.to_bevy()?,
            },
            logical_type_id,
        })
        .add_subclass(Self))
    }

    #[getter]
    pub fn index(slf: PyRef<'_, Self>) -> PyAssetIndex {
        let base = slf.into_super();
        match base.id {
            UntypedAssetId::Index { index, .. } => PyAssetIndex::from_owned(index),
            UntypedAssetId::Uuid { .. } => unreachable!("AssetId.Index changed discriminant"),
        }
    }
}

#[pyclass(
    module = "pybevy.assets",
    name = "Uuid",
    extends = PyAssetId,
    frozen
)]
pub struct PyAssetIdUuid;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyAssetIdUuid {
    #[classattr]
    const __qualname__: &'static str = "AssetId.Uuid";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("uuid",)
    }

    #[new]
    pub fn new(uuid: u128, asset_type: &Bound<'_, PyType>) -> PyResult<PyClassInitializer<Self>> {
        let (type_id, logical_type_id) = registered_asset_identity(asset_type)?;
        Ok(PyClassInitializer::from(PyAssetId {
            id: UntypedAssetId::Uuid {
                type_id,
                uuid: Uuid::from_u128(uuid),
            },
            logical_type_id,
        })
        .add_subclass(Self))
    }

    #[getter]
    pub fn uuid(slf: PyRef<'_, Self>) -> u128 {
        let base = slf.into_super();
        match base.id {
            UntypedAssetId::Uuid { uuid, .. } => uuid.as_u128(),
            UntypedAssetId::Index { .. } => unreachable!("AssetId.Uuid changed discriminant"),
        }
    }
}

pub fn materialize_asset_id(py: Python<'_>, id: PyAssetId) -> PyResult<Py<PyAssetId>> {
    let initializer = PyClassInitializer::from(id);
    match id.id {
        UntypedAssetId::Index { .. } => Ok(Py::new(py, initializer.add_subclass(PyAssetIdIndex))?
            .into_bound(py)
            .into_super()
            .unbind()),
        UntypedAssetId::Uuid { .. } => Ok(Py::new(py, initializer.add_subclass(PyAssetIdUuid))?
            .into_bound(py)
            .into_super()
            .unbind()),
    }
}

pub fn register_asset_id_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    let base = module.getattr("AssetId")?;
    base.setattr("Index", py.get_type::<PyAssetIdIndex>())?;
    base.setattr("Uuid", py.get_type::<PyAssetIdUuid>())?;
    Ok(())
}

pub fn extract_asset_id_from_any(obj: &Bound<'_, PyAny>) -> PyResult<PyAssetId> {
    if let Ok(id) = obj.extract::<PyAssetId>() {
        return Ok(id);
    }
    if let Ok(handle) = obj.extract::<crate::PyHandle>() {
        return Ok(handle.asset_id());
    }
    Err(pyo3::exceptions::PyTypeError::new_err(
        EXPECTED_ASSET_ID_OR_HANDLE,
    ))
}

fn registered_asset_identity(
    asset_type: &Bound<'_, PyType>,
) -> PyResult<(std::any::TypeId, Option<LogicalTypeId>)> {
    let (actual_type, logical_type_id) = if asset_type.hasattr("__pybevy_asset_type__")? {
        let actual_type = asset_type
            .getattr("__pybevy_asset_type__")?
            .cast_into::<PyType>()?;
        let logical_type_id = asset_type
            .getattr("__pybevy_logical_type_id__")?
            .extract::<u64>()?;
        (actual_type, Some(LogicalTypeId::new(logical_type_id)))
    } else {
        (asset_type.clone(), None)
    };
    let asset_type_name = asset_type.name()?;
    global_registry::get_asset_bridge_by_py_type(actual_type.as_type_ptr())
        .map(|bridge| (bridge.bevy_type_id(), logical_type_id))
        .ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err(format!(
                "Unknown asset type: {:?}. Asset type must be registered.",
                asset_type_name
            ))
        })
}
