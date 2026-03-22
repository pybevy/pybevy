use filters::{PyAdded, PyAnyOf, PyChanged, PyHas, PyWith, PyWithout};
use pyo3::{prelude::*, types::PyAny};
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
    /// Components that are present
    Has(PyHas),
    /// Any of the specified components
    AnyOf(PyAnyOf),
}

/// Helper function to parse multiple component types from a filter parameter.
///
/// This handles the following cases:
/// - `tuple[A, B]` - Multiple components via GenericAlias with __args__
/// - `(A, B)` - Plain tuple syntax (returns error, requires explicit tuple[])
/// - `A` - Single component type
///
/// Used by filters that accept multiple components: With, Without, AnyOf
pub(crate) fn parse_multi_component_filter(
    py: Python,
    key: &Bound<'_, PyAny>,
    filter_name: &str,
) -> PyResult<SmallVec<[PyComponentType; 4]>> {
    if let Ok(args) = key.getattr("__args__") {
        // This is a GenericAlias like tuple[A, B] - extract from __args__
        let mut components = SmallVec::new();
        for item in args.try_iter()? {
            let item = item?;
            let component = extract_component_type_from_query_param(py, &item)?;
            components.push(component);
        }
        Ok(components)
    } else if key.is_instance_of::<pyo3::types::PyTuple>() {
        // Plain tuple (A, B) is not supported - require explicit tuple[] syntax
        Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "Use {}[tuple[...]] for multiple components, not {}[(...)]. \
            Example: {}[tuple[ComponentA, ComponentB]]",
            filter_name, filter_name, filter_name
        )))
    } else {
        // Single component type
        let component = extract_component_type_from_query_param(py, &key)?;
        Ok(smallvec![component])
    }
}

/// Helper function to parse a single component type from a filter parameter.
///
/// Used by filters that accept only one component: Changed, Added, Has
pub(crate) fn parse_single_component_filter(
    py: Python,
    key: &Bound<'_, PyAny>,
) -> PyResult<PyComponentType> {
    extract_component_type_from_query_param(py, key)
}
