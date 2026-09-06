use std::sync::Arc;

use pyo3::{PyTraverseError, PyVisit, prelude::*, types::PyType};
use smallvec::SmallVec;

use crate::ecs::component_type::{PyComponentType, clone_retained_classes, retain_custom_classes};

/// View parameters for ECS View API.
///
/// `View[Mut[Transform], Velocity]` would create a `PyViewParam` with
/// component types containing `Transform` (mutable) and `Velocity` (read-only)
#[pyclass(name = "ViewParam", module = "pybevy.ecs", frozen, from_py_object)]
#[derive(Debug)]
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
    /// Strong references backing the `PyComponentType::Custom` pointers above.
    ///
    /// Without these a `@component` class can be freed while this param still
    /// names it, and column resolution then dereferences a stale type object.
    pub(crate) retained_types: SmallVec<[Arc<Py<PyType>>; 4]>,
}

impl PyViewParam {
    #[allow(dead_code)] // cross-crate API
    pub(crate) fn param_types(&self) -> &[ViewParamType] {
        &self.parameters
    }

    /// Retain every custom class this param names; see
    /// `component_type::retain_custom_classes` and docs/safety.md section 6.
    pub(crate) fn retain_custom_types(
        py: Python<'_>,
        parameters: &[ViewParamType],
        filters: [&[PyComponentType]; 4],
    ) -> SmallVec<[Arc<Py<PyType>>; 4]> {
        let from_data = parameters.iter().map(|param| match param {
            ViewParamType::Component { comp_type, .. } => *comp_type,
        });
        let from_filters = filters.into_iter().flatten().copied();
        retain_custom_classes(py, from_data.chain(from_filters))
    }
}

/// Deep-clones `retained_types` so every holder owns independent increfs; a
/// shared handle visited by two traversing owners is a use-after-free.
impl Clone for PyViewParam {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            parameters: self.parameters.clone(),
            with_filters: self.with_filters.clone(),
            without_filters: self.without_filters.clone(),
            changed_filters: self.changed_filters.clone(),
            added_filters: self.added_filters.clone(),
            has_entity: self.has_entity,
            retained_types: clone_retained_classes(py, &self.retained_types),
        })
    }
}

/// `retained_types` is derived from the parameters and filters, so it carries
/// no identity of its own and is excluded from equality.
impl PartialEq for PyViewParam {
    fn eq(&self, other: &Self) -> bool {
        self.parameters == other.parameters
            && self.with_filters == other.with_filters
            && self.without_filters == other.without_filters
            && self.changed_filters == other.changed_filters
            && self.added_filters == other.added_filters
            && self.has_entity == other.has_entity
    }
}

#[pymethods]
impl PyViewParam {
    /// Report the retained classes to the cyclic GC; see docs/safety.md.
    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        for class in &self.retained_types {
            visit.call(class.as_ref())?;
        }
        Ok(())
    }

    pub fn _data_len(&self) -> usize {
        self.parameters.len()
    }

    #[getter]
    pub fn _data_names(&self, py: Python<'_>) -> Vec<String> {
        self.parameters
            .iter()
            .map(|param| param.display_name(py))
            .collect()
    }

    #[getter]
    pub fn _filter_names(&self, py: Python<'_>) -> Vec<String> {
        let mut names: Vec<String> = self
            .with_filters
            .iter()
            .map(|ct| format!("With[{}]", ct.display_name(py)))
            .collect();
        names.extend(
            self.without_filters
                .iter()
                .map(|ct| format!("Without[{}]", ct.display_name(py))),
        );
        names.extend(
            self.changed_filters
                .iter()
                .map(|ct| format!("Changed[{}]", ct.display_name(py))),
        );
        names.extend(
            self.added_filters
                .iter()
                .map(|ct| format!("Added[{}]", ct.display_name(py))),
        );
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

impl ViewParamType {
    fn display_name(&self, py: Python<'_>) -> String {
        match self {
            ViewParamType::Component { comp_type, mutable } => {
                if *mutable {
                    format!("Mut[{}]", comp_type.display_name(py))
                } else {
                    comp_type.display_name(py)
                }
            }
        }
    }
}
