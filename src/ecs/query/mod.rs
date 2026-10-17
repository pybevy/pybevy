use pybevy_core::LogicalTypeId;

use crate::{
    assets::asset_type::PyAssetTypeParam,
    ecs::{
        component_type::PyComponentType,
        filter::{QueryFilter, filters::PyHas},
        query::query_helpers::construct_query_class_item,
    },
};

pub mod query_helpers;
pub mod query_param;
pub mod query_runtime;
pub mod single;
pub mod single_runtime;

use pyo3::{
    Bound,
    ffi::PyTypeObject,
    prelude::*,
    types::{PyAny, PyType},
};
use smallvec::SmallVec;

pub(crate) enum ParamType {
    Entity,
    Component {
        ty: PyComponentType,
        mutable: bool,
        optional: bool,
        logical_type_id: Option<LogicalTypeId>,
    },
    AssetResource {
        param: Py<PyAssetTypeParam>,
        type_ptr: *const PyTypeObject,
        name: String,
        mutable: bool,
        optional: bool,
    },
    Has(PyHas),
    AnyOf(SmallVec<[query_param::AnyOfItem; 4]>),
    Filter(QueryFilter),
}

impl From<QueryFilter> for ParamType {
    fn from(filter: QueryFilter) -> Self {
        ParamType::Filter(filter)
    }
}

/// An ECS Query that can be used to retrieve entities and their components.
#[pyclass(name = "Query", module = "pybevy.ecs", frozen)]
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
