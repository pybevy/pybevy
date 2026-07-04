use bevy::text::Font;
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::pyasset;
use pyo3::{prelude::*, types::PyBytes};

#[pyasset(Font, bridge)]
#[pyclass(name = "Font", extends = PyAsset, skip_from_py_object)]
#[derive(Debug)]
pub struct PyFont {
    pub(crate) storage: AssetStorage<Font>,
}

#[pymethods]
impl PyFont {
    #[staticmethod]
    pub fn try_from_bytes(py: Python<'_>, font_data: &Bound<'_, PyBytes>) -> PyResult<Py<Self>> {
        let bytes = font_data.as_bytes().to_vec();
        let font = Font::from_bytes(bytes);

        Py::new(py, PyFont::from_owned(font))
    }

    pub fn data<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let font = self.as_ref()?;
        Ok(PyBytes::new(py, font.data.as_ref()))
    }
}
