use pyo3::{IntoPyObjectExt, prelude::*, types::PyType};
use smallvec::SmallVec;

use crate::ecs::{
    component_type::PyComponentType,
    filter::{parse_multi_component_filter, parse_single_component_filter},
};

#[pyclass(name = "With", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWith {
    pub(crate) values: SmallVec<[PyComponentType; 4]>,
}

#[pymethods]
impl PyWith {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let values = parse_multi_component_filter(py, key, "With")?;
        PyWith { values }.into_py_any(py)
    }
}

#[pyclass(name = "Without", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyWithout {
    pub(crate) values: SmallVec<[PyComponentType; 4]>,
}

#[pymethods]
impl PyWithout {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let values = parse_multi_component_filter(py, key, "Without")?;
        PyWithout { values }.into_py_any(py)
    }
}

#[pyclass(name = "Changed", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyChanged {
    pub(crate) component_type: PyComponentType,
}

#[pymethods]
impl PyChanged {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let component_type = parse_single_component_filter(py, key)?;
        PyChanged { component_type }.into_py_any(py)
    }
}

#[pyclass(name = "Added", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAdded {
    pub(crate) component_type: PyComponentType,
}

#[pymethods]
impl PyAdded {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let component_type = parse_single_component_filter(py, key)?;
        PyAdded { component_type }.into_py_any(py)
    }
}

#[pyclass(name = "Has", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyHas {
    pub(crate) component_type: PyComponentType,
}

#[pymethods]
impl PyHas {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let component_type = parse_single_component_filter(py, key)?;
        PyHas { component_type }.into_py_any(py)
    }
}

#[pyclass(name = "AnyOf", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyAnyOf {
    pub(crate) values: SmallVec<[PyComponentType; 4]>,
}

#[pymethods]
impl PyAnyOf {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let values = parse_multi_component_filter(py, key, "AnyOf")?;
        PyAnyOf { values }.into_py_any(py)
    }
}
