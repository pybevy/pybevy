use bevy::{
    asset::{Asset, AssetEvent},
    image::Image,
};
use pybevy_core::{AssetEventRecord, MaterializedPyAssetId, PyAssetId, extract_asset_id_from_any};
use pybevy_macros::pyenum;
use pyo3::{prelude::*, types::PyType};

use super::asset_type::PyAssetTypeParam;
use crate::ecs::messages::{MessageType, PyMessageType};

#[derive(Debug, Clone, PartialEq)]
enum AssetEventValue {
    Added { id: PyAssetId },
    Modified { id: PyAssetId },
    Removed { id: PyAssetId },
    Unused { id: PyAssetId },
    LoadedWithDependencies { id: PyAssetId },
}

impl<A: Asset> From<&AssetEvent<A>> for AssetEventValue {
    fn from(event: &AssetEvent<A>) -> Self {
        match event {
            AssetEvent::Added { id } => Self::Added {
                id: PyAssetId::from(id),
            },
            AssetEvent::Modified { id } => Self::Modified {
                id: PyAssetId::from(id),
            },
            AssetEvent::Removed { id } => Self::Removed {
                id: PyAssetId::from(id),
            },
            AssetEvent::Unused { id } => Self::Unused {
                id: PyAssetId::from(id),
            },
            AssetEvent::LoadedWithDependencies { id } => Self::LoadedWithDependencies {
                id: PyAssetId::from(id),
            },
        }
    }
}

impl From<AssetEventRecord> for AssetEventValue {
    fn from(event: AssetEventRecord) -> Self {
        match event {
            AssetEventRecord::Added { id } => Self::Added {
                id: PyAssetId::from_untyped(id),
            },
            AssetEventRecord::Modified { id } => Self::Modified {
                id: PyAssetId::from_untyped(id),
            },
            AssetEventRecord::Removed { id } => Self::Removed {
                id: PyAssetId::from_untyped(id),
            },
            AssetEventRecord::Unused { id } => Self::Unused {
                id: PyAssetId::from_untyped(id),
            },
            AssetEventRecord::LoadedWithDependencies { id } => Self::LoadedWithDependencies {
                id: PyAssetId::from_untyped(id),
            },
        }
    }
}

#[pyenum(
    AssetEvent<Image>,
    message,
    no_bridge,
    mirror = AssetEventValue
)]
#[pyclass(module = "pybevy.assets", name = "AssetEvent")]
pub enum PyAssetEvent {
    Added {
        #[py_type(MaterializedPyAssetId)]
        id: PyAssetId,
    },
    Modified {
        #[py_type(MaterializedPyAssetId)]
        id: PyAssetId,
    },
    Removed {
        #[py_type(MaterializedPyAssetId)]
        id: PyAssetId,
    },
    Unused {
        #[py_type(MaterializedPyAssetId)]
        id: PyAssetId,
    },
    LoadedWithDependencies {
        #[py_type(MaterializedPyAssetId)]
        id: PyAssetId,
    },
}

#[pymethods]
impl PyAssetEvent {
    pub fn is_added(&self, asset_id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let id = extract_asset_id_from_any(asset_id)?;
        Ok(self.inner == AssetEventValue::Added { id })
    }

    pub fn is_modified(&self, asset_id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let id = extract_asset_id_from_any(asset_id)?;
        Ok(self.inner == AssetEventValue::Modified { id })
    }

    pub fn is_removed(&self, asset_id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let id = extract_asset_id_from_any(asset_id)?;
        Ok(self.inner == AssetEventValue::Removed { id })
    }

    pub fn is_unused(&self, asset_id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let id = extract_asset_id_from_any(asset_id)?;
        Ok(self.inner == AssetEventValue::Unused { id })
    }

    pub fn is_loaded_with_dependencies(&self, asset_id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let id = extract_asset_id_from_any(asset_id)?;
        Ok(self.inner == AssetEventValue::LoadedWithDependencies { id })
    }

    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        _cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let asset_type = key.cast::<PyType>()?;
        let asset_param = PyAssetTypeParam::try_from_py_type(asset_type)?;
        Ok(Py::new(
            key.py(),
            PyMessageType(MessageType::AssetEvent(asset_param.type_ptr())),
        )?
        .into_any())
    }
}

pub fn materialize_asset_event_record(
    py: Python<'_>,
    event: AssetEventRecord,
) -> PyResult<Py<PyAny>> {
    materialize_asset_event_inner(py, AssetEventValue::from(event))
}
