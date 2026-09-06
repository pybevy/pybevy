use std::sync::Arc;

use pybevy_core::LogicalTypeId;
use pyo3::{PyTraverseError, PyVisit, prelude::*, types::PyType};
use smallvec::SmallVec;

use crate::ecs::{
    component_type::{PyComponentType, clone_retained_classes, retain_custom_classes},
    filter::QueryFilter,
};

/// Query parameters for ECS queries.
///
/// `Query[Transform, With[Velocity]]` would create a `PyQueryParam` with
/// `component_types` containing `Transform` and `filters` containing `With[Velocity]
#[pyclass(name = "QueryParam", module = "pybevy.ecs", frozen, from_py_object)]
#[derive(Debug)]
pub struct PyQueryParam {
    /// The component types in the query parameter.
    pub(crate) data: SmallVec<[QueryData; 16]>,
    /// Indicates if the query is for a single component type only.
    pub(crate) single: bool,
    /// The filters in the query parameter.
    pub(crate) filters: SmallVec<[QueryFilter; 4]>,
    /// Indicates if the query enforces exactly one entity (Single<T>).
    pub(crate) single_entity_enforced: bool,
    /// Strong references backing Python-owned component/resource pointers above.
    ///
    /// `@component` and `@resource` classes are ordinary Python objects. Their
    /// registries drop old strong references when a same-qualname class
    /// re-registers onto the ComponentId, so nothing else keeps a class alive
    /// for the life of a query that stored its pointer. Holding them here means
    /// the pointers cannot dangle while this param exists.
    #[allow(
        dead_code,
        reason = "held purely to keep the type pointers above valid"
    )]
    pub(crate) retained_types: SmallVec<[Arc<Py<PyType>>; 4]>,
}

impl PyQueryParam {
    pub(crate) fn _param_types(&self) -> &[QueryData] {
        &self.data
    }

    /// Retain every custom class this param names; see
    /// `component_type::retain_custom_classes` and docs/safety.md section 6.
    pub(crate) fn retain_custom_types(
        py: Python<'_>,
        data: &[QueryData],
        filters: &[QueryFilter],
    ) -> SmallVec<[Arc<Py<PyType>>; 4]> {
        let from_data = data.iter().flat_map(QueryData::component_types);
        let from_filters = filters.iter().flat_map(QueryFilter::component_types);
        retain_custom_classes(py, from_data.chain(from_filters))
    }

    /// Copy the query shape without duplicating its ownership edge.
    ///
    /// Used only for the private cached copy embedded in an ad-hoc runtime;
    /// that runtime's independently retained `param` keeps every pointer live.
    pub(crate) fn clone_without_retained_types(&self) -> Self {
        Self {
            data: self.data.clone(),
            single: self.single,
            filters: self.filters.clone(),
            single_entity_enforced: self.single_entity_enforced,
            retained_types: SmallVec::new(),
        }
    }
}

impl Clone for PyQueryParam {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            data: self.data.clone(),
            single: self.single,
            filters: self.filters.clone(),
            single_entity_enforced: self.single_entity_enforced,
            retained_types: clone_retained_classes(py, &self.retained_types),
        })
    }
}

/// `retained_types` is derived from `data` and `filters`, so it carries no
/// identity of its own and is excluded from equality.
impl PartialEq for PyQueryParam {
    fn eq(&self, other: &Self) -> bool {
        self.data == other.data
            && self.single == other.single
            && self.filters == other.filters
            && self.single_entity_enforced == other.single_entity_enforced
    }
}

#[pymethods]
impl PyQueryParam {
    /// Returns a list of component type names in the query parameter.
    #[getter]
    pub fn _data_names(&self, py: Python<'_>) -> Vec<String> {
        self.data.iter().map(|data| data.display_name(py)).collect()
    }

    /// Returns the number of component types in the query parameter.
    pub fn _data_len(&self) -> usize {
        self.data.len()
    }

    /// Returns the number of filters in the query parameter.
    pub fn _filter_len(&self) -> usize {
        self.filters.len()
    }

    /// Report the retained classes to Python's cyclic GC.
    ///
    /// `retained_types` is a Rust-held strong reference into Python, and a
    /// component class reaches back here through its module dict and the
    /// annotations of any system that names it. Without this the cycle is
    /// invisible to the collector and every reload leaks a generation.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained_types {
            visit.call(class.as_ref())?;
        }
        Ok(())
    }

    pub fn __repr__(&self, py: Python<'_>) -> String {
        let parts: Vec<String> = self.data.iter().map(|data| data.display_name(py)).collect();
        format!("Query[{}]", parts.join(", "))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum QueryData {
    /// An entity type in the query parameter.
    Entity,
    /// A component type in the query parameter.
    Component {
        /// The type of the component.
        ty: PyComponentType,
        /// Whether the component access is requested as mutable.
        mutable: bool,
        /// Whether this component is optional (Optional[T] / T | None).
        optional: bool,
        /// Optional Python-level identity when several types share this native component.
        logical_type_id: Option<LogicalTypeId>,
    },
    /// Whether the entity has a component, without accessing its value.
    Has { ty: PyComponentType },
    /// Bevy `AnyOf` query data. Every item is fetched optionally, while the
    /// query matches only entities for which at least one item is present.
    AnyOf { items: SmallVec<[AnyOfItem; 4]> },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AnyOfItem {
    pub(crate) ty: PyComponentType,
    pub(crate) mutable: bool,
    pub(crate) logical_type_id: Option<LogicalTypeId>,
}

impl QueryData {
    pub(crate) fn component_types(&self) -> SmallVec<[PyComponentType; 4]> {
        match self {
            Self::Component { ty, .. } | Self::Has { ty } => smallvec::smallvec![*ty],
            Self::AnyOf { items } => items.iter().map(|item| item.ty).collect(),
            Self::Entity => SmallVec::new(),
        }
    }

    fn display_name(&self, py: Python<'_>) -> String {
        match self {
            QueryData::Entity => "Entity".to_owned(),
            QueryData::Component {
                ty,
                mutable,
                optional,
                logical_type_id: _,
            } => {
                let inner = if *mutable {
                    format!("Mut[{}]", ty.display_name(py))
                } else {
                    ty.display_name(py)
                };
                if *optional {
                    format!("Optional[{inner}]")
                } else {
                    inner
                }
            }
            QueryData::Has { ty } => format!("Has[{}]", ty.display_name(py)),
            QueryData::AnyOf { items } => format!(
                "AnyOf[tuple[{}]]",
                items
                    .iter()
                    .map(|item| {
                        let name = item.ty.display_name(py);
                        if item.mutable {
                            format!("Mut[{name}]")
                        } else {
                            name
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }
}
