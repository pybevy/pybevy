use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use pybevy_core::{
    LogicalTypeId, PyAsset, PyComponent, PyMessage,
    public_error::{
        SystemParamRejection, SystemParamWrapper, pipe_input_must_be_first,
        pipe_target_requires_input, system_annotations_unresolved, system_param_rejection_message,
    },
};
use pybevy_gizmos::gizmos::PyGizmos;
use pyo3::{
    PyTypeInfo,
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyAny, PyTuple, PyType},
};
use smallvec::SmallVec;

use super::{
    commands::PyCommands,
    local::PyLocal,
    message::MessageTypeParam,
    mutable::PyMut,
    resource::{PyRes, PyResMut, PyResource},
    system_input::PyInParam,
    world::PyWorld,
};
use crate::{
    assets::asset_type::PyAssetTypeParam,
    ecs::{
        component_type::PyComponentType,
        messages::{MessageType, PyMessageType},
        observer::EventType,
        query::{query_param::PyQueryParam, single::PySingle},
        resource_type::{reject_state_type_as_resource, reject_unparameterized_button_input},
        view::{view::PyView, view_param::PyViewParam},
    },
};

const STACK_PARAMS: usize = 8;

/// Global cache for parsed system parameters to avoid re-parsing the same function
/// Key: Python function pointer address (as usize)
/// Value: Parsed parameters
/// Cache entry: (code_object_ptr, params).
///
/// We store the `__code__` object address alongside the params so we can detect
/// address reuse: if CPython recycles a function's memory for a new closure with
/// different parameters, the `__code__` pointer will differ and we re-parse.
type SystemParamCacheEntry = (usize, Arc<SmallVec<[SystemParam; STACK_PARAMS]>>);
type SystemParamCache = HashMap<usize, SystemParamCacheEntry>;
static SYSTEM_PARAM_CACHE: Mutex<Option<SystemParamCache>> = Mutex::new(None);

/// Represents a pythonic system function with its parameters cached for efficient calls
#[derive(Debug)]
pub struct SystemFunction {
    /// The python function to call
    pub func: Py<PyAny>,

    /// The parameters of the system function
    pub params: SmallVec<[SystemParam; STACK_PARAMS]>,
}

impl Clone for SystemFunction {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            func: self.func.clone_ref(py),
            params: self.params.clone(),
        })
    }
}

/// The annotated class, or `None` for a generic alias or any other non-class
/// object. Aliases are excluded up front: older CPython calls `list[int]` a type.
fn annotation_class<'py>(annotation: &Bound<'py, PyAny>) -> Option<Bound<'py, PyType>> {
    if annotation.hasattr("__origin__").unwrap_or(false) {
        return None;
    }
    annotation.cast::<PyType>().ok().cloned()
}

fn unwrap_optional_param<'py>(
    annotation: &Bound<'py, PyAny>,
) -> PyResult<(bool, Bound<'py, PyAny>)> {
    let py = annotation.py();
    let union_type = py.import("types")?.getattr("UnionType")?;
    let typing_union = py.import("typing")?.getattr("Union")?;
    let is_union = annotation.get_type().is(&union_type)
        || annotation
            .getattr("__origin__")
            .is_ok_and(|origin| origin.is(&typing_union));
    if !is_union {
        return Ok((false, annotation.clone()));
    }
    let Ok(args) = annotation.getattr("__args__") else {
        return Ok((false, annotation.clone()));
    };
    let args = args.cast::<PyTuple>()?;
    if args.len() != 2 {
        return Ok((false, annotation.clone()));
    }
    let none_type = py.None().bind(py).get_type();
    let first = args.get_item(0)?;
    let second = args.get_item(1)?;
    let inner = if first.is(&none_type) {
        second
    } else if second.is(&none_type) {
        first
    } else {
        return Ok((false, annotation.clone()));
    };
    if let Ok(param) = inner.extract::<PyRef<'_, PyQueryParam>>() {
        if param.single_entity_enforced {
            return Ok((true, inner.clone()));
        }
    }
    let Ok(origin) = inner.getattr("__origin__") else {
        return Ok((false, annotation.clone()));
    };
    if origin.is(PySingle::type_object(py)) {
        let param = unwrap_type_argument(&inner)?;
        if param
            .extract::<PyRef<'_, PyQueryParam>>()?
            .single_entity_enforced
        {
            return Ok((true, param));
        }
    }
    if origin.is(PyRes::type_object(py)) || origin.is(PyResMut::type_object(py)) {
        Ok((true, inner))
    } else {
        Ok((false, annotation.clone()))
    }
}

