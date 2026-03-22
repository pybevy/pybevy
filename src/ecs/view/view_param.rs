use core::fmt;

use pyo3::prelude::*;
use smallvec::SmallVec;

use crate::ecs::component_type::PyComponentType;

/// View parameters for ECS View API.
///
/// `View[Mut[Transform], Velocity]` would create a `PyViewParam` with
/// component types containing `Transform` (mutable) and `Velocity` (read-only)
#[pyclass(name = "ViewParam", frozen)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyViewParam {
    /// The component types in the view parameter with their mutability
    pub(crate) parameters: SmallVec<[ViewParamType; 8]>,
    /// Include filter component types (With[T] pattern)
    pub(crate) with_filters: SmallVec<[PyComponentType; 4]>,
    /// Exclude filter component types (Without[T] pattern)
    pub(crate) without_filters: SmallVec<[PyComponentType; 4]>,
    /// Changed filter component types (Changed[T] pattern)
    pub(crate) changed_filters: SmallVec<[PyComponentType; 4]>,
    /// Added filter component types (Added[T] pattern)
    pub(crate) added_filters: SmallVec<[PyComponentType; 4]>,
    /// Whether Entity was requested in the View type parameters
    pub(crate) has_entity: bool,
}

impl PyViewParam {
    #[allow(dead_code)] // cross-crate API
    pub(crate) fn param_types(&self) -> &[ViewParamType] {
        &self.parameters
    }
}

#[pymethods]
impl PyViewParam {
    pub fn _data_len(&self) -> usize {
        self.parameters.len()
    }

    #[getter]
    pub fn _data_names(&self) -> Vec<String> {
        self.parameters.iter().map(|ct| ct.to_string()).collect()
    }

    #[getter]
    pub fn _filter_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .with_filters
            .iter()
            .map(|ct| format!("With[{ct}]"))
            .collect();
        names.extend(
            self.without_filters
                .iter()
                .map(|ct| format!("Without[{ct}]")),
        );
        names.extend(
            self.changed_filters
                .iter()
                .map(|ct| format!("Changed[{ct}]")),
        );
        names.extend(self.added_filters.iter().map(|ct| format!("Added[{ct}]")));
        names
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ViewParamType {
    Component {
        comp_type: PyComponentType,
        mutable: bool,
    },
}

impl fmt::Display for ViewParamType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ViewParamType::Component { comp_type, mutable } => {
                if *mutable {
                    write!(f, "Mut[{comp_type}]")
                } else {
                    write!(f, "{comp_type}")
                }
            }
        }
    }
}
