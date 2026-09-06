use pybevy_core::public_error::{
    ANY_OF_VIEW_UNSUPPORTED, OR_VIEW_UNSUPPORTED, RESOURCE_VIEW_DATA, RESOURCE_VIEW_FILTER,
};
use pyo3::{IntoPyObjectExt, exceptions::PyRuntimeError, prelude::*, types::PyType};
use smallvec::SmallVec;

use crate::ecs::{
    component_type::PyComponentType,
    filter::QueryFilter,
    query::{ParamType, query_helpers::extract_param_type_from_query_param},
};

pub mod cached_view;
pub mod view;
pub mod view_column;
pub mod view_param;

use view_param::{PyViewParam, ViewParamType};

/// Constructs a View class item, handling both single and multiple parameters.
///
/// View syntax (consistent with Query):
/// - `View[Mut[Transform]]` - Only mutable Transform component
/// - `View[Mut[Transform], With[Cube]]` - Mutable Transform with Cube filter
/// - `View[Mut[Transform], Without[Moving]]` - Transform excluding Moving entities
/// - `View[Mut[Transform], tuple[With[Cube], Without[Moving]]]` - Transform with multiple filters
///
/// In View, Mut[T] parameters are data components. Filters must be explicit (With, Without, etc.).
pub(crate) fn construct_view_class_item(
    cls: &Bound<'_, PyType>,
    key: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let py = cls.py();

    let param_types = match key.try_iter() {
        Ok(iter) => {
            // Multiple parameters (tuple)
            let mut components = Vec::new();
            for item in iter {
                let item = item?;
                let (_single, new_components) = extract_param_type_from_query_param(py, &item)?;
                components.extend(new_components);
            }
            components
        }

        Err(_) => {
            // Single parameter
            let (_single, params) = extract_param_type_from_query_param(py, key)?;
            params.into_vec()
        }
    };

    // In View: Mut[T] → mutable component, T → read-only component, With[T]/Without[T] → filter
    let mut components = SmallVec::new();
    let mut with_filters: SmallVec<[PyComponentType; 4]> = SmallVec::new();
    let mut without_filters: SmallVec<[PyComponentType; 4]> = SmallVec::new();
    let mut changed_filters: SmallVec<[PyComponentType; 4]> = SmallVec::new();
    let mut added_filters: SmallVec<[PyComponentType; 4]> = SmallVec::new();
    let mut has_entity = false;

    for param in param_types {
        match param {
            ParamType::Component {
                ty: comp_type,
                mutable,
                optional,
                logical_type_id,
            } => {
                if matches!(comp_type, PyComponentType::Resource(_)) {
                    return Err(PyRuntimeError::new_err(RESOURCE_VIEW_DATA));
                }
                if logical_type_id.is_some() {
                    return Err(PyRuntimeError::new_err(
                        "Logical component specializations are supported by Query, not View",
                    ));
                }
                if optional {
                    return Err(PyRuntimeError::new_err(
                        "Optional[T] is not supported in View. Use Query[tuple[T, Optional[U]]] instead.",
                    ));
                }
                components.push(ViewParamType::Component { comp_type, mutable });
            }
            ParamType::Filter(filter) => match filter {
                QueryFilter::With(with) => {
                    if with
                        .values
                        .iter()
                        .any(|ty| matches!(ty, PyComponentType::Resource(_)))
                    {
                        return Err(PyRuntimeError::new_err(RESOURCE_VIEW_FILTER));
                    }
                    with_filters.extend(with.values.iter().cloned());
                }
                QueryFilter::Without(without) => {
                    if without
                        .values
                        .iter()
                        .any(|ty| matches!(ty, PyComponentType::Resource(_)))
                    {
                        return Err(PyRuntimeError::new_err(RESOURCE_VIEW_FILTER));
                    }
                    without_filters.extend(without.values.iter().cloned());
                }
                QueryFilter::Changed(changed) => {
                    if matches!(changed.component_type, PyComponentType::Resource(_)) {
                        return Err(PyRuntimeError::new_err(RESOURCE_VIEW_FILTER));
                    }
                    changed_filters.push(changed.component_type);
                }
                QueryFilter::Added(added) => {
                    if matches!(added.component_type, PyComponentType::Resource(_)) {
                        return Err(PyRuntimeError::new_err(RESOURCE_VIEW_FILTER));
                    }
                    added_filters.push(added.component_type);
                }
                QueryFilter::Or(_) => {
                    return Err(PyRuntimeError::new_err(OR_VIEW_UNSUPPORTED));
                }
            },
            ParamType::Has(_) => {
                return Err(PyRuntimeError::new_err(
                    "Has[T] query data is not supported in View. Use Query for per-entity Has checks.",
                ));
            }
            ParamType::AnyOf(_) => {
                return Err(PyRuntimeError::new_err(ANY_OF_VIEW_UNSUPPORTED));
            }
            ParamType::Entity => {
                has_entity = true;
            }
        }
    }

    PyViewParam {
        retained_types: PyViewParam::retain_custom_types(
            py,
            &components,
            [
                &with_filters,
                &without_filters,
                &changed_filters,
                &added_filters,
            ],
        ),
        parameters: components,
        with_filters,
        without_filters,
        changed_filters,
        added_filters,
        has_entity,
    }
    .into_py_any(py)
}
