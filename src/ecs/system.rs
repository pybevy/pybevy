use std::{
    collections::HashSet,
    sync::{Arc, Mutex, Weak},
};

use bevy::ecs::resource::Resource;
use pybevy_core::{
    LogicalTypeId, PyAsset, PyComponent, PyMessage,
    public_error::{
        SYSTEM_CALLABLE_ROUTINE_REQUIRED, SystemParamRejection, SystemParamWrapper,
        param_set_member_unsupported, pipe_input_must_be_first, pipe_target_requires_input,
        system_annotations_unresolved, system_param_rejection_message,
    },
};
use pybevy_gizmos::gizmos::PyGizmos;
use pyo3::{
    PyTraverseError, PyTypeInfo, PyVisit,
    exceptions::{PyRuntimeError, PyTypeError},
    ffi::PyTypeObject,
    prelude::*,
    types::{PyAny, PyTuple, PyType},
};
use smallvec::SmallVec;

use super::{
    commands::PyCommands,
    local::PyLocal,
    message::{MessageClass, MessageTypeParam},
    mutable::PyMut,
    resource::{PyRes, PyResMut, PyResource},
    system_input::PyInParam,
    world::PyWorld,
};
use crate::{
    assets::asset_type::PyAssetTypeParam,
    ecs::{
        component_type::PyComponentType,
        dynamic_system::{DynamicSystemHandle, DynamicSystemInner},
        messages::{MessageType, PyMessageType},
        non_send_marker::PyNonSendMarker,
        observer::{EventType, PyOnTypeParam},
        param_set::PyParamSetParam,
        query::{query_param::PyQueryParam, single::PySingle},
        removed_components::PyRemovedComponentsParam,
        resource_type::{reject_state_type_as_resource, reject_unparameterized_button_input},
        view::{view::PyView, view_param::PyViewParam},
    },
};

const STACK_PARAMS: usize = 8;

/// Represents a pythonic system function with its parameters cached for efficient calls
#[derive(Debug)]
pub struct SystemFunction {
    /// The python function to call
    pub func: Py<PyAny>,

    /// The parameters of the system function
    pub params: Arc<SmallVec<[SystemParam; STACK_PARAMS]>>,
}

impl Clone for SystemFunction {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            func: self.func.clone_ref(py),
            params: Arc::new(self.params.as_ref().clone()),
        })
    }
}