/// The public annotation spelling, otherwise the annotation's repr.
fn annotation_type_name(annotation: &Bound<'_, PyAny>) -> String {
    if let Ok(asset) = annotation.extract::<PyRef<'_, PyAssetTypeParam>>() {
        let py = annotation.py();
        let class = asset
            .wrapper_class(py)
            .or_else(|| asset.asset_type_class().ok().map(Py::into_any));
        if let Some(class) = class {
            return format!("Assets[{}]", annotation_type_name(class.bind(py)));
        }
    }
    annotation_class(annotation)
        .and_then(|class| class.getattr("__name__").ok())
        .and_then(|name| name.extract::<String>().ok())
        .unwrap_or_else(|| format!("{annotation:?}"))
}

/// Why an annotation that matched no parameter kind cannot be lowered. The
/// base-class tests also catch user `@component` and `@message` classes.
fn classify_system_param(
    wrapper: SystemParamWrapper,
    annotation: &Bound<'_, PyAny>,
) -> PyResult<SystemParamRejection> {
    if wrapper == SystemParamWrapper::Mut {
        if annotation.is_instance_of::<PyAssetTypeParam>() {
            return Ok(SystemParamRejection::NakedMutAssets);
        }
        return Ok(SystemParamRejection::NakedMut);
    }
    let Some(class) = annotation_class(annotation) else {
        return Ok(SystemParamRejection::Unsupported);
    };
    if class.is_subclass_of::<PyMessage>()? {
        if matches!(
            PyMessageType::from_message_type(&class),
            Ok(PyMessageType(MessageType::Custom(_)))
        ) {
            Ok(SystemParamRejection::MessageType)
        } else {
            Ok(SystemParamRejection::NativeMessageType)
        }
    } else if class.is_subclass_of::<PyComponent>()? {
        Ok(SystemParamRejection::ComponentType)
    } else if class.is_subclass_of::<PyAsset>()? {
        Ok(SystemParamRejection::AssetType)
    } else {
        Ok(SystemParamRejection::Unsupported)
    }
}

fn system_param_error(
    system: &str,
    field_name: &str,
    wrapper: SystemParamWrapper,
    annotation: &Bound<'_, PyAny>,
    kind: SystemParamRejection,
) -> PyErr {
    PyTypeError::new_err(system_param_rejection_message(
        system,
        field_name,
        wrapper,
        &annotation_type_name(annotation),
        kind,
    ))
}

/// `Mut[T]`, `Res[T]` and `ResMut[T]` nest their argument as `((T,),)` once
/// `typing.get_type_hints` has re-evaluated the alias.
fn unwrap_type_argument<'py>(alias: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let args = alias.getattr("__args__")?;
    let first = args.cast::<PyTuple>()?.get_item(0)?;
    if first.is_instance_of::<PyTuple>() {
        first.cast::<PyTuple>()?.get_item(0)
    } else {
        Ok(first)
    }
}

impl SystemFunction {
    /// Clear the global system parameter cache.
    /// This should be called when tearing down an app to prevent stale cache entries.
    pub fn clear_cache() {
        // Cached params can own Python objects whose finalizers re-enter system
        // registration. Move the cache out before dropping any entries so Python
        // code never runs while SYSTEM_PARAM_CACHE is locked.
        let old_cache = SYSTEM_PARAM_CACHE
            .lock()
            .ok()
            .and_then(|mut cache_guard| cache_guard.take());
        drop(old_cache);
    }

