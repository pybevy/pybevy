use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{Fields, Ident, ItemStruct, parse_macro_input};

use crate::util::{
    FieldDef, generate_field_accessors, is_primitive_type, parse_field_annotation,
    pybevy_crate_paths, to_snake_case,
};

/// Derives a complete PyO3 component wrapper for a Bevy component.
///
/// Generates:
/// 1. `Py{Name}` wrapper struct with `ComponentStorage<T>` extending `PyComponent`
/// 2. `#[pymethods]` with `#[new]` constructor and field getters/setters
/// 3. `From`, `TryFrom`, helper methods (`from_owned`, `from_borrowed`, `as_ref`, `as_mut`)
/// 4. `{Name}Bridge` struct implementing `ComponentBridge`
/// 5. `register_{name}()` function for global registry
///
/// # Usage
///
/// ```rust,ignore
/// #[derive(Component, PyComponent)]
/// struct Health {
///     value: f32,
///     max: f32,
/// }
/// ```
///
/// Generates `PyHealth`, `HealthBridge`, and `register_health()`.
pub fn derive_py_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemStruct);
    let bevy_type = &input.ident;
    let bevy_type_str = bevy_type.to_string();

    // Resolve crate paths for generated code (handles internal vs external usage)
    let (core_path, pyo3_path) = pybevy_crate_paths();

    // Check for #[py_name = "..."] attribute on the struct
    let py_name_str = input
        .attrs
        .iter()
        .find_map(|attr| {
            if attr.path().is_ident("py_name") {
                attr.parse_args::<syn::LitStr>().ok().map(|lit| lit.value())
            } else {
                None
            }
        })
        .unwrap_or_else(|| bevy_type_str.clone());

    let py_type = Ident::new(&format!("Py{}", bevy_type), Span::call_site());
    let bridge_type = Ident::new(&format!("{}Bridge", bevy_type), Span::call_site());
    let register_fn = Ident::new(
        &format!("register_{}", to_snake_case(&bevy_type_str)),
        Span::call_site(),
    );

    // Extract fields
    let named_fields = match &input.fields {
        Fields::Named(fields) => &fields.named,
        _ => {
            return syn::Error::new_spanned(
                &input,
                "PyComponent can only be derived for structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut field_defs = Vec::new();
    let mut field_names = Vec::new();
    let mut field_types = Vec::new();

    for field in named_fields {
        let name = field.ident.clone().unwrap();
        let ty = &field.ty;
        let annotation = parse_field_annotation(&field.attrs);

        field_names.push(name.clone());
        field_types.push(ty.clone());
        field_defs.push(FieldDef {
            name,
            ty: ty.clone(),
            annotation,
        });
    }

    let accessors = generate_field_accessors(&py_type, &field_defs);

    // Generate constructor defaults: each field uses BevyType::default().field
    let constructor_defaults: Vec<proc_macro2::TokenStream> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(name, _ty)| {
            quote! { #name = #bevy_type::default().#name }
        })
        .collect();

    let constructor_params: Vec<proc_macro2::TokenStream> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(name, ty)| {
            quote! { #name: #ty }
        })
        .collect();

    let field_assignments: Vec<proc_macro2::TokenStream> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(name, ty)| {
            if is_primitive_type(ty) {
                quote! { #name }
            } else {
                quote! { #name: #name.into() }
            }
        })
        .collect();

    let expanded = quote! {
        #[#pyo3_path::pyclass(name = #py_name_str, extends = #core_path::PyComponent)]
        #[derive(Debug)]
        pub struct #py_type {
            storage: #core_path::ComponentStorage<#bevy_type>,
        }

        impl From<#bevy_type> for #py_type {
            fn from(component: #bevy_type) -> Self {
                Self {
                    storage: #core_path::ComponentStorage::owned(component),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = #pyo3_path::PyErr;

            fn try_from(py_component: #py_type) -> #pyo3_path::PyResult<Self> {
                Ok(py_component.storage.into_owned()?)
            }
        }

        impl TryFrom<&#bevy_type> for #py_type {
            type Error = #pyo3_path::PyErr;

            fn try_from(component: &#bevy_type) -> #pyo3_path::PyResult<Self> {
                Ok(Self {
                    storage: #core_path::ComponentStorage::owned(component.clone()),
                })
            }
        }

        impl #py_type {
            pub fn from_owned(component: #bevy_type) -> (Self, #core_path::PyComponent) {
                (Self { storage: #core_path::ComponentStorage::owned(component) }, #core_path::PyComponent)
            }

            pub fn from_borrowed(storage: #core_path::ComponentStorage<#bevy_type>) -> (Self, #core_path::PyComponent) {
                (Self { storage }, #core_path::PyComponent)
            }

            #[inline(always)]
            pub fn as_ref(&self) -> #pyo3_path::PyResult<&#bevy_type> {
                Ok(self.storage.as_ref()?)
            }

            #[inline(always)]
            pub fn as_mut(&mut self) -> #pyo3_path::PyResult<&mut #bevy_type> {
                Ok(self.storage.as_mut()?)
            }
        }

        #[#pyo3_path::pymethods]
        impl #py_type {
            #[new]
            #[pyo3(signature = (#(#constructor_defaults),*))]
            pub fn new(#(#constructor_params),*) -> (Self, #core_path::PyComponent) {
                Self::from_owned(#bevy_type {
                    #(#field_assignments),*
                })
            }

            #accessors
        }

        pub struct #bridge_type;

        impl #core_path::ComponentBridge for #bridge_type {
            fn bevy_type_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<#bevy_type>()
            }

            fn py_type_ptr(&self) -> *const #pyo3_path::ffi::PyTypeObject {
                use #pyo3_path::types::PyTypeMethods;
                #pyo3_path::Python::attach(|py| {
                    <#py_type as #pyo3_path::PyTypeInfo>::type_object(py).as_type_ptr()
                })
            }

            fn py_type<'py>(&self, py: #pyo3_path::Python<'py>) -> #pyo3_path::Bound<'py, #pyo3_path::types::PyType> {
                <#py_type as #pyo3_path::PyTypeInfo>::type_object(py)
            }

            fn name(&self) -> &'static str {
                #bevy_type_str
            }

            fn register(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<#bevy_type>()
            }

            fn extract(
                &self,
                entity: &mut #core_path::FilteredEntityAccess,
                component_id: bevy::ecs::component::ComponentId,
                validity: #core_path::ValidityFlagWithMode,
                py: #pyo3_path::Python,
            ) -> #pyo3_path::PyResult<#pyo3_path::Py<#pyo3_path::PyAny>> {
                let ptr = if validity.access_mode() == #core_path::AccessMode::Write {
                    let mut untyped = entity.get_mut_by_id(component_id).ok_or_else(|| {
                        #pyo3_path::exceptions::PyRuntimeError::new_err(concat!(#bevy_type_str, " not found"))
                    })?;
                    unsafe { untyped.as_mut().deref_mut::<#bevy_type>() as *mut #bevy_type }
                } else {
                    let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                        #pyo3_path::exceptions::PyRuntimeError::new_err(concat!(#bevy_type_str, " not found"))
                    })?;
                    // TODO(pybevy/pybevy#90): use a read-only ComponentStorage variant to avoid *const -> *mut cast
                    // SAFETY: component_id was registered for #bevy_type; pointer is not written through
                    // because AccessMode::Read prevents mutation at the Python boundary.
                    unsafe { untyped.deref::<#bevy_type>() as *const #bevy_type as *mut #bevy_type }
                };

                // SAFETY: ptr is from a valid Bevy entity borrow; validity flag invalidates storage when borrow expires.
                let storage = unsafe {
                    #core_path::ComponentStorage::borrowed(ptr, validity)
                };

                let obj = #pyo3_path::Py::new(py, #py_type::from_borrowed(storage))?;
                Ok(obj.into_any())
            }

            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                component: &#pyo3_path::Bound<#pyo3_path::PyAny>,
            ) -> #pyo3_path::PyResult<()> {
                use #pyo3_path::prelude::PyAnyMethods;
                let py_component = component.extract::<#pyo3_path::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.storage.as_ref()?.clone();

                world.entity_mut(entity).insert(native);
                Ok(())
            }

            fn insert_into_entity(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                component: &#pyo3_path::Bound<#pyo3_path::PyAny>,
            ) -> #pyo3_path::PyResult<()> {
                use #pyo3_path::prelude::PyAnyMethods;
                let py_component = component.extract::<#pyo3_path::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.storage.as_ref()?.clone();

                entity.insert(native);
                Ok(())
            }

            fn prepare_uniform(
                &self,
                component: &#pyo3_path::Bound<#pyo3_path::PyAny>,
            ) -> #pyo3_path::PyResult<Box<dyn #core_path::PreparedUniformComponent>> {
                use #pyo3_path::prelude::PyAnyMethods;
                let py_component = component.extract::<#pyo3_path::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.storage.as_ref()?.clone();
                Ok(Box::new(#core_path::PreparedNativeUniform::new(native)))
            }

            fn extract_fn(&self) -> #core_path::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut #core_path::FilteredEntityAccess,
                    component_id: bevy::ecs::component::ComponentId,
                    validity: #core_path::ValidityFlagWithMode,
                    py: #pyo3_path::Python,
                ) -> #pyo3_path::PyResult<#pyo3_path::Py<#pyo3_path::PyAny>> {
                    let ptr = if validity.access_mode() == #core_path::AccessMode::Write {
                        let mut untyped = entity.get_mut_by_id(component_id).ok_or_else(|| {
                            #pyo3_path::exceptions::PyRuntimeError::new_err("Component not found")
                        })?;
                        // SAFETY: component_id was registered for this type; MutUntyped guarantees valid mutable access.
                        unsafe { untyped.as_mut().deref_mut::<#bevy_type>() as *mut #bevy_type }
                    } else {
                        let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                            #pyo3_path::exceptions::PyRuntimeError::new_err("Component not found")
                        })?;
                        // TODO(pybevy/pybevy#90): use a read-only ComponentStorage variant to avoid *const -> *mut cast
                        // SAFETY: component_id was registered for this type; pointer is not written through
                        // because AccessMode::Read prevents mutation at the Python boundary.
                        unsafe { untyped.deref::<#bevy_type>() as *const #bevy_type as *mut #bevy_type }
                    };

                    // SAFETY: ptr is from a valid Bevy entity borrow; validity flag invalidates storage when borrow expires.
                    let storage = unsafe {
                        #core_path::ComponentStorage::borrowed(ptr, validity)
                    };

                    let obj = #pyo3_path::Py::new(py, #py_type::from_borrowed(storage))?;
                    Ok(obj.into_any())
                }
                extract_impl
            }

            fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
                entity.contains::<#bevy_type>()
            }

            unsafe fn extract_from_entity_ref(
                &self,
                entity_id: bevy::ecs::entity::Entity,
                world_ptr: *mut bevy::ecs::world::World,
                validity: #core_path::ValidityFlagWithMode,
                py: #pyo3_path::Python,
            ) -> #pyo3_path::PyResult<Option<#pyo3_path::Py<#pyo3_path::PyAny>>> {
                // SAFETY: caller guarantees world_ptr validity (trait contract). The handle
                // re-resolves the component's address per access.
                match unsafe {
                    #core_path::resolve_revalidating_component::<#bevy_type>(
                        entity_id, world_ptr, validity,
                    )
                } {
                    Some(storage) => {
                        let obj = #pyo3_path::Py::new(py, #py_type::from_borrowed(storage))?;
                        Ok(Some(obj.into_any()))
                    }
                    None => Ok(None),
                }
            }

            unsafe fn extract_from_entity_mut(
                &self,
                entity_id: bevy::ecs::entity::Entity,
                world_ptr: *mut bevy::ecs::world::World,
                validity: #core_path::ValidityFlagWithMode,
                py: #pyo3_path::Python,
            ) -> #pyo3_path::PyResult<Option<#pyo3_path::Py<#pyo3_path::PyAny>>> {
                // SAFETY: caller guarantees world_ptr validity (trait contract). `validity`
                // carries write access so mutations land on the live component.
                match unsafe {
                    #core_path::resolve_revalidating_component::<#bevy_type>(
                        entity_id, world_ptr, validity,
                    )
                } {
                    Some(storage) => {
                        let obj = #pyo3_path::Py::new(py, #py_type::from_borrowed(storage))?;
                        Ok(Some(obj.into_any()))
                    }
                    None => Ok(None),
                }
            }
        }

        pub fn #register_fn() {
            #core_path::registry::global_registry::register_component_bridge(#bridge_type);
        }
    };

    TokenStream::from(expanded)
}
