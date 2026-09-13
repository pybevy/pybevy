//! PyO3 adapter for logical component specializations.

use pybevy_storage::LogicalTypeId;
use pyo3::{
    prelude::*,
    types::{PyAny, PyType},
};

/// Callable component specialization carrying a logical type identity.
#[pyclass(name = "LogicalComponentParam", frozen)]
pub struct PyLogicalComponentParam {
    component_type: Py<PyType>,
    logical_type_id: LogicalTypeId,
    logical_type_name: String,
}

impl PyLogicalComponentParam {
    pub fn new(
        component_type: &Bound<'_, PyType>,
        logical_type_id: LogicalTypeId,
        logical_type_name: String,
    ) -> Self {
        Self {
            component_type: component_type.clone().unbind(),
            logical_type_id,
            logical_type_name,
        }
    }

    /// Build a specialization from the declarative metadata installed on a
    /// language-level logical component type.
    pub fn from_redirect(logical_type: &Bound<'_, PyAny>) -> PyResult<Self> {
        let component = logical_type.getattr("__pybevy_component_type__")?;
        let component_type = component.cast::<PyType>()?;
        let logical_type_id = logical_type
            .getattr("__pybevy_logical_type_id__")?
            .extract::<u64>()?;
        let logical_type_name = logical_type.getattr("__qualname__")?.extract::<String>()?;
        Ok(Self::new(
            component_type,
            LogicalTypeId::new(logical_type_id),
            logical_type_name,
        ))
    }

    pub fn component_type_ptr(&self) -> *const pyo3::ffi::PyTypeObject {
        self.component_type.as_ptr().cast()
    }

    pub const fn logical_type_id(&self) -> LogicalTypeId {
        self.logical_type_id
    }

    fn optional_union(
        slf: &Py<Self>,
        py: Python<'_>,
        other: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        if !other.is_none() {
            return Ok(py.NotImplemented());
        }
        py.import("typing")?
            .getattr("Optional")?
            .get_item(slf.bind(py))
            .map(Bound::unbind)
    }
}

#[pymethods]
impl PyLogicalComponentParam {
    #[pyo3(signature = (handle, /))]
    fn __call__(&self, handle: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let py = handle.py();
        let component = self.component_type.bind(py).call1((handle,))?;
        component
            .call_method1("_with_logical_type_id", (self.logical_type_id.get(),))
            .map(Bound::unbind)
    }

    fn __repr__(&self) -> String {
        format!("LogicalComponent[{}]", self.logical_type_name)
    }

    fn __or__(slf: Py<Self>, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::optional_union(&slf, py, other)
    }

    fn __ror__(slf: Py<Self>, py: Python<'_>, other: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        Self::optional_union(&slf, py, other)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::sync::Once;

    use pyo3::types::PyInt;

    use super::*;

    static INIT: Once = Once::new();

    fn probe_param(py: Python<'_>) -> Py<PyLogicalComponentParam> {
        let component_type = py.get_type::<PyInt>();
        Py::new(
            py,
            PyLogicalComponentParam::new(
                &component_type,
                LogicalTypeId::new(7),
                "RoutingLogicalProbe".to_owned(),
            ),
        )
        .expect("probe param alloc")
    }

    #[test]
    fn component_type_pointer_does_not_require_ambient_attachment() {
        INIT.call_once(Python::initialize);
        let (param, expected) = Python::attach(|py| {
            let component_type = py.get_type::<PyInt>();
            (
                PyLogicalComponentParam::new(
                    &component_type,
                    LogicalTypeId::new(1),
                    "int".to_owned(),
                ),
                component_type.as_type_ptr(),
            )
        });

        assert_eq!(param.component_type_ptr(), expected);
    }

    #[test]
    fn repr_is_exact_and_optional_union_is_symmetric() {
        INIT.call_once(Python::initialize);
        Python::attach(|py| {
            let param = probe_param(py);
            let bound = param.bind(py);
            assert_eq!(
                bound.repr().unwrap().to_string(),
                "LogicalComponent[RoutingLogicalProbe]"
            );

            let optional = bound
                .call_method1("__or__", (py.None(),))
                .expect("__or__(None) must build the Optional union");
            // CPython renders typing.Optional as `X | None` from 3.14 and as
            // `typing.Optional[X]` before it; both spell the same union.
            let optional_repr = optional.repr().unwrap().to_string();
            assert!(
                optional_repr == "LogicalComponent[RoutingLogicalProbe] | None"
                    || optional_repr == "typing.Optional[LogicalComponent[RoutingLogicalProbe]]",
                "unexpected Optional repr: {optional_repr}"
            );
            let reflected = param
                .bind(py)
                .call_method1("__ror__", (py.None(),))
                .expect("__ror__(None) must build the Optional union");
            assert!(
                optional.is(&reflected),
                "both directions must yield the cached Optional union"
            );

            let not_implemented = param
                .bind(py)
                .call_method1("__or__", (42i32,))
                .expect("__or__ with a non-None operand must not raise");
            assert!(
                not_implemented.is(py.NotImplemented().into_bound(py)),
                "a non-None operand must defer to Python's TypeError"
            );
        });
    }
}