    pub fn new(py: Python, func: Bound<'_, PyAny>) -> PyResult<Self> {
        // Check if function is async (coroutine) - not supported
        let inspect = py.import("inspect")?;
        let is_coroutine_fn = inspect.getattr("iscoroutinefunction")?;
        let is_async = is_coroutine_fn.call1((&func,))?.extract::<bool>()?;

        if is_async {
            return Err(PyRuntimeError::new_err(
                "Async systems (async def) are not supported. \
                Use synchronous 'def' instead of 'async def'. \
                Async functions would break PyBevy's safety guarantees (ValidityFlag becomes invalid when function suspends).",
            ));
        }

        let func_addr = func.as_ptr() as usize;
        // Use __code__ object pointer as a fingerprint so we detect CPython address
        // reuse: if a new closure is allocated at the same address as a GC'd one,
        // its __code__ will differ and the stale cache entry is discarded.
        let code_ptr = func
            .getattr("__code__")
            .map(|c| c.as_ptr() as usize)
            .unwrap_or(0);

        // Only clone the Arc while holding the cache lock. Cloning the actual
        // parameters can incref Python objects and must happen after unlocking.
        let (cached_params, stale_entry) = {
            let mut cache_guard = SYSTEM_PARAM_CACHE.lock().unwrap();
            let cache = cache_guard.get_or_insert_with(HashMap::new);

            if let Some((cached_code, cached_params)) = cache.get(&func_addr) {
                if *cached_code == code_ptr {
                    (Some(Arc::clone(cached_params)), None)
                } else {
                    // Move the stale entry out so it is dropped after unlocking.
                    (None, cache.remove(&func_addr))
                }
            } else {
                (None, None)
            }
        };
        drop(stale_entry);

        let params = if let Some(cached_params) = cached_params {
            cached_params.as_ref().clone()
        } else {
            let parsed_params = Self::parse_system_parameters(&func, py)?;
            let params_for_cache = Arc::new(parsed_params.clone());

            // Move any concurrently replaced entry out and drop it after unlocking.
            let replaced_entry = {
                let mut cache_guard = SYSTEM_PARAM_CACHE.lock().unwrap();
                let cache = cache_guard.get_or_insert_with(HashMap::new);
                cache.insert(func_addr, (code_ptr, params_for_cache))
            };
            drop(replaced_entry);

            parsed_params
        };

        // Every registration needs its own Local. `Local[T]` in an annotation is
        // one object built at module definition, and the parameter cache hands
        // the same one back for a repeated callable, so rebuild here: this is
        // the single point both the parsed and the cached path reach.
        let mut params = params;
        for param in &mut params {
            if let SystemParamType::Local(local) = &param.ty {
                let fresh = local
                    .bind(py)
                    .cast::<PyLocal>()?
                    .borrow()
                    .fresh_for_registration(py)?;
                param.ty = SystemParamType::Local(fresh);
            }
        }

        Ok(Self {
            func: func.unbind(),
            params,
        })
    }

    /// Check the marker-only part of a downstream pipe signature before the
    /// ordinary system-parameter parser rejects an arbitrary first annotation.
    pub(crate) fn validate_pipe_target_signature(
        func: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let name = func.getattr("__name__")?.extract::<String>()?;
        let typing = py.import("typing")?;
        let type_hints = typing
            .call_method1("get_type_hints", (func,))
            .map_err(|error| PyTypeError::new_err(system_annotations_unresolved(&name, error)))?;
        let inspect = py.import("inspect")?;
        let parameters = inspect
            .call_method1("signature", (func,))?
            .getattr("parameters")?
            .getattr("values")?
            .call0()?;
        let mut input_indices = Vec::new();
        for (index, parameter) in parameters.try_iter()?.enumerate() {
            let parameter = parameter?;
            let field_name = parameter.getattr("name")?.extract::<String>()?;
            let annotation = type_hints
                .get_item(&field_name)
                .or_else(|_| parameter.getattr("annotation"))?;
            if annotation.get_type().is(PyInParam::type_object(py)) {
                input_indices.push(index);
            }
        }
        match input_indices.as_slice() {
            [0] => Ok(()),
            [] => Err(PyTypeError::new_err(pipe_target_requires_input(name))),
            _ => Err(PyTypeError::new_err(pipe_input_must_be_first(name))),
        }
    }

