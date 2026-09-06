use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use bevy::ecs::{component::ComponentId, world::World};
use pybevy_bytecodevm::{
    bytecode::FieldType,
    view_engine::ViewFilter,
    view_runtime::{CachedViewCore, ResolvedViewSpec, ViewRuntimeError},
};
use pybevy_core::registry::global_registry;
use pyo3::{ffi::PyTypeObject, prelude::*, types::PyType};

use crate::ecs::{
    component_layout::{
        ComponentLayout, ComponentLayoutExt, ComponentStorageType, ComponentStorageTypeExt,
        PrimitiveTypeExt,
    },
    component_type::PyComponentType,
    view::view_param::{PyViewParam, ViewParamType},
};

/// Main-backend metadata resolved once for one View parameter.
///
/// The neutral core owns the safety-critical IDs, layouts, filters, and
/// mutability set. This adapter retains only the interpreter type lookup data
/// needed by the existing PyO3 proxy surface during migration.
pub(crate) struct CachedPyView {
    pub(crate) core: Arc<CachedViewCore>,
    component_ids: HashMap<PyComponentType, ComponentId>,
    pub(crate) component_types: Vec<PyComponentType>,
    pub(crate) mutable_components: HashSet<PyComponentType>,
}

impl CachedPyView {
    /// Resolve one parsed View parameter against the initialized World.
    ///
    /// # Safety
    ///
    /// `param` must be the same descriptor used to declare this system's
    /// scheduler access. For observer dispatch, the caller must instead hold
    /// exclusive World access through the complete View lifetime.
    pub(crate) unsafe fn build(
        world: &mut World,
        param: &PyViewParam,
        custom_component_ids: &HashMap<*const PyTypeObject, ComponentId>,
        py: Python,
    ) -> Result<Arc<Self>, ViewRuntimeError> {
        let mut component_types = Vec::with_capacity(param.parameters.len());
        let mut mutable_components = HashSet::new();
        for param_type in &param.parameters {
            let ViewParamType::Component { comp_type, mutable } = param_type;
            component_types.push(*comp_type);
            if *mutable {
                mutable_components.insert(*comp_type);
            }
        }

        let filter_types = param.with_filters.to_vec();
        let without_filter_types = param.without_filters.to_vec();
        let changed_filter_types = param.changed_filters.to_vec();
        let added_filter_types = param.added_filters.to_vec();

        let mut component_ids = HashMap::new();
        for component_type in component_types
            .iter()
            .chain(&filter_types)
            .chain(&without_filter_types)
            .chain(&changed_filter_types)
            .chain(&added_filter_types)
        {
            if component_ids.contains_key(component_type) {
                continue;
            }
            let component_id = match component_type {
                PyComponentType::Custom(type_ptr) => custom_component_ids
                    .get(type_ptr)
                    .copied()
                    .unwrap_or_else(|| component_type.register_simple(world, py)),
                PyComponentType::Dynamic(_) | PyComponentType::Resource(_) => {
                    component_type.register_simple(world, py)
                }
            };
            component_ids.insert(*component_type, component_id);
        }

        let data_ids: HashSet<ComponentId> = component_types
            .iter()
            .map(|component_type| component_ids[component_type])
            .collect();
        let mutable_ids = mutable_components
            .iter()
            .map(|component_type| component_ids[component_type])
            .collect();
        let allowed_fields = component_types
            .iter()
            .map(|component_type| {
                (
                    component_ids[component_type],
                    expanded_field_offsets(component_type, py),
                )
            })
            .collect();
        let component_strides = data_ids
            .iter()
            .map(|&component_id| {
                let stride = world
                    .components()
                    .get_info(component_id)
                    .expect("View component was registered immediately above")
                    .layout()
                    .size();
                (component_id, stride)
            })
            .collect();
        let filter = ViewFilter {
            component_ids: data_ids,
            with_ids: filter_types
                .iter()
                .map(|component_type| component_ids[component_type])
                .collect(),
            without_ids: without_filter_types
                .iter()
                .map(|component_type| component_ids[component_type])
                .collect(),
            changed_ids: changed_filter_types
                .iter()
                .map(|component_type| component_ids[component_type])
                .collect(),
            added_ids: added_filter_types
                .iter()
                .map(|component_type| component_ids[component_type])
                .collect(),
        };
        // SAFETY: IDs and live strides were resolved from this exact World;
        // allowed fields come from the registered bridge/custom layout for the
        // same types. The caller binds this same `param` to scheduler access or
        // holds exclusive observer World access, per this method's contract.
        let spec = unsafe {
            ResolvedViewSpec::new(
                world.id(),
                filter,
                mutable_ids,
                allowed_fields,
                component_strides,
            )
        }?;
        let core = Arc::new(CachedViewCore::new(spec, world)?);

        Ok(Arc::new(Self {
            core,
            component_ids,
            component_types,
            mutable_components,
        }))
    }

    pub(crate) fn component_id(&self, component_type: &PyComponentType) -> Option<ComponentId> {
        self.component_ids.get(component_type).copied()
    }
}

fn insert_expanded_field(
    set: &mut HashSet<(usize, FieldType)>,
    offset: usize,
    field_type: FieldType,
) {
    let lanes = match field_type {
        FieldType::Vec2 => 2,
        FieldType::Vec3 => 3,
        FieldType::Vec4 => 4,
        scalar => {
            set.insert((offset, scalar));
            return;
        }
    };
    for lane in 0..lanes {
        set.insert((offset + lane * 4, FieldType::F32));
    }
}

/// Resolve the legitimate primitive VM fields for one backend component type.
///
/// Components without View-compatible storage deliberately produce an empty
/// set, causing any attempted field program to fail closed during validation.
fn expanded_field_offsets(
    component_type: &PyComponentType,
    py: Python,
) -> HashSet<(usize, FieldType)> {
    let mut fields = HashSet::new();
    match component_type {
        PyComponentType::Custom(type_ptr) => {
            // SAFETY: registered type pointers live for the interpreter lifetime.
            let py_type =
                unsafe { Bound::from_borrowed_ptr(py, *type_ptr as *mut pyo3::ffi::PyObject) };
            if let Ok(class) = py_type.cast::<PyType>()
                && matches!(
                    ComponentStorageType::from_python_class(class)
                        .unwrap_or(ComponentStorageType::PyObject),
                    ComponentStorageType::Wrapper(_)
                )
                && let Ok(layout) = ComponentLayout::from_annotations(class)
            {
                for field in &layout.fields {
                    insert_expanded_field(
                        &mut fields,
                        field.offset,
                        field.field_type.to_field_type(),
                    );
                }
            }
        }
        PyComponentType::Dynamic(type_ptr) => {
            if let Some(bridge) = global_registry::get_bridge_by_py_type(*type_ptr)
                && let Some(view_bridge) = bridge.view_bridge()
            {
                let is_transform = bridge.name() == "Transform";
                for &name in (view_bridge.field_names)() {
                    if let Some(field) = (view_bridge.field_offset)(name) {
                        if is_transform && name == "rotation" {
                            for lane in 0..4 {
                                fields.insert((field.offset + lane * 4, FieldType::F32));
                            }
                        } else {
                            insert_expanded_field(&mut fields, field.offset, field.field_type);
                        }
                    }
                }
            }
        }
        PyComponentType::Resource(_) => {}
    }
    fields
}