pub(crate) fn require_routine_callable(func: &Bound<'_, PyAny>) -> PyResult<()> {
    if !func.is_callable() {
        return Err(PyTypeError::new_err("system or observer must be callable"));
    }
    let inspect = func.py().import("inspect")?;
    for predicate in ["isfunction", "ismethod", "isbuiltin"] {
        if inspect
            .getattr(predicate)?
            .call1((func,))?
            .extract::<bool>()?
        {
            return Ok(());
        }
    }
    Err(PyTypeError::new_err(SYSTEM_CALLABLE_ROUTINE_REQUIRED))
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
    if let Ok(param) = inner.extract::<PyRef<'_, PyQueryParam>>()
        && param.single_entity_enforced
    {
        return Ok((true, inner.clone()));
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

#[derive(Resource, Default)]
pub(crate) struct SystemGcRegistry {
    handles: Vec<Weak<Mutex<DynamicSystemInner>>>,
    queries: Vec<Weak<PyQueryParam>>,
}

impl SystemGcRegistry {
    pub(crate) fn register_handles(&mut self, handles: &[DynamicSystemHandle]) {
        self.handles.retain(|handle| handle.strong_count() > 0);
        self.handles.extend(handles.iter().map(Arc::downgrade));
    }

    pub(crate) fn register_queries<'a>(
        &mut self,
        queries: impl Iterator<Item = &'a Arc<PyQueryParam>>,
    ) {
        self.queries.retain(|param| param.strong_count() > 0);
        self.queries.extend(queries.map(Arc::downgrade));
    }

    pub(crate) fn traverse(
        &self,
        owners: &mut SystemGcOwners,
        visit: &PyVisit<'_>,
    ) -> Result<(), PyTraverseError> {
        for handle in &self.handles {
            if let Some(handle) = handle.upgrade() {
                owners.traverse_handle(&handle, visit)?;
            }
        }
        for param in &self.queries {
            if let Some(param) = param.upgrade() {
                owners.traverse_query(&param, visit)?;
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct SystemGcOwners {
    handles: HashSet<usize>,
    signatures: HashSet<usize>,
    descriptors: HashSet<usize>,
}

impl SystemGcOwners {
    pub(crate) fn traverse_handle(
        &mut self,
        handle: &DynamicSystemHandle,
        visit: &PyVisit<'_>,
    ) -> Result<(), PyTraverseError> {
        if !self.handles.insert(Arc::as_ptr(handle) as usize) {
            return Ok(());
        }
        let Ok(inner) = handle.try_lock() else {
            return Ok(());
        };
        if let Some(func) = &inner.cached_func {
            visit.call(func)?;
        }
        if let Some(func) = &inner.system_func {
            visit.call(&func.func)?;
            if self.signatures.insert(Arc::as_ptr(&func.params) as usize) {
                for param in func.params.iter() {
                    self.traverse_param(&param.ty, visit)?;
                }
            }
        }
        Ok(())
    }

    fn traverse_query(
        &mut self,
        param: &Arc<PyQueryParam>,
        visit: &PyVisit<'_>,
    ) -> Result<(), PyTraverseError> {
        if self.descriptors.insert(Arc::as_ptr(param) as usize) {
            for class in &param.retained_types {
                visit.call(class.as_ref())?;
            }
            for value in &param.retained_asset_params {
                visit.call(value)?;
            }
        }
        Ok(())
    }

    fn traverse_param(
        &mut self,
        param: &SystemParamType,
        visit: &PyVisit<'_>,
    ) -> Result<(), PyTraverseError> {
        match param {
            SystemParamType::PipeInput { value_type } => visit.call(value_type)?,
            SystemParamType::Local(value) => visit.call(value)?,
            SystemParamType::Resource { type_obj, .. } => visit.call(type_obj)?,
            SystemParamType::Assets {
                wrapper_class: Some(class),
                ..
            } => visit.call(class)?,
            SystemParamType::Query { param } => self.traverse_query(param, visit)?,
            SystemParamType::View { param } => {
                if self.descriptors.insert(Arc::as_ptr(param) as usize) {
                    for class in &param.retained_types {
                        visit.call(class.as_ref())?;
                    }
                }
            }
            SystemParamType::RemovedComponents { param } => {
                if self.descriptors.insert(Arc::as_ptr(param) as usize) {
                    for class in &param.retained_types {
                        visit.call(class.as_ref())?;
                    }
                }
            }
            SystemParamType::MessageWriter { message_type }
            | SystemParamType::MessageReader { message_type }
            | SystemParamType::MessageMutator { message_type } => message_type.traverse(visit)?,
            SystemParamType::On { event_type, .. } => event_type.traverse(visit)?,
            SystemParamType::ParamSet { members }
                if self.descriptors.insert(Arc::as_ptr(members) as usize) =>
            {
                for member in members.iter() {
                    self.traverse_param(&member.ty, visit)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl SystemFunction {
    pub fn new(py: Python, func: Bound<'_, PyAny>) -> PyResult<Self> {
        require_routine_callable(&func)?;
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

        for (predicate, kind) in [
            ("isgeneratorfunction", "generator"),
            ("isasyncgenfunction", "async generator"),
        ] {
            if inspect
                .getattr(predicate)?
                .call1((&func,))?
                .extract::<bool>()?
            {
                return Err(PyTypeError::new_err(format!(
                    "{kind} functions cannot be systems or observers; use a synchronous function without yield"
                )));
            }
        }

        let mut params = Self::parse_system_parameters(&func, py)?;
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

        let params = Arc::new(params);
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
        let keyword_only = sig_module.getattr("Parameter")?.getattr("KEYWORD_ONLY")?;
        let none_type = py.None().bind(py).get_type();

        let mut result = SmallVec::new();

        for param in params.try_iter()? {
            let param = param?;
            let field_name = param.getattr("name")?.to_string();

            if param.getattr("kind")?.is(&keyword_only) {
                return Err(PyTypeError::new_err(format!(
                    "System function `{name}` parameter `{field_name}` is keyword-only; system parameters must be positional"
                )));
            }

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

            result.push(Self::parse_annotation(
                py,
                &name,
                &field_name,
                &raw_annotation,
            )?);
        }

        Ok(result)
    }

    /// Lower one resolved parameter annotation of system `name`.
    fn parse_annotation(
        py: Python,
        name: &str,
        field_name: &str,
        raw_annotation: &Bound<'_, PyAny>,
    ) -> PyResult<SystemParam> {
        let (optional_resource, raw_annotation) = unwrap_optional_param(raw_annotation)?;

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
                name,
                field_name,
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

        let ty = if annotation.get_type().is(PyParamSetParam::type_object(py)) {
            if optional_resource || wrapper != SystemParamWrapper::Bare {
                return Err(PyTypeError::new_err(param_set_member_unsupported(
                    &annotation_type_name(&raw_annotation),
                )));
            }
            let descriptor = annotation.extract::<PyParamSetParam>()?;
            let members = descriptor
                .members()
                .iter()
                .enumerate()
                .map(|(index, member)| {
                    let member_name = format!("{field_name}.p{index}()");
                    let param = Self::parse_annotation(py, name, &member_name, member.bind(py))?;
                    if param.ty.allowed_in_param_set() {
                        Ok(param)
                    } else {
                        Err(PyTypeError::new_err(param_set_member_unsupported(
                            &annotation_type_name(member.bind(py)),
                        )))
                    }
                })
                .collect::<PyResult<Vec<_>>>()?;
            SystemParamType::ParamSet {
                members: Arc::new(members),
            }
        } else if annotation.get_type().is(PyQueryParam::type_object(py)) {
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
                    name,
                    field_name,
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
        } else if annotation.is(PyNonSendMarker::type_object(py)) {
            SystemParamType::NonSendMarker
        } else if annotation.is(PyGizmos::type_object(py)) {
            SystemParamType::Gizmos
        } else if annotation.get_type().is(MessageTypeParam::type_object(py)) {
            // MessageWriter[T], MessageReader[T], or MessageMutator[T]
            let message_param = annotation.extract::<MessageTypeParam>()?;
            match message_param.ty {
                MessageClass::Writer => SystemParamType::MessageWriter {
                    message_type: message_param.message_type,
                },
                MessageClass::Reader => SystemParamType::MessageReader {
                    message_type: message_param.message_type,
                },
                MessageClass::Mutator => {
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
            .is(PyRemovedComponentsParam::type_object(py))
        {
            SystemParamType::RemovedComponents {
                param: Arc::new(annotation.extract::<PyRemovedComponentsParam>()?),
            }
        } else if annotation.get_type().is(PyOnTypeParam::type_object(py)) {
            // On[E] or On[E, B] - extract event type and optional bundle filter
            let on_param = annotation.extract::<PyOnTypeParam>()?;
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
                    name,
                    field_name,
                    wrapper,
                    &annotation,
                    SystemParamRejection::BareResource,
                ));
            }
        } else {
            return Err(system_param_error(
                name,
                field_name,
                wrapper,
                &annotation,
                classify_system_param(wrapper, &annotation)?,
            ));
        };

        Ok(SystemParam {
            name: param_name,
            ty,
        })
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
/// SAFETY: asset parameters retain their class or use a registered native class.
/// Interpreter access requires attachment; Bevy access requires scheduler authority.
#[derive(Debug, Clone, Copy)]
pub(crate) struct AssetTypePtr(pub(crate) *const PyTypeObject);

// SAFETY: asset parameters retain the class; interpreter access requires attachment.
unsafe impl Send for AssetTypePtr {}
// SAFETY: the pointer carries identity only; dereferences require retained ownership.
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
    /// Bevy's `NonSendMarker`: pins the system to the main thread.
    NonSendMarker,
    MessageWriter {
        message_type: MessageType,
    },
    MessageReader {
        message_type: MessageType,
    },
    MessageMutator {
        message_type: MessageType,
    },
    /// Members borrowed one at a time through `p0()` to `p7()`.
    ParamSet {
        members: Arc<Vec<SystemParam>>,
    },
    RemovedComponents {
        param: Arc<PyRemovedComponentsParam>,
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
                param: Arc::new(param.as_ref().clone()),
            },
            SystemParamType::View { param } => SystemParamType::View {
                param: Arc::new(param.as_ref().clone()),
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
            SystemParamType::NonSendMarker => SystemParamType::NonSendMarker,
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
            SystemParamType::ParamSet { members } => SystemParamType::ParamSet {
                members: Arc::clone(members),
            },
            SystemParamType::RemovedComponents { param } => SystemParamType::RemovedComponents {
                param: Arc::new(param.as_ref().clone()),
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

impl SystemParamType {
    /// Whether a `ParamSet` may contain this parameter: each member is built
    /// on demand for one borrow, so it must not need the call frame itself.
    fn allowed_in_param_set(&self) -> bool {
        matches!(
            self,
            SystemParamType::Query { .. }
                | SystemParamType::View { .. }
                | SystemParamType::Resource { .. }
                | SystemParamType::Assets { .. }
                | SystemParamType::MessageReader { .. }
                | SystemParamType::MessageWriter { .. }
                | SystemParamType::MessageMutator { .. }
        )
    }
}

/// The parameters in signature order with every `ParamSet` replaced by its
/// members, the order in which run state such as query caches is built.
pub(crate) fn param_leaves(params: &[SystemParam]) -> Vec<&SystemParam> {
    params
        .iter()
        .flat_map(|param| match &param.ty {
            SystemParamType::ParamSet { members } => members.iter().collect(),
            _ => vec![param],
        })
        .collect()
}
