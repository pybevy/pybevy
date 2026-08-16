use pybevy_core::{
    AssetLoadFailedRecord, MaterializedPyAssetId, PyAssetId, PyAssetPath, PyMessage,
};
use pyo3::{prelude::*, types::PyType};

use super::asset_type::PyAssetTypeParam;
use crate::ecs::messages::{MessageType, PyMessageType};

#[pyclass(
    module = "pybevy.assets",
    name = "AssetLoadFailedEvent",
    extends = PyMessage,
    frozen,
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAssetLoadFailedEvent {
    id: PyAssetId,
    #[pyo3(get)]
    path: PyAssetPath,
    #[pyo3(get)]
    error: String,
}

#[pymethods]
impl PyAssetLoadFailedEvent {
    #[getter]
    pub fn id(&self) -> MaterializedPyAssetId {
        self.id.into()
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
            PyMessageType(MessageType::AssetLoadFailed(asset_param.type_ptr())),
        )?
        .into_any())
    }

    pub fn __repr__(&self) -> String {
        format!(
            "AssetLoadFailedEvent(id={:?}, path={:?}, error={:?})",
            self.id, self.path, self.error
        )
    }
}

impl From<AssetLoadFailedRecord> for PyAssetLoadFailedEvent {
    fn from(record: AssetLoadFailedRecord) -> Self {
        Self {
            id: PyAssetId::from_untyped(record.id),
            path: PyAssetPath::from(record.path),
            error: record.error,
        }
    }
}

pub fn materialize_asset_load_failed_record(
    py: Python<'_>,
    record: AssetLoadFailedRecord,
) -> PyResult<Py<PyAny>> {
    Ok(Py::new(py, (PyAssetLoadFailedEvent::from(record), PyMessage))?.into_any())
}
