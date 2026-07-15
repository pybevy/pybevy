use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use pyo3::{
    PyTypeInfo,
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyAny, PyDict, PyTuple, PyType},
};
use smallvec::SmallVec;

use super::{
    commands::PyCommands,
    local::PyLocal,
    message::MessageTypeParam,
    mutable::PyMut,
    resource::{PyRes, PyResMut, PyResource},
    world::PyWorld,
};
use crate::{
    assets::asset_type::PyAssetTypeParam,
    ecs::{
        component_type::PyComponentType,
        messages::MessageType,
        observer::EventType,
        query::query_param::PyQueryParam,
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
static SYSTEM_PARAM_CACHE: Mutex<
    Option<HashMap<usize, (usize, SmallVec<[SystemParam; STACK_PARAMS]>)>>,
> = Mutex::new(None);

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

impl SystemFunction {
    /// Clear the global system parameter cache.
    /// This should be called when tearing down an app to prevent stale cache entries.
    pub fn clear_cache() {
        if let Ok(mut cache_guard) = SYSTEM_PARAM_CACHE.lock()
            && let Some(cache) = cache_guard.as_mut()
        {
            cache.clear();
        }
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

        // Try to get from cache first
        let params = {
            let mut cache_guard = SYSTEM_PARAM_CACHE.lock().unwrap();
            let cache = cache_guard.get_or_insert_with(HashMap::new);

            if let Some((cached_code, cached_params)) = cache.get(&func_addr) {
                if *cached_code == code_ptr {
                    // Same function - return cached params
                    cached_params.clone()
                } else {
                    // Address was reused for a different function - discard stale entry
                    cache.remove(&func_addr);
                    drop(cache_guard);
                    let parsed_params = Self::parse_system_parameters(&func, py)?;
                    let mut cache_guard = SYSTEM_PARAM_CACHE.lock().unwrap();
                    let cache = cache_guard.get_or_insert_with(HashMap::new);
                    cache.insert(func_addr, (code_ptr, parsed_params.clone()));
                    parsed_params
                }
            } else {
                // Not in cache - parse and insert
                drop(cache_guard); // Release lock before parsing
                let parsed_params = Self::parse_system_parameters(&func, py)?;

                // Insert into cache
                let mut cache_guard = SYSTEM_PARAM_CACHE.lock().unwrap();
                let cache = cache_guard.get_or_insert_with(HashMap::new);
                cache.insert(func_addr, (code_ptr, parsed_params.clone()));

                parsed_params
            }
        };

        Ok(Self {
            func: func.unbind(),
            params,
        })
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
            .unwrap_or_else(|_| {
                // If get_type_hints fails, fall back to empty dict
                PyDict::new(py).into_any()
            });

        let sig_module = py.import("inspect")?;

        let sig = sig_module.call_method1("signature", (func,))?;
        let params = sig.getattr("parameters")?;
        // cal values to get the actual values
        let values = params.getattr("values")?;
        let params = values.call0()?;

        let mut result = SmallVec::new();

        for param in params.try_iter()? {
            let param = param?;
            let field_name = param.getattr("name")?.to_string();

            // Try to get the resolved type hint first, fall back to raw annotation
            let raw_annotation = type_hints
                .get_item(&field_name)
                .or_else(|_| param.getattr("annotation"))?;

            // Check if the raw annotation is a generic alias (e.g., Mut[Time], Res[Time], ResMut[Time])
            // by checking if it has __origin__ attribute
            let (is_mutable, annotation, is_wrapped_resource) =
                if let Ok(origin) = raw_annotation.getattr("__origin__") {
                    // This is a generic type like Mut[Time], Res[Time], ResMut[Time], Assets[Mesh], etc.
                    if origin.is(PyMut::type_object(py)) {
                        // Mut[T] - extract the inner type from __args__
                        let args = raw_annotation.getattr("__args__")?;
                        let inner = args.get_item(0)?;
                        (true, inner, false)
                    } else if origin.is(PyRes::type_object(py)) {
                        // Res[T] - extract the inner type, mark as read-only
                        // Note: typing.get_type_hints() creates nested tuples: ((<class T>,),)
                        // So we need to extract twice: first gets (<class T>,), second gets <class T>
                        let args = raw_annotation.getattr("__args__")?;
                        let args_tuple = args.cast::<PyTuple>()?;
                        let first_elem = args_tuple.get_item(0)?;
                        // Check if it's a nested tuple (from typing.get_type_hints)
                        let inner = if first_elem.is_instance_of::<PyTuple>() {
                            first_elem.cast::<PyTuple>()?.get_item(0)?
                        } else {
                            first_elem
                        };
                        (false, inner.clone(), true)
                    } else if origin.is(PyResMut::type_object(py)) {
                        // ResMut[T] - extract the inner type, mark as mutable
                        // Note: typing.get_type_hints() creates nested tuples: ((<class T>,),)
                        let args = raw_annotation.getattr("__args__")?;
                        let args_tuple = args.cast::<PyTuple>()?;
                        let first_elem = args_tuple.get_item(0)?;
                        // Check if it's a nested tuple (from typing.get_type_hints)
                        let inner = if first_elem.is_instance_of::<PyTuple>() {
                            first_elem.cast::<PyTuple>()?.get_item(0)?
                        } else {
                            first_elem
                        };
                        (true, inner.clone(), true)
                    } else {
                        // Other generic type (not Mut/Res/ResMut) - use as-is
                        (false, raw_annotation.clone(), false)
                    }
                } else {
                    // Direct type annotation - immutable
                    (false, raw_annotation.clone(), false)
                };

            // Now check if we need to resolve this further with get_type_hints
            // for things like forward references
            let annotation = if annotation.is_none() {
                return Err(PyTypeError::new_err(format!(
                    "System function `{}` parameter `{}` has no type annotation",
                    name, field_name
                )));
            } else {
                annotation
            };

            let param_name = if annotation.is_instance_of::<PyType>() {
                annotation.getattr("__name__")?.extract::<String>()?
            } else {
                annotation
                    .get_type()
                    .getattr("__name__")?
                    .extract::<String>()?
            };

            let ty = if annotation.get_type().is(PyQueryParam::type_object(py)) {
                let obj = annotation.extract::<PyQueryParam>()?;
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
            } else if annotation.hasattr("__origin__")? {
                // Check if it's a View generic alias by checking __origin__.__name__
                let origin = annotation.getattr("__origin__")?;
                let origin_name = origin.getattr("__name__")?.extract::<String>()?;
                if origin_name == "View" {
                    // This shouldn't happen anymore since __class_getitem__ returns PyViewParam
                    return Err(PyTypeError::new_err(format!(
                        "System function `{}` has unexpected View format. Use View[Mut[Component]] or View[Component]",
                        name,
                    )));
                } else {
                    return Err(PyTypeError::new_err(format!(
                        "System function `{}` has an unsupported system parameter `{:?}`",
                        name, annotation,
                    )));
                }
            } else if annotation.is_instance_of::<PyLocal>() {
                SystemParamType::Local(annotation.unbind().clone_ref(py))
            } else if annotation.get_type().is(PyAssetTypeParam::type_object(py)) {
                // Assets[T] - extract the asset type
                let asset_param = annotation.extract::<PyAssetTypeParam>()?;
                SystemParamType::Assets {
                    type_ptr: AssetTypePtr(asset_param.type_ptr()),
                    wrapper_class: asset_param.wrapper_class().map(AssetTypePtr),
                    mutable: is_mutable,
                }
            } else if annotation.is(PyWorld::type_object(py)) {
                SystemParamType::World
            } else if annotation.is(PyCommands::type_object(py)) {
                SystemParamType::Commands
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
                if is_wrapped_resource {
                    // Resource wrapped in Res[T] or ResMut[T] - allowed
                    let type_obj = annotation.cast::<PyType>()?;
                    SystemParamType::Resource {
                        type_obj: type_obj.clone().unbind(),
                        mutable: is_mutable,
                    }
                } else {
                    // Bare resources are not allowed - must use Res[T] or ResMut[T]
                    let type_name = annotation
                        .cast::<PyType>()?
                        .getattr("__name__")?
                        .extract::<String>()?;
                    return Err(PyTypeError::new_err(format!(
                        "System function `{}` parameter `{}` must use Res[{}] for read-only or ResMut[{}] for mutable access",
                        name, field_name, type_name, type_name
                    )));
                }
            } else {
                return Err(PyTypeError::new_err(format!(
                    "System function `{}` has an unsupported system parameter `{:?}`",
                    name, annotation,
                )));
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
    },
    Assets {
        type_ptr: AssetTypePtr,
        /// Optional wrapper class for `@material` redirects (e.g. HologramMaterial).
        wrapper_class: Option<AssetTypePtr>,
        mutable: bool,
    },
    World,
    Commands,
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
            SystemParamType::Query { param } => SystemParamType::Query {
                param: Arc::clone(param),
            },
            SystemParamType::View { param } => SystemParamType::View {
                param: Arc::clone(param),
            },
            SystemParamType::Local(l) => SystemParamType::Local(l.clone_ref(py)),
            SystemParamType::Resource { type_obj, mutable } => SystemParamType::Resource {
                type_obj: type_obj.clone_ref(py),
                mutable: *mutable,
            },
            SystemParamType::Assets {
                type_ptr: ptr,
                wrapper_class,
                mutable,
            } => SystemParamType::Assets {
                type_ptr: *ptr,
                wrapper_class: *wrapper_class,
                mutable: *mutable,
            },
            SystemParamType::World => SystemParamType::World,
            SystemParamType::Commands => SystemParamType::Commands,
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
