use pyo3::{exceptions::PyNotImplementedError, prelude::*};

/// Base class for types that can be converted to materials.
///
/// This is a marker class for the Python type hierarchy.
/// Subclasses (like Color) can be used in material contexts.
#[pyclass(name = "Materializable", subclass, frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyMaterializable;

#[pymethods]
impl PyMaterializable {
    pub fn materialize(&self, _py: Python<'_>) -> PyResult<Py<PyAny>> {
        Err(PyNotImplementedError::new_err(
            "Subclasses of Materializable must implement materialize()",
        ))
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use pyo3::{
        exceptions::{PyNotImplementedError, PyTypeError},
        prelude::*,
        type_object::PyTypeInfo,
    };

    use super::*;

    #[test]
    fn class_name_and_construction_rejection() {
        Python::initialize();
        Python::attach(|py| {
            let class = PyMaterializable::type_object(py);
            assert_eq!(class.name().unwrap().to_string(), "Materializable");

            let error = class
                .call0()
                .expect_err("the marker base must not be instantiable");
            assert!(error.is_instance_of::<PyTypeError>(py));
            assert_eq!(
                error.value(py).str().unwrap().to_string(),
                "cannot create 'builtins.Materializable' instances"
            );

            let error = Py::new(py, PyMaterializable)
                .unwrap()
                .bind(py)
                .call_method0("materialize")
                .expect_err("the marker base materialize must not succeed");
            assert!(error.is_instance_of::<PyNotImplementedError>(py));
            assert_eq!(
                error.value(py).str().unwrap().to_string(),
                "Subclasses of Materializable must implement materialize()"
            );
        });
    }
}
