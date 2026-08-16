use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemStruct, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::util::{AssetArgs, find_storage_field_type, reflect_registration_tokens};

/// Generates boilerplate implementations for PyBevy asset wrapper types.
///
/// This macro generates:
/// - `impl NativeAsset for PyType`
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - Helper methods: `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage
///
/// ```rust,ignore
/// #[native_asset(Shader)]  // PyAssetType variant name
/// #[pyclass(name = "Shader", extends = PyAsset)]
/// #[derive(Debug, Clone)]
/// pub struct PyShader {
///     storage: AssetStorage<Shader>,  // Bevy type is inferred from here
/// }
/// ```
///
/// The macro parses the `storage` field to extract the Bevy asset type (`Shader` in this case).
pub fn native_asset(attr: TokenStream, item: TokenStream) -> TokenStream {
    let _args = parse_macro_input!(attr as AssetArgs);
    let input = parse_macro_input!(item as ItemStruct);

    let py_type = &input.ident;

    // Find the storage field and extract the Bevy type
    let bevy_type = match find_storage_field_type(&input, "AssetStorage") {
        Ok(ty) => ty.clone(),
        Err(e) => return e.to_compile_error().into(),
    };

    let expanded = quote! {
        #input

        impl NativeAsset for #py_type {
            type Asset = #bevy_type;

            fn take(&mut self) -> PyResult<Self::Asset> {
                Ok(self.storage.take()?)
            }
        }

        impl From<#bevy_type> for #py_type {
            fn from(asset: #bevy_type) -> Self {
                Self {
                    storage: AssetStorage::owned(asset),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(py_asset: #py_type) -> PyResult<Self> {
                Ok(py_asset.storage.into_owned()?)
            }
        }

        impl #py_type {
            /// Create from an owned asset value. Returns tuple for PyO3 class inheritance.
            pub fn from_owned(asset: #bevy_type) -> (Self, PyAsset) {
                (Self { storage: AssetStorage::owned(asset) }, PyAsset)
            }

            /// Create from a borrowed asset storage (for asset iteration).
            pub fn from_borrowed(storage: AssetStorage<#bevy_type>) -> (Self, PyAsset) {
                (Self { storage }, PyAsset)
            }

            #[inline(always)]
            pub fn as_ref(&self) -> PyResult<pybevy_core::StorageRef<'_, #bevy_type>> {
                Ok(self.storage.as_ref()?)
            }

            #[inline(always)]
            pub fn as_mut(&mut self) -> PyResult<pybevy_core::StorageMut<'_, #bevy_type>> {
                Ok(self.storage.as_mut()?)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generates storage boilerplate for asset wrappers in feature crates.
///
/// Unlike `native_asset`, this macro does NOT generate PyAssetType enum handling,
/// making it usable in feature crates that don't have access to `PyAssetType`.
///
/// This macro generates:
/// - `impl NativeAsset for PyType`
/// - `impl Clone for PyType` (if Bevy type is Clone)
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - Helper methods: `from_owned()`, `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage in feature crates (e.g., pybevy_audio)
///
/// ```rust,ignore
/// #[pyasset(AudioSource)]
/// #[pyclass(name = "AudioSource", extends = PyAsset)]
/// pub struct PyAudioSource {
///     pub(crate) storage: AssetStorage<AudioSource>,
/// }
/// ```
///
/// For assets where Clone is not implemented, use `no_clone`:
///
/// ```rust,ignore
/// #[pyasset(SomeAsset, no_clone)]
/// #[pyclass(name = "SomeAsset", extends = PyAsset)]
/// pub struct PySomeAsset {
///     pub(crate) storage: AssetStorage<SomeAsset>,
/// }
/// ```
///
/// With bridge generation:
///
/// ```rust
/// #[pyasset(Mesh, bridge)]
/// #[pyasset(TextureAtlasLayout, bridge, not_loadable)]
/// ```
pub fn pyasset(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct AssetStorageArgs {
        bevy_type: Type,
        no_clone: bool,
        bridge: bool,
        bridge_name: Option<String>,
        not_loadable: bool,
        material: bool,
        input_converter: bool,
    }

    impl Parse for AssetStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let mut no_clone = false;
            let mut bridge = false;
            let mut bridge_name = None;
            let mut not_loadable = false;
            let mut input_converter = false;
            let mut material = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if !input.peek(syn::Ident) && !input.peek(syn::LitStr) {
                    break;
                }
                if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                } else {
                    let ident: Ident = input.parse()?;
                    match ident.to_string().as_str() {
                        "no_clone" => no_clone = true,
                        "bridge" => bridge = true,
                        "not_loadable" => not_loadable = true,
                        "material" => material = true,
                        "input_converter" => input_converter = true,
                        other => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!(
                                    "unknown option '{}', expected one of: no_clone, bridge, not_loadable, input_converter, material",
                                    other
                                ),
                            ));
                        }
                    }
                }
            }

            if input_converter && !bridge {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "`input_converter` requires `bridge`",
                ));
            }

            Ok(AssetStorageArgs {
                bevy_type,
                no_clone,
                bridge,
                bridge_name,
                not_loadable,
                input_converter,
                material,
            })
        }
    }

    let args = parse_macro_input!(attr as AssetStorageArgs);
    let bevy_type = &args.bevy_type;
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    // `material` inserts PyMaterial between PyAsset and the wrapper, mirroring
    // `impl Material for T` in Bevy. The assertion makes a mislabelled wrapper a
    // compile error; the reverse direction is covered by the issubclass pins.
    let (initializer, constructor_return, material_assert) = if args.material {
        (
            quote! {
                |value| pyo3::PyClassInitializer::from(pybevy_core::PyAsset)
                    .add_subclass(pybevy_core::PyMaterial)
                    .add_subclass(value)
            },
            quote! { pyo3::PyClassInitializer<Self> },
            quote! {
                const _: fn() = {
                    fn assert_material<T: bevy::pbr::Material>() {}
                    assert_material::<#bevy_type>
                };
            },
        )
    } else {
        // Unchanged for every other asset: the tuple is the documented return
        // type and downstream callers destructure it.
        (
            quote! { |value| (value, pybevy_core::PyAsset) },
            quote! { (Self, pybevy_core::PyAsset) },
            quote! {},
        )
    };

    let clone_impl = if args.no_clone {
        quote! {}
    } else {
        quote! {
            impl Clone for #py_type {
                fn clone(&self) -> Self {
                    Self {
                        storage: self.storage.clone(),
                    }
                }
            }
        }
    };

    let bridge_tokens = if args.bridge {
        generate_asset_bridge_tokens(
            bevy_type,
            py_type,
            args.bridge_name.as_deref(),
            args.not_loadable,
            args.input_converter,
            args.material,
        )
    } else {
        quote! {}
    };

    let expanded = quote! {
        #input

        #material_assert

        impl pybevy_core::NativeAsset for #py_type {
            type Asset = #bevy_type;

            fn take(&mut self) -> PyResult<Self::Asset> {
                Ok(self.storage.take()?)
            }
        }

        #clone_impl

        impl From<#bevy_type> for #py_type {
            fn from(asset: #bevy_type) -> Self {
                Self {
                    storage: pybevy_core::AssetStorage::owned(asset),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(py_asset: #py_type) -> PyResult<Self> {
                Ok(py_asset.storage.into_owned()?)
            }
        }

        impl #py_type {
            /// Create from an owned asset value, initialized for PyO3 class inheritance.
            pub fn from_owned(asset: #bevy_type) -> #constructor_return {
                (#initializer)(Self { storage: pybevy_core::AssetStorage::owned(asset) })
            }

            /// Create from a borrowed asset storage (for asset iteration).
            pub fn from_borrowed(storage: pybevy_core::AssetStorage<#bevy_type>) -> #constructor_return {
                (#initializer)(Self { storage })
            }

            #[inline(always)]
            pub fn as_ref(&self) -> PyResult<pybevy_core::StorageRef<'_, #bevy_type>> {
                Ok(self.storage.as_ref()?)
            }

            #[inline(always)]
            pub fn as_mut(&mut self) -> PyResult<pybevy_core::StorageMut<'_, #bevy_type>> {
                Ok(self.storage.as_mut()?)
            }
        }

        #bridge_tokens
    };

    TokenStream::from(expanded)
}

