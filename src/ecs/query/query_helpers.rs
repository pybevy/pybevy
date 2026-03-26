use pyo3::{
    IntoPyObjectExt, PyTypeInfo,
    exceptions::PyTypeError,
    prelude::*,
    types::{PyGenericAlias, PyTuple, PyType},
};
use smallvec::{SmallVec, smallvec};

use crate::ecs::{
    PyEntity,
    component_type::PyComponentType,
    filter::{
        QueryFilter,
        filters::{PyAdded, PyAnyOf, PyChanged, PyHas, PyWith, PyWithout},
    },
    mutable::PyMut,
    query::{
        ParamType,
        query_param::{PyQueryParam, QueryData},
    },
};

pub(crate) fn extract_param_type_from_query_param(
    py: Python,
    key: &Bound<'_, PyAny>,
) -> Result<(bool, SmallVec<[ParamType; 2]>), PyErr> {
    // Check for Optional[T] / T | None FIRST
    if let Some(inner) = extract_optional_inner(py, key)? {
        // Reject Optional[Entity]
        if inner.is(PyEntity::type_object(py)) {
            return Err(PyTypeError::new_err(
                "Optional[Entity] is not supported. Entity is always present on every entity.",
            ));
        }

        let (mutable, component_type) = extract_component_with_mutability(py, &inner)?;
        return ok_single(ParamType::Component {
            ty: component_type,
            mutable,
            optional: true,
        });
    }

    // Check for Mut[Component] FIRST before generic tuple check
    // because Mut[T] is also a GenericAlias but should be handled differently
    if let Ok(origin) = key.getattr("__origin__")
        && origin.is(PyMut::type_object(py)) {
            // This is Mut[Component] - extract with mutability
            let (mutable, component_type) = extract_component_with_mutability(py, key)?;

            return ok_single(ParamType::Component {
                ty: component_type,
                mutable,
                optional: false,
            });
        }

    #[inline(always)]
    fn ok_single(param: ParamType) -> Result<(bool, SmallVec<[ParamType; 2]>), PyErr> {
        Ok((true, smallvec![param]))
    }

    if key.is_instance_of::<PyGenericAlias>() {
        // tuple[.., ...]
        let generic_alias = key.cast::<PyGenericAlias>()?;
        let args = generic_alias.getattr("__args__")?;

        let mut values = SmallVec::<[ParamType; 2]>::new();
        for arg in args.cast::<PyTuple>()?.iter() {
            let (_, new_values) = extract_param_type_from_query_param(py, &arg)?;
            values.extend(new_values);
        }

        Ok((false, values))
    } else if key.is_instance_of::<PyWith>() {
        ok_single(QueryFilter::With(key.extract::<PyWith>()?).into())
    } else if key.is_instance_of::<PyWithout>() {
        ok_single(QueryFilter::Without(key.extract::<PyWithout>()?).into())
    } else if key.is_instance_of::<PyChanged>() {
        ok_single(QueryFilter::Changed(key.extract::<PyChanged>()?).into())
    } else if key.is_instance_of::<PyAdded>() {
        ok_single(QueryFilter::Added(key.extract::<PyAdded>()?).into())
    } else if key.is_instance_of::<PyHas>() {
        ok_single(QueryFilter::Has(key.extract::<PyHas>()?).into())
    } else if key.is_instance_of::<PyAnyOf>() {
        ok_single(QueryFilter::AnyOf(key.extract::<PyAnyOf>()?).into())
    } else if key.is(PyEntity::type_object(py)) {
        ok_single(ParamType::Entity)
    } else {
        // Plain component type (not wrapped in Mut[])
        let (mutable, component_type) = extract_component_with_mutability(py, key)?;
        ok_single(ParamType::Component {
            ty: component_type,
            mutable,
            optional: false,
        })
    }
}

/// Extract component type and check if it's wrapped in Mut[]
fn extract_component_with_mutability(
    py: Python,
    key: &Bound<'_, PyAny>,
) -> Result<(bool, PyComponentType), PyErr> {
    // Check if this is a generic alias (Mut[Component]) by checking for __origin__ attribute
    let (mutable, type_key) = if let Ok(origin) = key.getattr("__origin__") {
        if origin.is(PyMut::type_object(py)) {
            // This is Mut[Component] - extract the inner component type
            let args = key.getattr("__args__")?;
            let inner = args.get_item(0)?;
            (true, inner)
        } else {
            (false, key.clone())
        }
    } else {
        // Not wrapped in Mut[] - it's read-only
        (false, key.clone())
    };

    let comp_type = PyComponentType::try_from((type_key.cast::<PyType>()?, py))?;
    Ok((mutable, comp_type))
}

pub(crate) fn extract_component_type_from_query_param(
    py: Python,
    key: &Bound<'_, PyAny>,
) -> Result<PyComponentType, PyErr> {
    let (_mutable, comp_type) = extract_component_with_mutability(py, key)?;
    Ok(comp_type)
}

