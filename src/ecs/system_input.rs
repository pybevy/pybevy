use pyo3::{
    PyTraverseError, PyTypeInfo, PyVisit,
    prelude::*,
    types::{PyAny, PyTuple, PyType},
};

/// Annotation marker for the value received from the previous stage of a pipe.
#[pyclass(name = "In", frozen)]
pub struct PyIn;

/// Descriptor returned by `In[T]`.
#[pyclass(name = "InParam", frozen)]
pub struct PyInParam {
    value_type: Py<PyAny>,
    type_name: String,
}

impl PyInParam {
    pub(crate) fn value_type(&self, py: Python<'_>) -> Py<PyAny> {
        self.value_type.clone_ref(py)
    }
}

#[pymethods]
impl PyIn {
    #[classmethod]
    #[pyo3(signature = (key, /))]
    fn __class_getitem__(_cls: &Bound<'_, PyType>, key: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let type_name = key
            .getattr("__name__")
            .and_then(|name| name.extract::<String>())
            .or_else(|_| key.str().map(|name| name.to_string()))?;
        Py::new(
            key.py(),
            PyInParam {
                value_type: key.clone().unbind(),
                type_name,
            },
        )
        .map(Py::into_any)
    }
}

#[pymethods]
impl PyInParam {
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.value_type)
    }

    #[getter]
    fn __origin__(&self, py: Python<'_>) -> Py<PyAny> {
        PyIn::type_object(py).into_any().unbind()
    }

    #[getter]
    fn __args__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, [self.value_type.bind(py)])
    }

    fn __repr__(&self) -> String {
        format!("In[{}]", self.type_name)
    }
}
