use bevy::scene::SceneRoot;
use pybevy_core::{PyComponent, PyHandle, extract_handle_from_any};
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pyclass(name = "SceneRoot", extends = PyComponent, eq, frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PySceneRoot(pub(crate) PyHandle);

impl TryFrom<&PySceneRoot> for SceneRoot {
    type Error = PyErr;

    fn try_from(value: &PySceneRoot) -> Result<Self, Self::Error> {
        Ok(SceneRoot((&value.0).try_into()?))
    }
}

impl From<&SceneRoot> for PySceneRoot {
    fn from(value: &SceneRoot) -> Self {
        PySceneRoot((&value.0).into())
    }
}

#[pymethods]
impl PySceneRoot {
    #[new]
    pub fn new(handle: &Bound<'_, PyAny>) -> PyResult<(Self, PyComponent)> {
        let handle = extract_handle_from_any(handle)?;

        if let Some(name) = handle.asset_type_name()
            && name != "Scene"
        {
            return Err(PyTypeError::new_err(format!(
                "AssetType `{}` does not match expected type `Scene`",
                name
            )));
        }

        Ok((Self(handle), PyComponent))
    }

    pub fn handle(&self) -> PyResult<PyHandle> {
        Ok(self.0.clone())
    }
}
