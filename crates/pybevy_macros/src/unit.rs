use quote::quote;
use syn::{Ident, Type};

use crate::util::reflect_registration_tokens;

/// Shared unit bridge code generation used by `#[pycomponent(..., unit, bridge)]`.
pub(crate) fn generate_unit_bridge_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    bridge_name_override: Option<&str>,
    no_reflect: bool,
) -> proc_macro2::TokenStream {
    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = bridge_name_override.map(String::from).unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let component_name = &bridge_name_str;

    let expanded = quote! {
        /// Bridge for #component_name unit/marker component
        pub struct #bridge_name;

        impl pybevy_core::ComponentBridge for #bridge_name {
            fn bevy_type_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<#bevy_type>()
            }

            fn py_type_ptr(&self) -> *const pyo3::ffi::PyTypeObject {
                pyo3::Python::attach(|py| {
                    <#py_type as pyo3::PyTypeInfo>::type_object(py).as_type_ptr()
                })
            }

            fn py_type<'py>(&self, py: pyo3::Python<'py>) -> pyo3::Bound<'py, pyo3::types::PyType> {
                <#py_type as pyo3::PyTypeInfo>::type_object(py)
            }

            fn name(&self) -> &'static str {
                #component_name
            }

            fn register(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<#bevy_type>()
            }

            #[inline(always)]
            fn extract(
                &self,
                entity: &mut pybevy_core::FilteredEntityAccess,
                component_id: bevy::ecs::component::ComponentId,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                // Unit components: just check if present
                if entity.get_by_id(component_id).is_some() {
                    let obj = pyo3::Py::new(py, (#py_type, pybevy_core::PyComponent))?;
                    Ok(obj.into_any())
                } else {
                    Err(pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found")))
                }
            }

            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                _component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                // Unit component: just insert the marker
                world.entity_mut(entity).insert(#bevy_type);
                Ok(())
            }

            fn insert_into_entity(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                _component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                // Unit component: just insert the marker
                entity.insert(#bevy_type);
                Ok(())
            }

            fn insert_bulk_uniform(
                &self,
                _component: &pyo3::Bound<pyo3::PyAny>,
                entities: &[bevy::ecs::entity::Entity],
                world: &mut bevy::ecs::world::World,
            ) -> pyo3::PyResult<()> {
                for &entity_id in entities {
                    world.entity_mut(entity_id).insert(#bevy_type);
                }
                Ok(())
            }

            fn extract_fn(&self) -> pybevy_core::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut pybevy_core::FilteredEntityAccess,
                    component_id: bevy::ecs::component::ComponentId,
                    _validity: pybevy_core::ValidityFlagWithMode,
                    py: pyo3::Python,
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    if entity.get_by_id(component_id).is_some() {
                        let obj = pyo3::Py::new(py, (#py_type, pybevy_core::PyComponent))?;
                        Ok(obj.into_any())
                    } else {
                        Err(pyo3::exceptions::PyRuntimeError::new_err("Unit component not found"))
                    }
                }
                extract_impl
            }

            fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
                entity.contains::<#bevy_type>()
            }

            fn extract_from_entity_ref(
                &self,
                entity: &bevy::ecs::world::EntityRef,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                if entity.contains::<#bevy_type>() {
                    let obj = pyo3::Py::new(py, (#py_type, pybevy_core::PyComponent))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }

            fn extract_from_entity_mut(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                if entity.contains::<#bevy_type>() {
                    let obj = pyo3::Py::new(py, (#py_type, pybevy_core::PyComponent))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }
        }
    };

    let reflect_submit = reflect_registration_tokens(bevy_type, no_reflect);
    let inventory_submit = quote! {
        pybevy_core::inventory::submit!(pybevy_core::ComponentBridgeRegistration {
            create: || std::sync::Arc::new(#bridge_name),
        });
        #reflect_submit
    };

    quote! {
        #expanded
        #inventory_submit
    }
}