/// Check if a type annotation is Optional[T] (typing.Union[T, NoneType]) or T | None (PEP 604).
/// Returns Some(inner_type) if it's optional, None otherwise.
fn extract_optional_inner<'py>(
    py: Python<'py>,
    key: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    // Check typing.Union (Optional[T] desugars to Union[T, None])
    if let Ok(origin) = key.getattr("__origin__") {
        let typing = py.import("typing")?;
        let union = typing.getattr("Union")?;
        if origin.is(&union) {
            return extract_non_none_from_union_args(py, key);
        }
    }

    // Check PEP 604: T | None (types.UnionType)
    let types_mod = py.import("types")?;
    if let Ok(union_type_any) = types_mod.getattr("UnionType")
        && let Ok(union_type) = union_type_any.cast::<PyType>()
            && key.is_instance(union_type)? {
                return extract_non_none_from_union_args(py, key);
            }

    Ok(None)
}

/// Extract the non-None type from a Union's __args__ tuple.
/// Returns Some(inner_type) if exactly one of the two args is NoneType.
fn extract_non_none_from_union_args<'py>(
    py: Python<'py>,
    key: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let args = key.getattr("__args__")?;
    let args_tuple = args.cast::<PyTuple>()?;
    if args_tuple.len() != 2 {
        return Ok(None);
    }

    let none_type = py.None().bind(py).get_type();
    if args_tuple.get_item(1)?.is(&none_type) {
        Ok(Some(args_tuple.get_item(0)?))
    } else if args_tuple.get_item(0)?.is(&none_type) {
        Ok(Some(args_tuple.get_item(1)?))
    } else {
        Ok(None)
    }
}

/// Constructs a Query class item, handling both single and multiple parameters.
pub(crate) fn construct_query_class_item(
    cls: &Bound<'_, PyType>,
    key: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    construct_query_class_item_with_options(cls, key, false)
}

/// Constructs a Query class item with optional Single<T> enforcement.
pub(crate) fn construct_query_class_item_with_options(
    cls: &Bound<'_, PyType>,
    key: &Bound<'_, PyAny>,
    single_entity_enforced: bool,
) -> PyResult<Py<PyAny>> {
    let py = cls.py();

    let (single, param_types) = match key.try_iter() {
        Ok(iter) => {
            let mut single = true;

            // multiple parameters
            let mut components = Vec::new();
            let mut count = 0;
            for item in iter {
                let item = item?;
                let (_single, new_components) = extract_param_type_from_query_param(py, &item)?;
                components.extend(new_components);

                count += 1;

                let is_filter = item.is_instance_of::<PyWith>()
                    || item.is_instance_of::<PyWithout>()
                    || item.is_instance_of::<PyChanged>()
                    || item.is_instance_of::<PyAdded>()
                    || item.is_instance_of::<PyHas>()
                    || item.is_instance_of::<PyAnyOf>();

                // Check if this is a tuple of filters
                let is_filter_tuple = if item.is_instance_of::<PyGenericAlias>() {
                    // Check if it's tuple[...] by looking at __origin__
                    if let Ok(origin) = item.getattr("__origin__") {
                        if origin.is(py.get_type::<pyo3::types::PyTuple>()) {
                            // It's a tuple - check if all elements are filters
                            if let Ok(args) = item.getattr("__args__") {
                                if let Ok(tuple_args) = args.cast::<PyTuple>() {
                                    tuple_args.iter().all(|arg| {
                                        arg.is_instance_of::<PyWith>()
                                            || arg.is_instance_of::<PyWithout>()
                                            || arg.is_instance_of::<PyChanged>()
                                            || arg.is_instance_of::<PyAdded>()
                                            || arg.is_instance_of::<PyHas>()
                                            || arg.is_instance_of::<PyAnyOf>()
                                    })
                                } else {
                                    false
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                // Only set single=false for component tuples, not filter tuples
                // Filter tuples don't affect the return type (still single component)
                if !_single && !is_filter_tuple {
                    single = false;
                }

                if (count == 2 && !is_filter && !is_filter_tuple) || count > 2 {
                    return Err(PyTypeError::new_err(
                        "Invalid Query. To query multiple components, use a tuple: Query[tuple[Component1, Component2, ...]]",
                    ));
                }
            }

            (single, components)
        }

        Err(_) => {
            // single parameter
            let (single, params) = extract_param_type_from_query_param(py, key)?;
            (single, params.into_vec())
        }
    };

    PyQueryParam {
        data: param_types
            .iter()
            .filter_map(|param| match param {
                ParamType::Entity => Some(QueryData::Entity),
                ParamType::Component {
                    ty: comp_type,
                    mutable,
                    optional,
                } => Some(QueryData::Component {
                    ty: comp_type.clone(),
                    mutable: *mutable,
                    optional: *optional,
                }),
                ParamType::Filter(_) => None,
            })
            .collect(),
        filters: param_types
            .into_iter()
            .filter_map(|param| match param {
                ParamType::Filter(filter) => Some(filter),
                ParamType::Entity | ParamType::Component { .. } => None,
            })
            .collect(),
        single,
        single_entity_enforced,
    }
    .into_py_any(py)
}
