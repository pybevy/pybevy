use crate::ecs::{
    component_type::PyComponentType, filter::QueryFilter,
    query::query_helpers::construct_query_class_item,
};

pub mod query_helpers;
pub mod query_param;
pub mod query_runtime;
pub mod single;
pub mod single_runtime;

use pyo3::{
    Bound,
    prelude::*,
    types::{PyAny, PyType},
};

pub(crate) enum ParamType {
    Entity,
    Component {
        ty: PyComponentType,
        mutable: bool,
        optional: bool,
    },
    Filter(QueryFilter),
}

impl From<QueryFilter> for ParamType {
    fn from(filter: QueryFilter) -> Self {
        ParamType::Filter(filter)
    }
}

/// An ECS Query that can be used to retrieve entities and their components.
#[pyclass(name = "Query", frozen)]
pub struct PyQuery;

#[pymethods]
impl PyQuery {
    /// Creates a new `QueryParam` from the given type parameters.
    ///
    /// This enables syntax like `Query[Transform]` or `Query[tuple[Transform, Velocity]]`
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        construct_query_class_item(cls, key)
    }
}