    /// Analyzes the function signature and parses the system parameters
    fn parse_system_parameters(
        func: &Bound<'_, PyAny>,
        py: Python,
    ) -> Result<SmallVec<[SystemParam; STACK_PARAMS]>, PyErr> {
        let name = func.getattr("__name__")?.to_string();

        // Use typing.get_type_hints() to resolve string annotations from __future__ imports
        let typing_module = py.import("typing")?;
        let type_hints = typing_module
            .call_method1("get_type_hints", (func,))
            .map_err(|error| PyTypeError::new_err(system_annotations_unresolved(&name, error)))?;

        let sig_module = py.import("inspect")?;

        let sig = sig_module.call_method1("signature", (func,))?;
        let params = sig.getattr("parameters")?;
        // cal values to get the actual values
        let values = params.getattr("values")?;
        let params = values.call0()?;

        // No annotation falls back to `inspect.Parameter.empty`; `x: None` resolves to `NoneType`.
        let empty_annotation = sig_module.getattr("Parameter")?.getattr("empty")?;
        let none_type = py.None().bind(py).get_type();

        let mut result = SmallVec::new();

        for param in params.try_iter()? {
            let param = param?;
            let field_name = param.getattr("name")?.to_string();

            // Try to get the resolved type hint first, fall back to raw annotation
            let raw_annotation = type_hints
                .get_item(&field_name)
                .or_else(|_| param.getattr("annotation"))?;

            if raw_annotation.is(&empty_annotation)
                || raw_annotation.is(&none_type)
                || raw_annotation.is_none()
            {
                return Err(system_param_error(
                    &name,
                    &field_name,
                    SystemParamWrapper::Bare,
                    &raw_annotation,
                    SystemParamRejection::MissingAnnotation,
                ));
            }

            let (optional_resource, raw_annotation) = unwrap_optional_param(&raw_annotation)?;

            // Check if the raw annotation is a generic alias (e.g., Mut[Time], Res[Time], ResMut[Time])
            // by checking if it has __origin__ attribute
            let (wrapper, annotation) = if let Ok(origin) = raw_annotation.getattr("__origin__") {
                // This is a generic type like Mut[Time], Res[Time], ResMut[Time], Assets[Mesh], etc.
                if origin.is(PyMut::type_object(py)) {
                    (
                        SystemParamWrapper::Mut,
                        unwrap_type_argument(&raw_annotation)?,
                    )
                } else if origin.is(PyRes::type_object(py)) {
                    (
                        SystemParamWrapper::Res,
                        unwrap_type_argument(&raw_annotation)?,
                    )
                } else if origin.is(PyResMut::type_object(py)) {
                    (
                        SystemParamWrapper::ResMut,
                        unwrap_type_argument(&raw_annotation)?,
                    )
                } else {
                    // Other generic type (not Mut/Res/ResMut) - use as-is
                    (SystemParamWrapper::Bare, raw_annotation.clone())
                }
            } else {
                // Direct type annotation - immutable
                (SystemParamWrapper::Bare, raw_annotation.clone())
            };
            if wrapper == SystemParamWrapper::Mut {
                return Err(system_param_error(
                    &name,
                    &field_name,
                    wrapper,
                    &annotation,
                    classify_system_param(wrapper, &annotation)?,
                ));
            }
            let is_mutable = matches!(
                wrapper,
                SystemParamWrapper::Mut | SystemParamWrapper::ResMut
            );
            let is_wrapped_resource = matches!(
                wrapper,
                SystemParamWrapper::Res | SystemParamWrapper::ResMut
            );

            if is_wrapped_resource && let Ok(type_obj) = annotation.cast::<PyType>() {
                reject_state_type_as_resource(type_obj)?;
            }

            let param_name = if annotation.is_instance_of::<PyType>() {
                annotation.getattr("__name__")?.extract::<String>()?
            } else {
                annotation
                    .get_type()
                    .getattr("__name__")?
                    .extract::<String>()?
            };

            let ty = if annotation.get_type().is(PyQueryParam::type_object(py)) {
                let mut obj = annotation.extract::<PyQueryParam>()?;
                obj.optional_single = optional_resource;
                // For Query, mutability is determined by the component parameters (Mut[Component]),
                // not by wrapping the entire Query parameter
                SystemParamType::Query {
                    param: Arc::new(obj),
                }
            } else if annotation.get_type().is(PyViewParam::type_object(py)) {
                let obj = annotation.extract::<PyViewParam>()?;
                // For View, mutability is determined by the component parameters (Mut[Component])
                SystemParamType::View {
                    param: Arc::new(obj),
                }
            } else if param_name == "View" || annotation.get_type().is(PyView::type_object(py)) {
                // Plain View without type parameters - not currently supported
                return Err(PyTypeError::new_err(format!(
                    "System function `{}` uses View without type parameters. Use View[Mut[Component]] or View[Component]",
                    name,
                )));
            } else if annotation.get_type().is(PyInParam::type_object(py)) {
                let input = annotation.extract::<PyRef<PyInParam>>()?;
                SystemParamType::PipeInput {
                    value_type: input.value_type(py),
                }
            } else if annotation.hasattr("__origin__")? {
                // Check if it's a View generic alias by checking __origin__.__name__
                let origin_name = annotation
                    .getattr("__origin__")
                    .and_then(|origin| origin.getattr("__name__"))
                    .and_then(|origin_name| origin_name.extract::<String>())
                    .unwrap_or_default();
                if origin_name == "View" {
                    // This shouldn't happen anymore since __class_getitem__ returns PyViewParam
                    return Err(PyTypeError::new_err(format!(
                        "System function `{}` has unexpected View format. Use View[Mut[Component]] or View[Component]",
                        name,
                    )));
                } else {
                    return Err(system_param_error(
                        &name,
                        &field_name,
                        wrapper,
                        &annotation,
                        classify_system_param(wrapper, &annotation)?,
                    ));
                }
            } else if annotation.is_instance_of::<PyLocal>() {
                SystemParamType::Local(annotation.clone().unbind())
            } else if annotation.get_type().is(PyAssetTypeParam::type_object(py)) {
                // Assets[T] - extract the asset type
                let asset_param = annotation.extract::<PyAssetTypeParam>()?;
                SystemParamType::Assets {
                    type_ptr: AssetTypePtr(asset_param.type_ptr()),
                    wrapper_class: asset_param.wrapper_class(py),
                    logical_type_id: asset_param.logical_type_id(),
                    logical_type_name: asset_param.logical_type_name().map(str::to_owned),
                    mutable: is_mutable,
                    optional: optional_resource,
                }
            } else if annotation.is(PyWorld::type_object(py)) {
                SystemParamType::World
            } else if annotation.is(PyCommands::type_object(py)) {
                SystemParamType::Commands
            } else if annotation.is(PyGizmos::type_object(py)) {
                SystemParamType::Gizmos
            } else if annotation.get_type().is(MessageTypeParam::type_object(py)) {
                // MessageWriter[T], MessageReader[T], or MessageMutator[T]
                let message_param = annotation.extract::<MessageTypeParam>()?;
                match message_param.ty {
                    super::message::MessageClass::Writer => SystemParamType::MessageWriter {
                        message_type: message_param.message_type,
                    },
                    super::message::MessageClass::Reader => SystemParamType::MessageReader {
                        message_type: message_param.message_type,
                    },
                    super::message::MessageClass::Mutator => {
                        if !matches!(message_param.message_type, MessageType::Custom(_)) {
                            return Err(PyTypeError::new_err(
                                "MessageMutator currently supports custom Python messages only; native message wrappers are snapshots and cannot be mutated in place",
                            ));
                        }
                        SystemParamType::MessageMutator {
                            message_type: message_param.message_type,
                        }
                    }
                }
            } else if annotation
                .get_type()
                .is(super::observer::PyOnTypeParam::type_object(py))
            {
                // On[E] or On[E, B] - extract event type and optional bundle filter
                let on_param = annotation.extract::<super::observer::PyOnTypeParam>()?;
                SystemParamType::On {
                    event_type: on_param.event_type,
                    bundle_filter: on_param.bundle_filter,
                }
            } else if annotation.is(PyResource::type_object(py)) {
                return Err(PyTypeError::new_err(
                    "Resource must be subclass of `Resource`".to_string(),
                ));
            } else if annotation.is_instance_of::<PyType>()
                && annotation
                    .cast::<PyType>()?
                    .is_subclass_of::<PyResource>()?
            {
                // Validate the specialization before generating a resource remedy.
                reject_unparameterized_button_input(annotation.cast::<PyType>()?)?;
                if is_wrapped_resource {
                    // Resource wrapped in Res[T] or ResMut[T] - allowed
                    let type_obj = annotation.cast::<PyType>()?;
                    SystemParamType::Resource {
                        type_obj: type_obj.clone().unbind(),
                        mutable: is_mutable,
                        optional: optional_resource,
                    }
                } else {
                    // Bare resources are not allowed - must use Res[T] or ResMut[T]
                    return Err(system_param_error(
                        &name,
                        &field_name,
                        wrapper,
                        &annotation,
                        SystemParamRejection::BareResource,
                    ));
                }
            } else {
                return Err(system_param_error(
                    &name,
                    &field_name,
                    wrapper,
                    &annotation,
                    classify_system_param(wrapper, &annotation)?,
                ));
            };

            result.push(SystemParam {
                name: param_name,
                ty,
            });
        }

        Ok(result)
    }
}

