use pybevy_core::{
    LogicalTypeId, PyLogicalComponentParam,
    public_error::{
        ANY_OF_EMPTY, ANY_OF_TUPLE_REQUIRED, OR_IS_FILTER, parenthesized_filter_tuple,
        query_data_required, variadic_filters,
    },
    registry::global_registry,
};
use pybevy_ecs::shared::query_runtime::QueryRowShape;
use pyo3::{
    IntoPyObjectExt, PyTypeInfo,
    exceptions::PyTypeError,
    prelude::*,
    types::{PyGenericAlias, PyTuple, PyType},
};
use smallvec::{SmallVec, smallvec};

use crate::{
    assets::asset_type::PyAssetTypeParam,
    ecs::{
        PyEntity,
        component_type::PyComponentType,
        filter::{
            QueryFilter,
            filters::{PyAdded, PyAnyOf, PyChanged, PyHas, PyOr, PyWith, PyWithout},
        },
        mutable::PyMut,
        query::{
            ParamType,
            query_param::{AnyOfItem, PyQueryParam, QueryData},
        },
    },
};

type ExtractedAssetResource = (
    bool,
    Py<PyAssetTypeParam>,
    *const pyo3::ffi::PyTypeObject,
    String,
);

fn extract_asset_resource(
    py: Python<'_>,
    key: &Bound<'_, PyAny>,
) -> PyResult<Option<ExtractedAssetResource>> {
    let (mutable, type_key) = if let Ok(origin) = key.getattr("__origin__")
        && origin.is(PyMut::type_object(py))
    {
        (true, key.getattr("__args__")?.get_item(0)?)
    } else {
        (false, key.clone())
    };
    if !type_key.get_type().is(PyAssetTypeParam::type_object(py)) {
        return Ok(None);
    }
    let retained = type_key.clone().cast_into::<PyAssetTypeParam>()?.unbind();
    let param = retained.borrow(py);
    let type_ptr = param.type_ptr();
    let name = param
        .logical_type_name()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            global_registry::get_asset_bridge_by_py_type(type_ptr)
                .map_or_else(|| "asset".to_owned(), |bridge| bridge.name().to_owned())
        });
    drop(param);
    Ok(Some((mutable, retained, type_ptr, name)))
}

