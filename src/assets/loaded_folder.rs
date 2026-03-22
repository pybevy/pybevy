use bevy::{asset::LoadedFolder, log::warn};
use pybevy_core::{AssetStorage, NativeAsset, PyAsset, handle::PyHandle};
use pybevy_macros::native_asset;
use pyo3::prelude::*;

#[native_asset(LoadedFolder)]
#[pyclass(name = "LoadedFolder", extends = PyAsset)]
pub struct PyLoadedFolder {
    storage: AssetStorage<LoadedFolder>,
}

#[pymethods]
impl PyLoadedFolder {
    #[new]
    fn new() -> (Self, PyAsset) {
        (
            PyLoadedFolder {
                storage: AssetStorage::owned(LoadedFolder {
                    handles: Vec::new(),
                }),
            },
            PyAsset,
        )
    }

    /// Get the list of asset handles in this folder.
    ///
    /// Returns a list of Handle objects for all assets loaded from the folder.
    /// Note: Some handles may fail to convert if they reference asset types not
    /// supported by PyBevy. In such cases, a warning is logged and those handles
    /// are skipped.
    #[getter]
    fn handles(&self) -> PyResult<Vec<PyHandle>> {
        let folder = self.storage.as_ref()?;
        let mut result = Vec::with_capacity(folder.handles.len());
        let mut skipped = 0;
        for handle in &folder.handles {
            match PyHandle::try_from(handle) {
                Ok(py_handle) => result.push(py_handle),
                Err(_) => skipped += 1,
            }
        }
        if skipped > 0 {
            warn!(
                "LoadedFolder: {} handle(s) skipped due to unsupported asset types",
                skipped
            );
        }
        Ok(result)
    }

    fn __len__(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.handles.len())
    }
}