/// Shared asset bridge code generation used by `#[pyasset(..., bridge)]`.
pub(crate) fn generate_asset_bridge_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    bridge_name_override: Option<&str>,
    not_loadable: bool,
    input_converter: bool,
    material: bool,
) -> proc_macro2::TokenStream {
    let bridge_initializer = if material {
        quote! {
            |value| pyo3::PyClassInitializer::from(pybevy_core::PyAsset)
                .add_subclass(pybevy_core::PyMaterial)
                .add_subclass(value)
        }
    } else {
        quote! { |value| (value, pybevy_core::PyAsset) }
    };

    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = bridge_name_override.map(String::from).unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let asset_name = &bridge_name_str;

    let is_loadable_impl = if not_loadable {
        quote! {
            fn is_loadable(&self) -> bool {
                false
            }
        }
    } else {
        quote! {}
    };

    let try_convert_input_impl = if input_converter {
        quote! {
            fn try_convert_input<'py>(
                &self,
                asset: &pyo3::Bound<'py, pyo3::PyAny>,
                py: pyo3::Python<'py>,
            ) -> pyo3::PyResult<Option<pyo3::Bound<'py, pyo3::PyAny>>> {
                <#py_type as pybevy_core::AssetInputConverter>::try_convert_input(asset, py)
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        /// Bridge for #asset_name asset
        pub struct #bridge_name;

        impl pybevy_core::AssetBridge for #bridge_name {
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
                #asset_name
            }

            fn resource_id(&self, world: &bevy::ecs::world::World) -> Option<bevy::ecs::component::ComponentId> {
                world.components().component_id::<bevy::asset::Assets<#bevy_type>>()
            }

            fn register_resource_id(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<bevy::asset::Assets<#bevy_type>>()
            }

            fn register_event_resource_id(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<bevy::ecs::message::Messages<bevy::asset::AssetEvent<#bevy_type>>>()
            }

            fn read_events(
                &self,
                world: &bevy::ecs::world::World,
                cursor_state: &mut Option<Box<dyn std::any::Any + Send + Sync>>,
            ) -> Vec<pybevy_core::AssetEventRecord> {
                let Some(messages) = world.get_resource::<bevy::ecs::message::Messages<bevy::asset::AssetEvent<#bevy_type>>>() else {
                    return Vec::new();
                };
                let mut cursor = cursor_state
                    .as_ref()
                    .and_then(|state| state.downcast_ref::<bevy::ecs::message::MessageCursor<bevy::asset::AssetEvent<#bevy_type>>>())
                    .cloned()
                    .unwrap_or_else(|| messages.get_cursor());
                let events = cursor
                    .read(messages)
                    .map(|event| match event {
                        bevy::asset::AssetEvent::Added { id } => pybevy_core::AssetEventRecord::Added { id: id.untyped() },
                        bevy::asset::AssetEvent::Modified { id } => pybevy_core::AssetEventRecord::Modified { id: id.untyped() },
                        bevy::asset::AssetEvent::Removed { id } => pybevy_core::AssetEventRecord::Removed { id: id.untyped() },
                        bevy::asset::AssetEvent::Unused { id } => pybevy_core::AssetEventRecord::Unused { id: id.untyped() },
                        bevy::asset::AssetEvent::LoadedWithDependencies { id } => pybevy_core::AssetEventRecord::LoadedWithDependencies { id: id.untyped() },
                    })
                    .collect();
                *cursor_state = Some(Box::new(cursor));
                events
            }

            fn clear_events(&self, world: &mut bevy::ecs::world::World) {
                if let Some(mut messages) = world.get_resource_mut::<bevy::ecs::message::Messages<bevy::asset::AssetEvent<#bevy_type>>>() {
                    messages.clear();
                }
            }

            fn events_is_empty(&self, world: &bevy::ecs::world::World) -> bool {
                world
                    .get_resource::<bevy::ecs::message::Messages<bevy::asset::AssetEvent<#bevy_type>>>()
                    .is_none_or(|messages| messages.is_empty())
            }

            fn event_count(&self, world: &bevy::ecs::world::World) -> usize {
                world
                    .get_resource::<bevy::ecs::message::Messages<bevy::asset::AssetEvent<#bevy_type>>>()
                    .map_or(0, |messages| messages.len())
            }

            fn register_load_failed_resource_id(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<bevy::ecs::message::Messages<bevy::asset::AssetLoadFailedEvent<#bevy_type>>>()
            }

            fn read_load_failed_events(
                &self,
                world: &bevy::ecs::world::World,
                cursor_state: &mut Option<Box<dyn std::any::Any + Send + Sync>>,
            ) -> Vec<pybevy_core::AssetLoadFailedRecord> {
                let Some(messages) = world.get_resource::<bevy::ecs::message::Messages<bevy::asset::AssetLoadFailedEvent<#bevy_type>>>() else {
                    return Vec::new();
                };
                let mut cursor = cursor_state
                    .as_ref()
                    .and_then(|state| state.downcast_ref::<bevy::ecs::message::MessageCursor<bevy::asset::AssetLoadFailedEvent<#bevy_type>>>())
                    .cloned()
                    .unwrap_or_else(|| messages.get_cursor());
                let events = cursor
                    .read(messages)
                    .map(|event| pybevy_core::AssetLoadFailedRecord {
                        id: event.id.untyped(),
                        path: event.path.clone(),
                        error: event.error.to_string(),
                    })
                    .collect();
                *cursor_state = Some(Box::new(cursor));
                events
            }

            fn clear_load_failed_events(&self, world: &mut bevy::ecs::world::World) {
                if let Some(mut messages) = world.get_resource_mut::<bevy::ecs::message::Messages<bevy::asset::AssetLoadFailedEvent<#bevy_type>>>() {
                    messages.clear();
                }
            }

            fn load_failed_events_is_empty(&self, world: &bevy::ecs::world::World) -> bool {
                world
                    .get_resource::<bevy::ecs::message::Messages<bevy::asset::AssetLoadFailedEvent<#bevy_type>>>()
                    .is_none_or(|messages| messages.is_empty())
            }

            fn load_failed_event_count(&self, world: &bevy::ecs::world::World) -> usize {
                world
                    .get_resource::<bevy::ecs::message::Messages<bevy::asset::AssetLoadFailedEvent<#bevy_type>>>()
                    .map_or(0, |messages| messages.len())
            }

            #is_loadable_impl

            #try_convert_input_impl

            fn get(
                &self,
                world: &bevy::ecs::world::World,
                id: bevy::asset::UntypedAssetId,
                validity: pybevy_core::ValidityFlagWithMode,
                borrow_counter: pybevy_core::AssetBorrowCounter,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                use bevy::asset::Assets;

                let resource_state = borrow_counter.scope().resource_state().clone();
                let _resource_guard = resource_state.try_read()?;
                let world_cell = world.as_unsafe_world_cell_readonly();
                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_id = id.typed::<#bevy_type>();
                match assets.get(typed_id) {
                    Some(asset) => {
                        let ptr = asset as *const #bevy_type;
                        // SAFETY: `ptr` is derived from a valid Bevy `Assets` borrow. The `validity` flag ensures the storage is invalidated before the borrow expires.
                        let storage = unsafe {
                            pybevy_core::AssetStorage::borrowed_readonly_tracked(
                                ptr,
                                world_cell,
                                id,
                                validity,
                                borrow_counter,
                            )
                        }?;
                        let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                        Ok(Some(obj.into_any()))
                    }
                    None => Ok(None),
                }
            }

            fn get_mut(
                &self,
                world_cell: bevy::ecs::world::unsafe_world_cell::UnsafeWorldCell<'_>,
                id: bevy::asset::UntypedAssetId,
                validity: pybevy_core::ValidityFlagWithMode,
                borrow_counter: pybevy_core::AssetBorrowCounter,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                use bevy::asset::Assets;

                let resource_state = borrow_counter.scope().resource_state().clone();
                let _resource_guard = resource_state.try_read()?;
                // SAFETY: the caller declared mutable scheduler access to the
                // exact resource. Wrapper creation intentionally derives only
                // a shared pointer; its first write re-derives mutable access.
                let assets = unsafe { world_cell.get_resource::<Assets<#bevy_type>>() }
                    .ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err(
                            concat!("Assets<", #asset_name, "> resource not found")
                        )
                    })?;

                let typed_id = id.typed::<#bevy_type>();
                match assets.get(typed_id) {
                    Some(asset) => {
                        let ptr = asset as *const #bevy_type;
                        // SAFETY: `ptr` is used for reads only. Mutable access
                        // is lazily re-derived through the retained world cell.
                        let storage = unsafe {
                            pybevy_core::AssetStorage::borrowed_mut_tracked(
                                ptr,
                                world_cell,
                                typed_id,
                                validity,
                                borrow_counter,
                            )
                        }?;
                        let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                        Ok(Some(obj.into_any()))
                    }
                    None => Ok(None),
                }
            }

            fn add(
                &self,
                world: &mut bevy::ecs::world::World,
                asset: &pyo3::Bound<pyo3::PyAny>,
                _py: pyo3::Python,
            ) -> pyo3::PyResult<bevy::asset::UntypedHandle> {
                use bevy::asset::Assets;
                use pybevy_core::NativeAsset;

                pybevy_core::ensure_asset_access_registry(world);
                let resource_state = world
                    .resource::<pybevy_core::AssetAccessRegistry>()
                    .state_for(std::any::TypeId::of::<#bevy_type>(), #asset_name);
                let _resource_guard = resource_state.try_write()?;
                if resource_state.active() > 0 {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(
                        pybevy_core::AssetRuntimeError::BorrowedAssetsLive {
                            asset_name: #asset_name.to_owned(),
                        }
                        .to_string(),
                    ));
                }
                let mut py_asset = asset.extract::<pyo3::PyRefMut<#py_type>>()?;
                // Resolve the collection before take() so a failed add leaves the
                // Python wrapper still owning its asset.
                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;
                resource_state.advance_epoch();
                let native_asset = py_asset.take()?;

                let handle = assets.add(native_asset);
                Ok(handle.untyped())
            }

            fn remove(
                &self,
                world: &mut bevy::ecs::world::World,
                id: bevy::asset::UntypedAssetId,
            ) -> pyo3::PyResult<bool> {
                use bevy::asset::Assets;

                pybevy_core::ensure_asset_access_registry(world);
                let resource_state = world
                    .resource::<pybevy_core::AssetAccessRegistry>()
                    .state_for(std::any::TypeId::of::<#bevy_type>(), #asset_name);
                let _resource_guard = resource_state.try_write()?;
                if resource_state.active() > 0 {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(
                        pybevy_core::AssetRuntimeError::BorrowedAssetsLive {
                            asset_name: #asset_name.to_owned(),
                        }
                        .to_string(),
                    ));
                }
                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;
                resource_state.advance_epoch();

                Ok(assets.remove(id.typed::<#bevy_type>()).is_some())
            }

            fn len(&self, world: &bevy::ecs::world::World) -> pyo3::PyResult<usize> {
                use bevy::asset::Assets;

                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                Ok(assets.len())
            }

            fn contains(
                &self,
                world: &bevy::ecs::world::World,
                id: bevy::asset::UntypedAssetId,
            ) -> pyo3::PyResult<bool> {
                use bevy::asset::Assets;

                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                Ok(assets.contains(id.typed::<#bevy_type>()))
            }

            fn iter_pairs(
                &self,
                world: &bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                borrow_counter: pybevy_core::AssetBorrowCounter,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Vec<(bevy::asset::UntypedAssetId, pyo3::Py<pyo3::PyAny>)>> {
                use bevy::asset::Assets;

                let resource_state = borrow_counter.scope().resource_state().clone();
                let _resource_guard = resource_state.try_read()?;
                let world_cell = world.as_unsafe_world_cell_readonly();
                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let mut result = Vec::new();
                for (id, asset) in assets.iter() {
                    let untyped_id = id.untyped();
                    let ptr = asset as *const #bevy_type;
                    // SAFETY: `ptr` is derived from a valid Bevy `Assets` borrow. The `validity` flag ensures the storage is invalidated before the borrow expires.
                    let storage = unsafe {
                        pybevy_core::AssetStorage::borrowed_readonly_tracked(
                            ptr,
                            world_cell,
                            untyped_id,
                            validity.clone(),
                            borrow_counter.clone(),
                        )
                    }?;
                    let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                    result.push((untyped_id, obj.into_any()));
                }
                Ok(result)
            }

            fn remove_and_return(
                &self,
                world: &mut bevy::ecs::world::World,
                id: bevy::asset::UntypedAssetId,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                use bevy::asset::Assets;

                pybevy_core::ensure_asset_access_registry(world);
                let resource_state = world
                    .resource::<pybevy_core::AssetAccessRegistry>()
                    .state_for(std::any::TypeId::of::<#bevy_type>(), #asset_name);
                let _resource_guard = resource_state.try_write()?;
                if resource_state.active() > 0 {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(
                        pybevy_core::AssetRuntimeError::BorrowedAssetsLive {
                            asset_name: #asset_name.to_owned(),
                        }
                        .to_string(),
                    ));
                }
                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;
                resource_state.advance_epoch();

                match assets.remove(id.typed::<#bevy_type>()) {
                    Some(asset) => {
                        let py_asset = #py_type::from(asset);
                        let obj = pyo3::Py::new(py, (#bridge_initializer)(py_asset))?;
                        Ok(Some(obj.into_any()))
                    }
                    None => Ok(None),
                }
            }

            fn load(
                &self,
                asset_server: &bevy::asset::AssetServer,
                path: bevy::asset::AssetPath,
            ) -> bevy::asset::UntypedHandle {
                asset_server.load::<#bevy_type>(path).untyped()
            }

            fn get_handle(
                &self,
                asset_server: &bevy::asset::AssetServer,
                path: bevy::asset::AssetPath,
            ) -> Option<bevy::asset::UntypedHandle> {
                asset_server.get_handle::<#bevy_type>(path).map(|h| h.untyped())
            }

            fn clear_programmatic(&self, world: &mut bevy::ecs::world::World, verbose: bool) {
                use bevy::asset::{Assets, AssetServer};

                pybevy_core::ensure_asset_access_registry(world);
                let resource_state = world
                    .resource::<pybevy_core::AssetAccessRegistry>()
                    .state_for(std::any::TypeId::of::<#bevy_type>(), #asset_name);
                let Ok(_resource_guard) = resource_state.try_write() else {
                    return;
                };
                if resource_state.active() > 0 {
                    return;
                }
                let ids_to_remove: Vec<_> = {
                    let Some(asset_server) = world.get_resource::<AssetServer>() else {
                        // No AssetServer - clear all assets (no file-loaded assets to preserve)
                        if let Some(mut assets) = world.get_resource_mut::<Assets<#bevy_type>>() {
                            resource_state.advance_epoch();
                            let count = assets.len();
                            if count > 0 {
                                let handles: Vec<_> = assets.ids().map(|id| id.untyped()).collect();
                                for handle in handles {
                                    assets.remove(handle.typed::<#bevy_type>());
                                }
                                if verbose {
                                    eprintln!("      Cleared {} {} (no AssetServer)", count, #asset_name);
                                }
                            }
                        }
                        return;
                    };
                    let Some(assets) = world.get_resource::<Assets<#bevy_type>>() else {
                        return;
                    };
                    assets
                        .ids()
                        .filter(|id| asset_server.get_path(id.untyped()).is_none())
                        .collect()
                };

                if ids_to_remove.is_empty() {
                    return;
                }

                if let Some(mut assets) = world.get_resource_mut::<Assets<#bevy_type>>() {
                    resource_state.advance_epoch();
                    for id in &ids_to_remove {
                        assets.remove(*id);
                    }
                }

                if verbose {
                    let preserved = world.get_resource::<Assets<#bevy_type>>().map_or(0, |a| a.len());
                    eprintln!(
                        "      Cleared {} programmatic {} (preserved {} file-loaded)",
                        ids_to_remove.len(),
                        #asset_name,
                        preserved
                    );
                }
            }
        }
    };

    let inventory_submit = quote! {
        pybevy_core::inventory::submit!(pybevy_core::AssetBridgeRegistration {
            create: || std::sync::Arc::new(#bridge_name),
        });

        pybevy_core::inventory::submit!(pybevy_core::AssetCleanupRegistration {
            clear: |world, verbose| {
                use pybevy_core::AssetBridge;
                #bridge_name.clear_programmatic(world, verbose);
            },
            name: #asset_name,
        });
    };

    quote! {
        #expanded
        #inventory_submit
    }
}

/// Shared handle bridge code generation used by `#[pyhandle]`.
pub(crate) fn generate_handle_bridge_tokens(
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
        /// Bridge for #component_name component (handle wrapper)
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

            fn can_insert(&self) -> bool {
                true
            }

            fn register(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<#bevy_type>()
            }

            fn extract(
                &self,
                entity: &mut pybevy_core::FilteredEntityAccess,
                component_id: bevy::ecs::component::ComponentId,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                })?;

                // SAFETY: component_id was registered for #bevy_type, so the untyped pointer points to a valid instance.
                let component = unsafe { untyped.deref::<#bevy_type>() };
                let py_component = #py_type::from(component);
                let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                Ok(obj.into_any())
            }

            fn extract_fn(&self) -> pybevy_core::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut pybevy_core::FilteredEntityAccess,
                    component_id: bevy::ecs::component::ComponentId,
                    _validity: pybevy_core::ValidityFlagWithMode,
                    py: pyo3::Python,
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                    })?;

                    // SAFETY: component_id was registered for #bevy_type, so the untyped pointer points to a valid instance.
                    let component = unsafe { untyped.deref::<#bevy_type>() };
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                    Ok(obj.into_any())
                }
                extract_impl
            }

            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = #bevy_type::try_from(&*py_component)?;

                world.entity_mut(entity).insert(native);
                Ok(())
            }

            fn insert_into_entity(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = #bevy_type::try_from(&*py_component)?;

                entity.insert(native);
                Ok(())
            }

            fn prepare_uniform(
                &self,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<Box<dyn pybevy_core::PreparedUniformComponent>> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = #bevy_type::try_from(&*py_component)?;
                Ok(Box::new(pybevy_core::PreparedNativeUniform::new(native)))
            }

            fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
                entity.contains::<#bevy_type>()
            }

            unsafe fn extract_from_entity_ref(
                &self,
                entity_id: bevy::ecs::entity::Entity,
                world_ptr: *mut bevy::ecs::world::World,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                // SAFETY: caller guarantees world_ptr validity (trait contract).
                let Some(entity_ref) =
                    (unsafe { pybevy_core::entity_ref_from_ptr(entity_id, world_ptr) })
                else {
                    return Ok(None);
                };
                if let Some(component) = entity_ref.get::<#bevy_type>() {
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }

            unsafe fn extract_from_entity_mut(
                &self,
                entity_id: bevy::ecs::entity::Entity,
                world_ptr: *mut bevy::ecs::world::World,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                // SAFETY: caller guarantees world_ptr validity (trait contract).
                let Some(entity_ref) =
                    (unsafe { pybevy_core::entity_ref_from_ptr(entity_id, world_ptr) })
                else {
                    return Ok(None);
                };
                if let Some(component) = entity_ref.get::<#bevy_type>() {
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
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

/// Attribute proc macro for handle wrapper types.
///
/// Generates the ComponentBridge impl and inventory registration for structs
/// that wrap a `PyHandle` (e.g., `Mesh3d` wrapping `Handle<Mesh>`).
///
/// # Usage
///
/// ```rust,ignore
/// #[pyhandle(Mesh3d)]
/// #[pyclass(name = "Mesh3d", extends = PyComponent, frozen)]
/// pub struct PyMesh3d(pub PyHandle);
/// ```
///
/// For generic Bevy types:
/// ```rust,ignore
/// #[pyhandle(MeshMaterial3d<StandardMaterial>, "MeshMaterial3d")]
/// #[pyclass(name = "MeshMaterial3d", extends = PyComponent, frozen)]
/// pub struct PyMeshMaterial3d(pub PyHandle);
/// ```
pub fn pyhandle(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct HandleStorageArgs {
        bevy_type: Type,
        bridge_name: Option<String>,
        no_reflect: bool,
    }

    impl Parse for HandleStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let mut bridge_name = None;
            let mut no_reflect = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                } else if input.peek(syn::Ident) {
                    let ident: Ident = input.parse()?;
                    match ident.to_string().as_str() {
                        "no_reflect" => no_reflect = true,
                        other => {
                            return Err(syn::Error::new_spanned(
                                ident,
                                format!("unknown option '{}', expected no_reflect", other),
                            ));
                        }
                    }
                } else {
                    break;
                }
            }

            Ok(HandleStorageArgs {
                bevy_type,
                bridge_name,
                no_reflect,
            })
        }
    }

    let args = parse_macro_input!(attr as HandleStorageArgs);
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    let bridge_tokens = generate_handle_bridge_tokens(
        &args.bevy_type,
        py_type,
        args.bridge_name.as_deref(),
        args.no_reflect,
    );

    let expanded = quote! {
        #input
        #bridge_tokens
    };

    TokenStream::from(expanded)
}
