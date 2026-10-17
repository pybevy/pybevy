use std::sync::Arc;

use filters::{PyAdded, PyAllow, PyChanged, PyOr, PyWith, PyWithout};
use pybevy_core::public_error::{
    multi_component_filter_tuple_required, parenthesized_component_filter,
};
use pyo3::{
    exceptions::PyTypeError,
    prelude::*,
    types::{PyAny, PyTuple, PyType},
};
use smallvec::{SmallVec, smallvec};

use crate::ecs::{
    component_type::PyComponentType, query::query_helpers::extract_component_type_from_query_param,
};

pub mod filters;

/// Filter types for ECS queries
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum QueryFilter {
    /// Components that must be present
    With(PyWith),
    /// Components that must be absent
    Without(PyWithout),
    /// Components that have changed
    Changed(PyChanged),
    /// Components that have been added
    Added(PyAdded),
    /// A disabling component the query opts into matching
    Allow(PyAllow),
    /// At least one of the nested query filters
    Or(PyOr),
}

impl QueryFilter {
    /// Every component type this filter references, in declaration order.
    pub(crate) fn component_types(&self) -> SmallVec<[PyComponentType; 4]> {
        match self {
            Self::With(filter) => filter.values.clone(),
            Self::Without(filter) => filter.values.clone(),
            Self::Or(filter) => filter
                .values
                .iter()
                .flat_map(Self::component_types)
                .collect(),
            Self::Changed(filter) => smallvec![filter.component_type],
            Self::Added(filter) => smallvec![filter.component_type],
            Self::Allow(filter) => smallvec![filter.component_type],
        }
    }

    /// Raw-only copy for holders whose owner retains the classes separately.
    /// Or flattens detached nested filters so no marker in the copy holds increfs.
    pub(crate) fn clone_detached(&self) -> Self {
        match self {
            Self::With(filter) => Self::With(filter.detached()),
            Self::Without(filter) => Self::Without(filter.detached()),
            Self::Changed(filter) => Self::Changed(filter.detached()),
            Self::Added(filter) => Self::Added(filter.detached()),
            Self::Allow(filter) => Self::Allow(filter.detached()),
            Self::Or(filter) => Self::Or(PyOr {
                values: filter.values.iter().map(Self::clone_detached).collect(),
            }),
        }
    }

    /// Every strongly retained class reference in this filter, in declaration
    /// order. Each reference is owned by the nested marker that created it;
    /// holders of this value must traverse exactly these edges.
    pub(crate) fn retained_classes(&self) -> SmallVec<[&Arc<Py<PyType>>; 4]> {
        match self {
            Self::With(filter) => filter.retained.iter().collect(),
            Self::Without(filter) => filter.retained.iter().collect(),
            Self::Or(filter) => filter
                .values
                .iter()
                .flat_map(Self::retained_classes)
                .collect(),
            Self::Changed(filter) => filter.retained.iter().collect(),
            Self::Added(filter) => filter.retained.iter().collect(),
            Self::Allow(filter) => filter.retained.iter().collect(),
        }
    }
}

/// Helper function to parse multiple component types from a filter parameter.
///
/// This handles the following cases:
/// - `tuple[A, B]` - Multiple components via GenericAlias with __args__
/// - `(A, B)` - Plain tuple syntax (returns error, requires explicit tuple[])
/// - `A` - Single component type
///
/// Used by filters that accept multiple components: With and Without.
pub(crate) fn parse_multi_component_filter(
    py: Python,
    key: &Bound<'_, PyAny>,
    filter_name: &str,
) -> PyResult<SmallVec<[PyComponentType; 4]>> {
    if let Ok(args) = key.getattr("__args__") {
        if !key.getattr("__origin__")?.is(py.get_type::<PyTuple>()) {
            return Err(PyTypeError::new_err(multi_component_filter_tuple_required(
                filter_name,
            )));
        }
        let mut components = SmallVec::new();
        for item in args.try_iter()? {
            let item = item?;
            let component = extract_component_type_from_query_param(py, &item)?;
            components.push(component);
        }
        Ok(components)
    } else if key.is_instance_of::<PyTuple>() {
        Err(PyTypeError::new_err(parenthesized_component_filter(
            filter_name,
        )))
    } else {
        // Single component type
        let component = extract_component_type_from_query_param(py, key)?;
        Ok(smallvec![component])
    }
}

/// Helper function to parse a single component type from a filter parameter.
///
/// Used by filters that accept only one component: Changed, Added, Allow, Has
pub(crate) fn parse_single_component_filter(
    py: Python,
    key: &Bound<'_, PyAny>,
) -> PyResult<PyComponentType> {
    extract_component_type_from_query_param(py, key)
}
