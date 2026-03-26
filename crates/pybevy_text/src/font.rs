use bevy::text::Font;
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::asset_storage;
use pyo3::{exceptions::PyValueError, prelude::*, types::PyBytes};

#[asset_storage(Font)]
#[pyclass(name = "Font", extends = PyAsset)]
#[derive(Debug)]
pub struct PyFont {
    pub(crate) storage: AssetStorage<Font>,
}

#[pymethods]
impl PyFont {
    #[staticmethod]
    pub fn try_from_bytes(py: Python<'_>, font_data: &Bound<'_, PyBytes>) -> PyResult<Py<Self>> {
        let bytes = font_data.as_bytes().to_vec();
        let font = Font::try_from_bytes(bytes)
            .map_err(|e| PyValueError::new_err(format!("Failed to parse font data: {:?}", e)))?;

        Py::new(py, PyFont::from_owned(font))
    }

    pub fn data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let font = self.as_ref()?;
        Ok(PyBytes::new(py, &font.data))
    }
}
