use core::fmt;

use pyo3::prelude::*;
use smallvec::SmallVec;

use crate::ecs::{component_type::PyComponentType, filter::QueryFilter};

/// Query parameters for ECS queries.
///
/// `Query[Transform, With[Velocity]]` would create a `PyQueryParam` with
/// `component_types` containing `Transform` and `filters` containing `With[Velocity]
#[pyclass(name = "QueryParam", frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyQueryParam {
    /// The component types in the query parameter.
    pub(crate) data: SmallVec<[QueryData; 16]>,
    /// Indicates if the query is for a single component type only.
    pub(crate) single: bool,
    /// The filters in the query parameter.
    pub(crate) filters: SmallVec<[QueryFilter; 4]>,
    /// Indicates if the query enforces exactly one entity (Single<T>).
    pub(crate) single_entity_enforced: bool,
}

impl PyQueryParam {
    pub(crate) fn _param_types(&self) -> &[QueryData] {
        &self.data
    }
}

#[pymethods]
impl PyQueryParam {
    /// Returns a list of component type names in the query parameter.
    #[getter]
    pub fn _data_names(&self) -> Vec<String> {
        self.data.iter().map(|ct| ct.to_string()).collect()
    }

    /// Returns the number of component types in the query parameter.
    pub fn _data_len(&self) -> usize {
        self.data.len()
    }

    /// Returns the number of filters in the query parameter.
    pub fn _filter_len(&self) -> usize {
        self.filters.len()
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
    },
}

impl fmt::Display for QueryData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryData::Entity => write!(f, "Entity"),
            QueryData::Component {
                ty,
                mutable,
                optional,
            } => {
                let inner = if *mutable {
                    format!("Mut[{ty}]")
                } else {
                    format!("{ty}")
                };
                if *optional {
                    write!(f, "Optional[{inner}]")
                } else {
                    write!(f, "{inner}")
                }
            }
        }
    }
}