/// Represents a single system parameter with its type information
#[derive(Debug, Clone)]
pub struct SystemParam {
    /// The name of the system parameter
    pub name: String,
    /// The type of the system parameter
    pub ty: SystemParamType,
}

/// Wrapper for `*const PyTypeObject` that is Send + Sync.
///
/// SAFETY: PyTypeObject pointers are stable for the lifetime of the Python interpreter.
/// Python type objects are immutable after creation and safe to share across threads.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AssetTypePtr(pub(crate) *const pyo3::ffi::PyTypeObject);

unsafe impl Send for AssetTypePtr {}
unsafe impl Sync for AssetTypePtr {}

#[derive(Debug)]
pub enum SystemParamType {
    PipeInput {
        value_type: Py<PyAny>,
    },
    Query {
        param: Arc<PyQueryParam>,
    },
    View {
        param: Arc<PyViewParam>,
    },
    Local(Py<PyAny>),
    Resource {
        type_obj: Py<PyType>,
        mutable: bool,
        optional: bool,
    },
    Assets {
        type_ptr: AssetTypePtr,
        /// Optional wrapper class for `@material` redirects (e.g. HologramMaterial).
        wrapper_class: Option<Py<PyAny>>,
        logical_type_id: Option<LogicalTypeId>,
        logical_type_name: Option<String>,
        mutable: bool,
        optional: bool,
    },
    World,
    Commands,
    Gizmos,
    MessageWriter {
        message_type: MessageType,
    },
    MessageReader {
        message_type: MessageType,
    },
    MessageMutator {
        message_type: MessageType,
    },
    On {
        event_type: EventType,
        bundle_filter: Option<Vec<PyComponentType>>,
    },
}

