use pyo3::{IntoPyObjectExt, prelude::*, types::PyType};
use smallvec::SmallVec;

use crate::ecs::{
    component_type::PyComponentType,
    filter::{parse_multi_component_filter, parse_single_component_filter},
};

#[pyclass(name = "With")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWith {
    pub(crate) values: SmallVec<[PyComponentType; 4]>,
}

#[pymethods]
impl PyWith {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__<'a>(
        cls: &'a Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let values = parse_multi_component_filter(py, key, "With")?;
        Ok(PyWith { values }.into_py_any(py)?)
    }
}

#[pyclass(name = "Without")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWithout {
    pub(crate) values: SmallVec<[PyComponentType; 4]>,
}

#[pymethods]
impl PyWithout {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__<'a>(
        cls: &'a Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let values = parse_multi_component_filter(py, key, "Without")?;
        Ok(PyWithout { values }.into_py_any(py)?)
    }
}

#[pyclass(name = "Changed")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyChanged {
    pub(crate) component_type: PyComponentType,
}

#[pymethods]
impl PyChanged {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__<'a>(
        cls: &'a Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let component_type = parse_single_component_filter(py, key)?;
        Ok(PyChanged { component_type }.into_py_any(py)?)
    }
}

#[pyclass(name = "Added")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAdded {
    pub(crate) component_type: PyComponentType,
}

#[pymethods]
impl PyAdded {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__<'a>(
        cls: &'a Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let component_type = parse_single_component_filter(py, key)?;
        Ok(PyAdded { component_type }.into_py_any(py)?)
    }
}

#[pyclass(name = "Has")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyHas {
    pub(crate) component_type: PyComponentType,
}

#[pymethods]
impl PyHas {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__<'a>(
        cls: &'a Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let component_type = parse_single_component_filter(py, key)?;
        Ok(PyHas { component_type }.into_py_any(py)?)
    }
}

#[pyclass(name = "AnyOf")]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAnyOf {
    pub(crate) values: SmallVec<[PyComponentType; 4]>,
}

#[pymethods]
impl PyAnyOf {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__<'a>(
        cls: &'a Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let values = parse_multi_component_filter(py, key, "AnyOf")?;
        Ok(PyAnyOf { values }.into_py_any(py)?)
    }
}