fn asset_param_type(
    py: Python<'_>,
    key: &Bound<'_, PyAny>,
    optional: bool,
) -> PyResult<Option<ParamType>> {
    Ok(
        extract_asset_resource(py, key)?.map(|(mutable, param, type_ptr, name)| {
            ParamType::AssetResource {
                param,
                type_ptr,
                name,
                mutable,
                optional,
            }
        }),
    )
}

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

        if let Some(asset) = asset_param_type(py, &inner, true)? {
            return ok_single(asset);
        }
        let (mutable, component_type, logical_type_id) =
            extract_component_with_mutability(py, &inner)?;
        return ok_single(ParamType::Component {
            ty: component_type,
            mutable,
            optional: true,
            logical_type_id,
        });
    }

    // Check for Mut[Component] FIRST before generic tuple check
    // because Mut[T] is also a GenericAlias but should be handled differently
    if let Ok(origin) = key.getattr("__origin__")
        && origin.is(PyMut::type_object(py))
    {
        if let Some(asset) = asset_param_type(py, key, false)? {
            return ok_single(asset);
        }
        // This is Mut[Component] - extract with mutability
        let (mutable, component_type, logical_type_id) =
            extract_component_with_mutability(py, key)?;

        return ok_single(ParamType::Component {
            ty: component_type,
            mutable,
            optional: false,
            logical_type_id,
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
        let mut contains_or_filter = false;
        for arg in args.cast::<PyTuple>()?.iter() {
            contains_or_filter |= arg.is_instance_of::<PyOr>();
            let (_, new_values) = extract_param_type_from_query_param(py, &arg)?;
            values.extend(new_values);
        }

        if contains_or_filter
            && values
                .iter()
                .any(|value| !matches!(value, ParamType::Filter(_)))
        {
            return Err(PyTypeError::new_err(OR_IS_FILTER));
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
        ok_single(ParamType::Has(key.extract::<PyHas>()?))
    } else if key.is_instance_of::<PyAnyOf>() {
        ok_single(ParamType::AnyOf(key.extract::<PyAnyOf>()?.items))
    } else if key.is_instance_of::<PyOr>() {
        ok_single(QueryFilter::Or(key.extract::<PyOr>()?).into())
    } else if key.is(PyEntity::type_object(py)) {
        ok_single(ParamType::Entity)
    } else {
        if let Some(asset) = asset_param_type(py, key, false)? {
            return ok_single(asset);
        }
        // Plain component type (not wrapped in Mut[])
        let (mutable, component_type, logical_type_id) =
            extract_component_with_mutability(py, key)?;
        ok_single(ParamType::Component {
            ty: component_type,
            mutable,
            optional: false,
            logical_type_id,
        })
    }
}

fn query_row_shape(py: Python<'_>, key: &Bound<'_, PyAny>) -> PyResult<Option<QueryRowShape>> {
    if key.is_instance_of::<PyWith>()
        || key.is_instance_of::<PyWithout>()
        || key.is_instance_of::<PyChanged>()
        || key.is_instance_of::<PyAdded>()
        || key.is_instance_of::<PyOr>()
    {
        return Ok(None);
    }

    if key.is_instance_of::<PyGenericAlias>()
        && key.getattr("__origin__")?.is(py.get_type::<PyTuple>())
    {
        let args = key.getattr("__args__")?.cast_into::<PyTuple>()?;
        let arg_count = args.len();
        let children = args
            .iter()
            .map(|arg| query_row_shape(py, &arg))
            .collect::<PyResult<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if arg_count != 0 && children.is_empty() {
            return Ok(None);
        }
        return Ok(Some(QueryRowShape::Tuple(children)));
    }

    Ok(Some(QueryRowShape::Item))
}

/// Extract component type and check if it's wrapped in Mut[]
pub(crate) fn extract_component_with_mutability(
    py: Python,
    key: &Bound<'_, PyAny>,
) -> Result<(bool, PyComponentType, Option<LogicalTypeId>), PyErr> {
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

    if let Ok(logical) = type_key.extract::<PyRef<'_, PyLogicalComponentParam>>() {
        return Ok((
            mutable,
            PyComponentType::Dynamic(logical.component_type_ptr()),
            Some(logical.logical_type_id()),
        ));
    }

    let comp_type = PyComponentType::try_from((type_key.cast::<PyType>()?, py))?;
    if mutable && !comp_type.supports_mutable_access() {
        return Err(pyo3::exceptions::PyTypeError::new_err(format!(
            "{} is a read-only resource and cannot be queried through Mut",
            comp_type.display_name(py)
        )));
    }
    Ok((mutable, comp_type, None))
}

pub(crate) fn parse_anyof_items(
    py: Python<'_>,
    key: &Bound<'_, PyAny>,
) -> PyResult<SmallVec<[AnyOfItem; 4]>> {
    if key.is_instance_of::<PyTuple>() {
        return Err(PyTypeError::new_err(ANY_OF_TUPLE_REQUIRED));
    }
    let origin = key
        .getattr("__origin__")
        .map_err(|_| PyTypeError::new_err(ANY_OF_TUPLE_REQUIRED))?;
    if !origin.is(py.get_type::<PyTuple>()) {
        return Err(PyTypeError::new_err(ANY_OF_TUPLE_REQUIRED));
    }
    let args = key.getattr("__args__")?.cast_into::<PyTuple>()?;
    if args.is_empty() {
        return Err(PyTypeError::new_err(ANY_OF_EMPTY));
    }
    args.iter()
        .map(|item| {
            let (mutable, ty, logical_type_id) = extract_component_with_mutability(py, &item)?;
            Ok(AnyOfItem {
                ty,
                mutable,
                logical_type_id,
            })
        })
        .collect()
}

pub(crate) fn extract_component_type_from_query_param(
    py: Python,
    key: &Bound<'_, PyAny>,
) -> Result<PyComponentType, PyErr> {
    let (_mutable, comp_type, logical_type_id) = extract_component_with_mutability(py, key)?;
    if logical_type_id.is_some() {
        return Err(PyTypeError::new_err(
            "Logical component specializations must be Query data; they are not supported inside component filters",
        ));
    }
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
        && key.is_instance(union_type)?
    {
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

    if let Ok(items) = key.cast::<PyTuple>() {
        if items
            .iter()
            .skip(1)
            .any(|item| is_parenthesized_filter_tuple(&item))
        {
            return Err(PyTypeError::new_err(parenthesized_filter_tuple(
                if single_entity_enforced {
                    "Single"
                } else {
                    "Query"
                },
            )));
        }
        if items.len() > 2
            && items
                .iter()
                .skip(1)
                .all(|item| is_filter_position_argument(py, &item))
        {
            return Err(PyTypeError::new_err(variadic_filters(
                if single_entity_enforced {
                    "Single"
                } else {
                    "Query"
                },
            )));
        }
    }

    let row_shape = if let Ok(items) = key.cast::<PyTuple>() {
        let shapes = items
            .iter()
            .map(|item| query_row_shape(py, &item))
            .collect::<PyResult<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if shapes.len() == 1 {
            shapes.into_iter().next().expect("length checked")
        } else {
            QueryRowShape::Tuple(shapes)
        }
    } else {
        query_row_shape(py, key)?.unwrap_or_else(|| QueryRowShape::Tuple(Vec::new()))
    };

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

                let is_filter = is_query_filter(&item);
                let is_filter_tuple = is_filter_tuple_alias(py, &item);

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

    let mut data = SmallVec::<[QueryData; 16]>::new();
    let mut filters = SmallVec::<[QueryFilter; 4]>::new();
    let mut retained_asset_params = SmallVec::new();
    for param in param_types {
        match param {
            ParamType::Entity => data.push(QueryData::Entity),
            ParamType::Component {
                ty: comp_type,
                mutable,
                optional,
                logical_type_id,
            } => data.push(QueryData::Component {
                ty: comp_type,
                mutable,
                optional,
                logical_type_id,
            }),
            ParamType::AssetResource {
                param,
                type_ptr,
                name,
                mutable,
                optional,
            } => {
                let retained_param_index = retained_asset_params.len();
                retained_asset_params.push(param);
                data.push(QueryData::AssetResource {
                    type_ptr,
                    retained_param_index,
                    name,
                    mutable,
                    optional,
                });
            }
            ParamType::Has(has) => data.push(QueryData::Has {
                ty: has.component_type,
            }),
            ParamType::AnyOf(items) => data.push(QueryData::AnyOf { items }),
            ParamType::Filter(filter) => filters.push(filter),
        }
    }

    for (index, asset) in data.iter().enumerate() {
        let QueryData::AssetResource {
            type_ptr,
            mutable,
            name,
            ..
        } = asset
        else {
            continue;
        };
        if let Some((previous, previous_mutable)) = data[..index].iter().find_map(|item| match item
        {
            QueryData::AssetResource {
                type_ptr: previous,
                name,
                mutable,
                ..
            } if previous == type_ptr => Some((name, *mutable)),
            _ => None,
        }) && (*mutable || previous_mutable)
        {
            return Err(PyTypeError::new_err(format!(
                "Query requests aliased mutable access to Assets[{name}] and Assets[{previous}]"
            )));
        }
    }

    let single = single && (!single_entity_enforced || !data.is_empty());

    if data.is_empty() && single {
        let kind = if single_entity_enforced {
            "Single"
        } else {
            "Query"
        };
        return Err(PyTypeError::new_err(filter_only_error_message(
            py, kind, &filters,
        )));
    }

    PyQueryParam {
        retained_types: PyQueryParam::retain_custom_types(py, &data, &filters),
        retained_asset_params,
        data,
        row_shape,
        filters,
        single,
        single_entity_enforced,
        optional_single: false,
    }
    .into_py_any(py)
}

fn is_query_filter(item: &Bound<'_, PyAny>) -> bool {
    item.is_instance_of::<PyWith>()
        || item.is_instance_of::<PyWithout>()
        || item.is_instance_of::<PyChanged>()
        || item.is_instance_of::<PyAdded>()
        || item.is_instance_of::<PyOr>()
}

fn is_filter_tuple_alias(py: Python<'_>, item: &Bound<'_, PyAny>) -> bool {
    if !item.is_instance_of::<PyGenericAlias>() {
        return false;
    }
    let Ok(origin) = item.getattr("__origin__") else {
        return false;
    };
    if !origin.is(py.get_type::<PyTuple>()) {
        return false;
    }
    let Ok(args) = item.getattr("__args__") else {
        return false;
    };
    let Ok(tuple_args) = args.cast::<PyTuple>() else {
        return false;
    };
    !tuple_args.is_empty() && tuple_args.iter().all(|arg| is_query_filter(&arg))
}

fn is_filter_position_argument(py: Python<'_>, item: &Bound<'_, PyAny>) -> bool {
    is_query_filter(item) || is_filter_tuple_alias(py, item)
}

pub(crate) fn is_parenthesized_filter_tuple(item: &Bound<'_, PyAny>) -> bool {
    let Ok(items) = item.cast::<PyTuple>() else {
        return false;
    };
    !items.is_empty() && items.iter().all(|item| is_query_filter(&item))
}

/// Names a query filter in the spelling users write, for error text.
fn filter_label(py: Python, filter: &QueryFilter) -> String {
    match filter {
        QueryFilter::With(with) => {
            let names = with
                .values
                .iter()
                .map(|ty| ty.display_name(py))
                .collect::<Vec<_>>();
            format!("With[{}]", names.join(", "))
        }
        QueryFilter::Without(without) => {
            let names = without
                .values
                .iter()
                .map(|ty| ty.display_name(py))
                .collect::<Vec<_>>();
            format!("Without[{}]", names.join(", "))
        }
        QueryFilter::Changed(changed) => {
            format!("Changed[{}]", changed.component_type.display_name(py))
        }
        QueryFilter::Added(added) => {
            format!("Added[{}]", added.component_type.display_name(py))
        }
        QueryFilter::Or(ort) => {
            let inner = ort
                .values
                .iter()
                .map(|filter| filter_label(py, filter))
                .collect::<Vec<_>>();
            format!("Or[tuple[{}]]", inner.join(", "))
        }
    }
}

/// Builds the stable error text for a descriptor whose filters occupy the
/// data position, leaving no data slots.
pub(crate) fn filter_only_error_message(py: Python, kind: &str, filters: &[QueryFilter]) -> String {
    let labels: Vec<String> = filters
        .iter()
        .map(|filter| filter_label(py, filter))
        .collect();
    let shape = if labels.is_empty() {
        format!("{kind}[()]")
    } else {
        format!("{kind}[{}]", labels.join(", "))
    };
    query_data_required(kind, &shape, &labels)
}