impl Clone for SystemParamType {
    fn clone(&self) -> Self {
        Python::attach(|py| match self {
            SystemParamType::PipeInput { value_type } => SystemParamType::PipeInput {
                value_type: value_type.clone_ref(py),
            },
            SystemParamType::Query { param } => SystemParamType::Query {
                param: Arc::clone(param),
            },
            SystemParamType::View { param } => SystemParamType::View {
                param: Arc::clone(param),
            },
            SystemParamType::Local(l) => SystemParamType::Local(l.clone_ref(py)),
            SystemParamType::Resource {
                type_obj,
                mutable,
                optional,
            } => SystemParamType::Resource {
                type_obj: type_obj.clone_ref(py),
                mutable: *mutable,
                optional: *optional,
            },
            SystemParamType::Assets {
                type_ptr: ptr,
                wrapper_class,
                logical_type_id,
                logical_type_name,
                mutable,
                optional,
            } => SystemParamType::Assets {
                type_ptr: *ptr,
                wrapper_class: wrapper_class.as_ref().map(|class| class.clone_ref(py)),
                logical_type_id: *logical_type_id,
                logical_type_name: logical_type_name.clone(),
                mutable: *mutable,
                optional: *optional,
            },
            SystemParamType::World => SystemParamType::World,
            SystemParamType::Commands => SystemParamType::Commands,
            SystemParamType::Gizmos => SystemParamType::Gizmos,
            SystemParamType::MessageWriter { message_type } => SystemParamType::MessageWriter {
                message_type: message_type.clone(),
            },
            SystemParamType::MessageReader { message_type } => SystemParamType::MessageReader {
                message_type: message_type.clone(),
            },
            SystemParamType::MessageMutator { message_type } => SystemParamType::MessageMutator {
                message_type: message_type.clone(),
            },
            SystemParamType::On {
                event_type,
                bundle_filter,
            } => SystemParamType::On {
                event_type: event_type.clone(),
                bundle_filter: bundle_filter.clone(),
            },
        })
    }
}
