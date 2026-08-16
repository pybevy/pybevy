use pybevy_core::public_error::{
    OR_FILTER_EMPTY, OR_FILTER_ITEM_REQUIRED, OR_FILTER_TUPLE_REQUIRED,
};
use pyo3::{IntoPyObjectExt, prelude::*, types::PyType};
use smallvec::SmallVec;

use crate::ecs::{
    component_type::PyComponentType,
    filter::{QueryFilter, parse_multi_component_filter, parse_single_component_filter},
    query::{query_helpers::parse_anyof_items, query_param::AnyOfItem},
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
    pub(crate) items: SmallVec<[AnyOfItem; 4]>,
}

#[pyclass(name = "Or", from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyOr {
    pub(crate) values: Vec<QueryFilter>,
}

#[pymethods]
impl PyOr {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let py = cls.py();
        let origin = key
            .getattr("__origin__")
            .map_err(|_| pyo3::exceptions::PyTypeError::new_err(OR_FILTER_TUPLE_REQUIRED))?;
        if !origin.is(py.get_type::<pyo3::types::PyTuple>()) {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                OR_FILTER_TUPLE_REQUIRED,
            ));
        }
        let args = key.getattr("__args__")?;
        let mut values = Vec::new();
        for value in args.try_iter()? {
            let value = value?;
            let filter = if value.is_instance_of::<PyWith>() {
                QueryFilter::With(value.extract()?)
            } else if value.is_instance_of::<PyWithout>() {
                QueryFilter::Without(value.extract()?)
            } else if value.is_instance_of::<PyChanged>() {
                QueryFilter::Changed(value.extract()?)
            } else if value.is_instance_of::<PyAdded>() {
                QueryFilter::Added(value.extract()?)
            } else if value.is_instance_of::<PyOr>() {
                values.extend(value.extract::<PyOr>()?.values);
                continue;
            } else {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    OR_FILTER_ITEM_REQUIRED,
                ));
            };
            values.push(filter);
        }
        if values.is_empty() {
            return Err(pyo3::exceptions::PyTypeError::new_err(OR_FILTER_EMPTY));
        }
        PyOr { values }.into_py_any(py)
    }
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
        let items = parse_anyof_items(py, key)?;
        PyAnyOf { items }.into_py_any(py)
    }
}
