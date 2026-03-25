use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemStruct, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::util::{find_storage_field_type, to_snake_case};

/// Component macro variant type
#[derive(Debug)]
enum ComponentVariant {
    /// Standard ComponentStorage-based component
    Standard,
    /// Unit/marker component with no data
    Unit,
    /// ComponentStorage but extraction returns error
    NoExtract,
    /// Handle wrapper for asset handles: handle(AssetType)
    Handle { asset_type: Ident },
    /// Simple newtype wrapper for Copy/Clone types
    Newtype,
}

/// Arguments for native_component attribute macro
struct ComponentArgs {
    /// The PyComponentType variant name (e.g., Transform, MeshMaterial3d)
    type_variant: Ident,
    /// Optional generic argument for the Bevy type (e.g., StandardMaterial for MeshMaterial3d<StandardMaterial>)
    generic_arg: Option<Type>,
    /// The variant of component wrapper
    variant: ComponentVariant,
}

impl Parse for ComponentArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let type_variant: Ident = input.parse()?;

        // Check for generic argument: TypeName<GenericArg>
        let generic_arg = if input.peek(Token![<]) {
            input.parse::<Token![<]>()?;
            let arg: Type = input.parse()?;
            input.parse::<Token![>]>()?;
            Some(arg)
        } else {
            None
        };

        let variant = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let variant_name: Ident = input.parse()?;

            match variant_name.to_string().as_str() {
                "unit" => ComponentVariant::Unit,
                "no_extract" => ComponentVariant::NoExtract,
                "newtype" => ComponentVariant::Newtype,
                "handle" => {
                    // Parse handle(AssetType)
                    let content;
                    syn::parenthesized!(content in input);
                    let asset_type: Ident = content.parse()?;
                    ComponentVariant::Handle { asset_type }
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        variant_name,
                        format!(
                            "unknown variant '{}', expected one of: unit, no_extract, newtype, handle(AssetType)",
                            other
                        ),
                    ));
                }
            }
        } else {
            ComponentVariant::Standard
        };

        Ok(ComponentArgs {
            type_variant,
            generic_arg,
            variant,
        })
    }
}

/// Generates storage boilerplate for component wrappers in feature crates.
///
/// Unlike `native_component`, this macro does NOT generate `NativeComponent` impl,
/// making it usable in feature crates that don't have access to `PyComponentType`.
///
/// This macro generates:
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - `impl TryFrom<&BevyType> for PyType`
/// - Helper methods: `from_owned()`, `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage in feature crates (e.g., pybevy_transform)
///
/// ```rust
/// #[component_storage(Transform)]
/// #[pyclass(name = "Transform", extends = PyComponent, eq)]
/// #[derive(Debug, Clone)]
/// pub struct PyTransform {
///     pub(crate) storage: ComponentStorage<Transform>,
/// }
/// ```
///
/// # Non-Clone components
///
/// For engine-managed components that don't implement `Clone` (like `AudioSink`),
/// use the `no_clone` option:
///
/// ```rust
/// #[component_storage(AudioSink, no_clone)]
/// #[pyclass(name = "AudioSink", extends = PyComponent)]
/// pub struct PyAudioSink {
///     pub(crate) storage: ComponentStorage<AudioSink>,
/// }
/// ```
///
/// This skips the `From`, `TryFrom`, and `from_owned` implementations that require `Clone`.
///
/// The main crate then adds `NativeComponent` impl separately.
pub fn component_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct ComponentStorageArgs {
        bevy_type: Type,
        no_clone: bool,
    }

    impl Parse for ComponentStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let no_clone = if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                let ident: Ident = input.parse()?;
                if ident == "no_clone" {
                    true
                } else {
                    return Err(syn::Error::new_spanned(ident, "expected 'no_clone'"));
                }
            } else {
                false
            };
            Ok(ComponentStorageArgs {
                bevy_type,
                no_clone,
            })
        }
    }

    let args = parse_macro_input!(attr as ComponentStorageArgs);
    let bevy_type = &args.bevy_type;
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    if args.no_clone {
        // Non-Clone component: only generate borrowed access
        let expanded = quote! {
            #input

            impl #py_type {
                /// Create from a borrowed component storage (for query iteration).
                pub fn from_borrowed(storage: ComponentStorage<#bevy_type>) -> (Self, pybevy_core::PyComponent) {
                    (Self { storage }, pybevy_core::PyComponent)
                }

                #[inline(always)]
                pub fn as_ref(&self) -> PyResult<&#bevy_type> {
                    Ok(self.storage.as_ref()?)
                }

                #[inline(always)]
                pub fn as_mut(&mut self) -> PyResult<&mut #bevy_type> {
                    Ok(self.storage.as_mut()?)
                }
            }
        };
        TokenStream::from(expanded)
    } else {
        // Clone component: full implementation
        let expanded = quote! {
            #input

            impl From<#bevy_type> for #py_type {
                fn from(component: #bevy_type) -> Self {
                    Self {
                        storage: ComponentStorage::owned(component),
                    }
                }
            }

            impl TryFrom<#py_type> for #bevy_type {
                type Error = PyErr;

                fn try_from(py_component: #py_type) -> PyResult<Self> {
                    Ok(py_component.storage.into_owned()?)
                }
            }

            impl TryFrom<&#bevy_type> for #py_type {
                type Error = PyErr;

                fn try_from(component: &#bevy_type) -> PyResult<Self> {
                    Ok(Self {
                        storage: ComponentStorage::owned(component.clone()),
                    })
                }
            }

            impl #py_type {
                /// Create from an owned component value. Returns tuple for PyO3 class inheritance.
                pub fn from_owned(component: #bevy_type) -> (Self, pybevy_core::PyComponent) {
                    (Self { storage: ComponentStorage::owned(component) }, pybevy_core::PyComponent)
                }

                /// Create from a borrowed component storage (for query iteration).
                pub fn from_borrowed(storage: ComponentStorage<#bevy_type>) -> (Self, pybevy_core::PyComponent) {
                    (Self { storage }, pybevy_core::PyComponent)
                }

                #[inline(always)]
                pub fn as_ref(&self) -> PyResult<&#bevy_type> {
                    Ok(self.storage.as_ref()?)
                }

                #[inline(always)]
                pub fn as_mut(&mut self) -> PyResult<&mut #bevy_type> {
                    Ok(self.storage.as_mut()?)
                }
            }

            #[pyo3::pymethods]
            impl #py_type {
                pub fn __copy__(&self, py: Python) -> PyResult<Py<Self>> {
                    Py::new(py, (Self { storage: self.storage.clone() }, pybevy_core::PyComponent))
                }

                pub fn __deepcopy__(&self, py: Python, _memo: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
                    Py::new(py, (Self { storage: self.storage.clone() }, pybevy_core::PyComponent))
                }
            }
        };
        TokenStream::from(expanded)
    }
}

/// Generates a ComponentBridge struct and implementation for feature crates.
///
/// This macro generates:
/// - A bridge struct (e.g., `TransformBridge`)
/// - `impl ComponentBridge for XBridge` with all required methods
///
/// # Usage
///
/// ```rust
/// // In pybevy_transform/src/lib.rs
/// component_bridge!(Transform, PyTransform);
/// ```
///
/// This generates `TransformBridge` struct with full `ComponentBridge` impl.
///
/// For components where the Bevy type name differs from the bridge prefix:
/// ```rust
/// component_bridge!(AudioPlayer<AudioSource>, PyAudioPlayer, "AudioPlayer");
/// ```
///
/// For read-only components that cannot be spawned from Python:
/// ```rust
/// component_bridge!(AudioSink, PyAudioSink, no_insert);
/// ```
pub fn component_bridge(input: TokenStream) -> TokenStream {
    /// A single field in view_fields/batch_only_fields.
    /// Can be a named field or a tuple index with a Python alias.
    #[derive(Clone)]
    struct BridgeField {
        /// Token stream for field access: `.intensity` or `.0` or `.0.x`
        rust_accessor: proc_macro2::TokenStream,
        /// Token stream for offset_of!: `intensity` or `0` or `0.x`
        offset_path: proc_macro2::TokenStream,
        /// Python-visible name used for from_numpy kwargs and View field names
        python_name: Ident,
    }

    impl Parse for BridgeField {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            if input.peek(syn::LitInt) {
                // Tuple form: `0 as name` or `0.x as name`
                let idx: syn::LitInt = input.parse()?;
                let idx_val = idx.base10_parse::<usize>()?;
                let idx_lit = syn::Index::from(idx_val);

                let mut accessor_tokens = quote! { .#idx_lit };
                let mut offset_tokens = quote! { #idx_lit };

                // Parse optional `.ident` chain
                while input.peek(Token![.]) {
                    input.parse::<Token![.]>()?;
                    let sub: Ident = input.parse()?;
                    accessor_tokens = quote! { #accessor_tokens.#sub };
                    offset_tokens = quote! { #offset_tokens.#sub };
                }

                // Require `as name`
                input.parse::<Token![as]>()?;
                let python_name: Ident = input.parse()?;

                Ok(BridgeField {
                    rust_accessor: accessor_tokens,
                    offset_path: offset_tokens,
                    python_name,
                })
            } else {
                // Named field: `intensity`
                let ident: Ident = input.parse()?;
                let python_name = ident.clone();
                Ok(BridgeField {
                    rust_accessor: quote! { .#ident },
                    offset_path: quote! { #ident },
                    python_name,
                })
            }
        }
    }

    /// A view-only field with an explicit type annotation.
    #[derive(Clone)]
    #[allow(dead_code)]
    struct ViewOnlyField {
        /// Token stream for field access
        rust_accessor: proc_macro2::TokenStream,
        /// Token stream for offset_of!
        offset_path: proc_macro2::TokenStream,
        /// Python-visible name
        python_name: Ident,
        /// Explicit Rust type (needed because we can't use Default for type inference)
        field_type: Type,
    }

    impl Parse for ViewOnlyField {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            // Parse the field part (same logic as BridgeField)
            let (rust_accessor, offset_path, python_name) = if input.peek(syn::LitInt) {
                let idx: syn::LitInt = input.parse()?;
                let idx_val = idx.base10_parse::<usize>()?;
                let idx_lit = syn::Index::from(idx_val);

                let mut accessor_tokens = quote! { .#idx_lit };
                let mut offset_tokens = quote! { #idx_lit };

                while input.peek(Token![.]) {
                    input.parse::<Token![.]>()?;
                    let sub: Ident = input.parse()?;
                    accessor_tokens = quote! { #accessor_tokens.#sub };
                    offset_tokens = quote! { #offset_tokens.#sub };
                }

                input.parse::<Token![as]>()?;
                let python_name: Ident = input.parse()?;
                (accessor_tokens, offset_tokens, python_name)
            } else {
                let ident: Ident = input.parse()?;
                let python_name = ident.clone();
                (quote! { .#ident }, quote! { #ident }, python_name)
            };

            // Parse `: Type`
            input.parse::<Token![:]>()?;
            let field_type: Type = input.parse()?;

            Ok(ViewOnlyField {
                rust_accessor,
                offset_path,
                python_name,
                field_type,
            })
        }
    }

    struct BridgeArgs {
        bevy_type: Type,
        py_type: Ident,
        bridge_name: Option<String>,
        no_insert: bool,
        view_fields: Option<Vec<BridgeField>>,
        batch_only_fields: Option<Vec<BridgeField>>,
        view_only_fields: Option<Vec<ViewOnlyField>>,
    }

    impl Parse for BridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            input.parse::<Token![,]>()?;
            let py_type: Ident = input.parse()?;

            let mut bridge_name = None;
            let mut no_insert = false;
            let mut view_fields = None;
            let mut batch_only_fields = None;
            let mut view_only_fields = None;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                } else if input.peek(syn::Ident) {
                    let ident: Ident = input.parse()?;
                    if ident == "no_insert" {
                        no_insert = true;
                    } else if ident == "view_fields" {
                        // Parse: view_fields = [field1, field2, ...]
                        input.parse::<Token![=]>()?;
                        let content;
                        syn::bracketed!(content in input);
                        let fields = content.parse_terminated(BridgeField::parse, Token![,])?;
                        view_fields = Some(fields.into_iter().collect());
                    } else if ident == "batch_only_fields" {
                        // Parse: batch_only_fields = [field1, field2, ...]
                        input.parse::<Token![=]>()?;
                        let content;
                        syn::bracketed!(content in input);
                        let fields = content.parse_terminated(BridgeField::parse, Token![,])?;
                        batch_only_fields = Some(fields.into_iter().collect());
                    } else if ident == "view_only_fields" {
                        // Parse: view_only_fields = [name: Type, ...]
                        input.parse::<Token![=]>()?;
                        let content;
                        syn::bracketed!(content in input);
                        let fields = content.parse_terminated(ViewOnlyField::parse, Token![,])?;
                        view_only_fields = Some(fields.into_iter().collect());
                    } else {
                        return Err(syn::Error::new_spanned(
                            ident,
                            "expected 'no_insert', 'view_fields = [...]', 'batch_only_fields = [...]', 'view_only_fields = [...]', or a string literal for bridge name",
                        ));
                    }
                }
            }

            Ok(BridgeArgs {
                bevy_type,
                py_type,
                bridge_name,
                no_insert,
                view_fields,
                batch_only_fields,
                view_only_fields,
            })
        }
    }

    let args = parse_macro_input!(input as BridgeArgs);
    let bevy_type = &args.bevy_type;
    let py_type = &args.py_type;

    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = args.bridge_name.unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let component_name = &bridge_name_str;

    // Generate view_bridge method if view_fields (or view_only_fields) is specified
    let has_view_fields = args.view_fields.is_some() || args.view_only_fields.is_some();
    let view_bridge_impl = if has_view_fields {
        // Collect all view field names + match arms from view_fields
        let vf_entries: Vec<_> = args.view_fields.iter().flat_map(|fs| fs.iter()).map(|f| {
            let name_str = f.python_name.to_string();
            let accessor = &f.rust_accessor;
            let offset = &f.offset_path;
            quote! {
                #name_str => {
                    let (dtype, element_size) = pybevy_core::field_offset_view_meta_for(&default_val #accessor);
                    Some(pybevy_core::FieldOffset {
                        offset: std::mem::offset_of!(#bevy_type, #offset),
                        element_size,
                        dtype,
                    })
                }
            }
        }).collect();

        // Collect view_only_fields entries (use explicit type instead of type inference)
        let vof_entries: Vec<_> = args.view_only_fields.iter().flat_map(|fs| fs.iter()).map(|f| {
            let name_str = f.python_name.to_string();
            let offset = &f.offset_path;
            let field_type = &f.field_type;
            quote! {
                #name_str => {
                    let (dtype, element_size) = (<#field_type as pybevy_core::BatchableField>::VIEW_DTYPE, <#field_type as pybevy_core::BatchableField>::VIEW_ELEMENT_SIZE);
                    Some(pybevy_core::FieldOffset {
                        offset: std::mem::offset_of!(#bevy_type, #offset),
                        element_size,
                        dtype,
                    })
                }
            }
        }).collect();

        // All view field name strings (union of view_fields + view_only_fields)
        let all_view_names: Vec<String> = args
            .view_fields
            .iter()
            .flat_map(|fs| fs.iter())
            .map(|f| f.python_name.to_string())
            .chain(
                args.view_only_fields
                    .iter()
                    .flat_map(|fs| fs.iter())
                    .map(|f| f.python_name.to_string()),
            )
            .collect();
        let all_view_names_slice = &all_view_names;
        let field_names_array = quote! { &[#(#all_view_names_slice),*] };

        // Only emit `default_val` when there are view_fields (which need type inference)
        let default_val_line = if args.view_fields.as_ref().is_some_and(|vf| !vf.is_empty()) {
            quote! { let default_val = <#bevy_type>::default(); }
        } else {
            quote! {}
        };

        quote! {
            fn view_bridge(&self) -> Option<pybevy_core::ViewBridge> {
                // Generate field_offset function with dtype/element_size metadata
                fn field_offset_fn(field_name: &str) -> Option<pybevy_core::FieldOffset> {
                    #default_val_line
                    match field_name {
                        #(#vf_entries,)*
                        #(#vof_entries,)*
                        _ => None,
                    }
                }

                fn field_names_fn() -> &'static [&'static str] {
                    #field_names_array
                }

                fn component_id_fn(world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                    world.register_component::<#bevy_type>()
                }

                fn column_data_ptr_fn(
                    column: &bevy::ecs::storage::Column,
                    entity_count: usize,
                ) -> *const u8 {
                    let data_slice = unsafe { column.get_data_slice::<#bevy_type>(entity_count) };
                    data_slice.as_ptr() as *const u8
                }

                Some(pybevy_core::ViewBridge {
                    field_offset: field_offset_fn,
                    field_names: field_names_fn,
                    component_id: component_id_fn,
                    column_data_ptr: column_data_ptr_fn,
                })
            }
        }
    } else {
        quote! {}
    };

    // Generate insert methods based on no_insert flag
    let insert_impl = if args.no_insert {
        quote! {
            fn insert(
                &self,
                _world: &mut bevy::ecs::world::World,
                _entity: bevy::ecs::entity::Entity,
                _component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    concat!(#component_name, " cannot be spawned from Python")
                ))
            }

            fn insert_into_entity(
                &self,
                _entity: &mut bevy::ecs::world::EntityWorldMut,
                _component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    concat!(#component_name, " cannot be spawned from Python")
                ))
            }
        }
    } else {
        quote! {
            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.storage.as_ref()?.clone();

                world.entity_mut(entity).insert(native);
                Ok(())
            }

            fn insert_into_entity(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.storage.as_ref()?.clone();

                entity.insert(native);
                Ok(())
            }

            fn insert_bulk_uniform(
                &self,
                component: &pyo3::Bound<pyo3::PyAny>,
                entities: &[bevy::ecs::entity::Entity],
                world: &mut bevy::ecs::world::World,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.storage.as_ref()?.clone();
                for &entity_id in entities {
                    world.entity_mut(entity_id).insert(native.clone());
                }
                Ok(())
            }
        }
    };

    let expanded = quote! {
        /// Bridge for #component_name component
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
                entity: &mut bevy::ecs::world::FilteredEntityMut,
                component_id: bevy::ecs::component::ComponentId,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let mut untyped = entity.get_mut_by_id(component_id).ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                })?;

                let ptr = unsafe {
                    untyped.as_mut().deref_mut::<#bevy_type>() as *mut #bevy_type
                };

                let storage = unsafe {
                    pybevy_core::ComponentStorage::borrowed(ptr, validity)
                };

                let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                Ok(obj.into_any())
            }

            #insert_impl

            fn extract_fn(&self) -> pybevy_core::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut bevy::ecs::world::FilteredEntityMut,
                    component_id: bevy::ecs::component::ComponentId,
                    validity: pybevy_core::ValidityFlagWithMode,
                    py: pyo3::Python,
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    let mut untyped = entity.get_mut_by_id(component_id).ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err("Component not found")
                    })?;

                    let ptr = unsafe {
                        untyped.as_mut().deref_mut::<#bevy_type>() as *mut #bevy_type
                    };

                    let storage = unsafe {
                        pybevy_core::ComponentStorage::borrowed(ptr, validity)
                    };

                    let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                    Ok(obj.into_any())
                }
                extract_impl
            }

            fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
                entity.contains::<#bevy_type>()
            }

            fn extract_from_entity_ref(
                &self,
                entity: &bevy::ecs::world::EntityRef,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                if let Some(component) = entity.get::<#bevy_type>() {
                    let ptr = component as *const #bevy_type as *mut #bevy_type;
                    let storage = unsafe {
                        pybevy_core::ComponentStorage::borrowed(ptr, validity)
                    };
                    let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }

            fn extract_from_entity_mut(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                if let Some(mut component) = entity.get_mut::<#bevy_type>() {
                    let ptr = component.as_mut() as *mut #bevy_type;
                    let storage = unsafe {
                        pybevy_core::ComponentStorage::borrowed(ptr, validity)
                    };
                    let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }

            #view_bridge_impl
        }
    };

    // Generate batch-related functions when view_fields or batch_only_fields is present.
    // Batch code uses the union of view_fields + batch_only_fields (NOT view_only_fields).
    let all_batch_fields: Option<Vec<BridgeField>> =
        match (&args.view_fields, &args.batch_only_fields) {
            (Some(vf), Some(bo)) => {
                let mut all = vf.clone();
                all.extend(bo.iter().cloned());
                Some(all)
            }
            (Some(vf), None) => Some(vf.clone()),
            (None, Some(bo)) => Some(bo.clone()),
            (None, None) => None,
        };
    let batch_impl = if let Some(fields) = &all_batch_fields {
        let field_strs: Vec<String> = fields.iter().map(|f| f.python_name.to_string()).collect();

        // Generate function names using snake_case
        let snake_name = to_snake_case(&bridge_name_str);
        let meta_fn_name = quote::format_ident!("{}_batch_field_meta", snake_name);
        let insert_fn_name = quote::format_ident!("{}_batch_insert", snake_name);
        let register_fn_name = quote::format_ident!("register_{}_batch", snake_name);
        let from_numpy_fn_name = quote::format_ident!("{}_from_numpy", snake_name);

        // Generate field_meta entries using type-inference trick
        let field_meta_entries: Vec<_> = fields
            .iter()
            .map(|field| {
                let accessor = &field.rust_accessor;
                let name_str = field.python_name.to_string();
                quote! {
                    pybevy_core::batch_field_meta_for(&default_val #accessor, #name_str)
                }
            })
            .collect();

        // Generate array extraction + assignment in the insert function
        let array_extract_stmts: Vec<_> = fields
            .iter()
            .map(|field| {
                let name_str = field.python_name.to_string();
                let array_var = quote::format_ident!("{}_array", field.python_name);
                let slice_var = quote::format_ident!("{}_slice", field.python_name);
                quote! {
                    let #array_var: Option<numpy::PyReadonlyArray2<f32>> = batch
                        .get_field_array(py, #name_str)
                        .map(|a| -> pyo3::PyResult<_> { Ok(a.extract()?) })
                        .transpose()?;
                    let #slice_var = #array_var.as_ref().and_then(|a| a.as_slice().ok());
                }
            })
            .collect();

        let field_assignments: Vec<_> = fields
            .iter()
            .map(|field| {
                let slice_var = quote::format_ident!("{}_slice", field.python_name);
                let accessor = &field.rust_accessor;
                quote! {
                    if let Some(data) = #slice_var {
                        pybevy_core::set_field_from_numpy(&mut component #accessor, data, i);
                    }
                }
            })
            .collect();

        // Generate validation for from_numpy helper
        let field_validations: Vec<_> = fields.iter().map(|field| {
            let accessor = &field.rust_accessor;
            let name_str = field.python_name.to_string();
            let cols_var = quote::format_ident!("{}_cols", field.python_name);
            quote! {
                let #cols_var = pybevy_core::batch_field_meta_for(&default_val #accessor, #name_str).numpy_columns;
            }
        }).collect();

        let field_normalize_stmts: Vec<_> = fields.iter().map(|field| {
            let name_str = field.python_name.to_string();
            let cols_var = quote::format_ident!("{}_cols", field.python_name);
            quote! {
                if let Some(arr) = kwargs.get_item(#name_str)? {
                    let np = py.import("numpy")?;
                    let arr_bound = &arr;
                    let ndim: usize = arr_bound.getattr("ndim")?.extract()?;

                    // Normalize: scalar fields accept 1D, vector fields accept 2D
                    let normalized = if #cols_var == 1 {
                        // Scalar field: must be 1D
                        if ndim != 1 {
                            return Err(pyo3::exceptions::PyValueError::new_err(
                                format!("Field '{}' expects a 1D array, got {}D", #name_str, ndim)
                            ));
                        }
                        let count_val: usize = arr_bound.len()?;
                        // Reshape to (N, 1) for uniform handling
                        let reshaped = arr_bound.call_method1("reshape", ((count_val, 1usize),))?;
                        let contiguous = np.call_method1("ascontiguousarray", (&reshaped,))?;
                        contiguous.call_method1("astype", (np.getattr("float32")?,))?
                    } else {
                        // Vector field: accept 1D (single entity) or 2D with correct columns
                        if ndim == 1 {
                            // Single-entity shorthand or flat array
                            let len: usize = arr_bound.len()?;
                            if len % #cols_var != 0 {
                                return Err(pyo3::exceptions::PyValueError::new_err(
                                    format!("Field '{}' requires {} columns, but 1D array length {} is not divisible", #name_str, #cols_var, len)
                                ));
                            }
                            let count_val = len / #cols_var;
                            let reshaped = arr_bound.call_method1("reshape", ((count_val, #cols_var),))?;
                            let contiguous = np.call_method1("ascontiguousarray", (&reshaped,))?;
                            contiguous.call_method1("astype", (np.getattr("float32")?,))?
                        } else if ndim == 2 {
                            let shape = arr_bound.getattr("shape")?;
                            let cols: usize = shape.get_item(1)?.extract()?;
                            if cols != #cols_var {
                                return Err(pyo3::exceptions::PyValueError::new_err(
                                    format!("Field '{}' expects {} columns, got {}", #name_str, #cols_var, cols)
                                ));
                            }
                            let contiguous = np.call_method1("ascontiguousarray", (arr_bound,))?;
                            contiguous.call_method1("astype", (np.getattr("float32")?,))?
                        } else {
                            return Err(pyo3::exceptions::PyValueError::new_err(
                                format!("Field '{}' must be a 1D or 2D array, got {}D", #name_str, ndim)
                            ));
                        }
                    };

                    // Track count
                    let shape = normalized.getattr("shape")?;
                    let rows: usize = shape.get_item(0)?.extract()?;
                    if let Some((prev_count, ref prev_name)) = batch_count {
                        if rows != prev_count {
                            return Err(pyo3::exceptions::PyValueError::new_err(
                                format!("Array length mismatch: '{}' has {} rows but '{}' has {}", prev_name, prev_count, #name_str, rows)
                            ));
                        }
                    } else {
                        batch_count = Some((rows, #name_str.to_string()));
                    }

                    field_arrays.insert(#name_str.to_string(), normalized.unbind());
                }
            }
        }).collect();

        quote! {
            fn #meta_fn_name() -> Vec<pybevy_core::BatchFieldMeta> {
                let default_val = <#bevy_type>::default();
                vec![
                    #(#field_meta_entries),*
                ]
            }

            fn #insert_fn_name(
                py: pyo3::Python,
                batch: &pybevy_core::PyRustComponentBatch,
                entities: &[bevy::ecs::entity::Entity],
                world: &mut bevy::ecs::world::World,
            ) -> pyo3::PyResult<()> {
                #(#array_extract_stmts)*
                for (i, &entity_id) in entities.iter().enumerate() {
                    let mut component = <#bevy_type>::default();
                    #(#field_assignments)*
                    world.entity_mut(entity_id).insert(component);
                }
                Ok(())
            }

            pub fn #register_fn_name() {
                let meta = Box::leak(Box::new(pybevy_core::ComponentBatchMeta {
                    component_name: #component_name,
                    fields: Box::leak(#meta_fn_name().into_boxed_slice()),
                    insert_fn: #insert_fn_name,
                }));

                pyo3::Python::attach(|py| {
                    let ptr = <#py_type as pyo3::PyTypeInfo>::type_object(py).as_type_ptr() as usize;
                    pybevy_core::registry::global_registry::register_component_batch_meta(ptr, meta);
                });
            }

            pub fn #from_numpy_fn_name<'py>(
                py: pyo3::Python<'py>,
                kwargs: &pyo3::Bound<'py, pyo3::types::PyDict>,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                use pyo3::types::PyAnyMethods;
                let valid_fields: &[&str] = &[#(#field_strs),*];

                // Validate field names
                for key in kwargs.keys() {
                    let key_str: String = key.extract()?;
                    if !valid_fields.contains(&key_str.as_str()) {
                        return Err(pyo3::exceptions::PyValueError::new_err(
                            format!("Unknown field '{}'. Valid fields: {:?}", key_str, valid_fields)
                        ));
                    }
                }

                let default_val = <#bevy_type>::default();
                #(#field_validations)*

                let mut field_arrays = std::collections::HashMap::new();
                let mut batch_count: Option<(usize, String)> = None;

                #(#field_normalize_stmts)*

                let count = batch_count.map(|(c, _)| c).unwrap_or(0);
                if count == 0 {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "from_numpy() requires at least one field array"
                    ));
                }

                // Get the component type pointer
                let type_ptr = <#py_type as pyo3::PyTypeInfo>::type_object(py).as_type_ptr() as usize;

                let batch = pybevy_core::PyRustComponentBatch {
                    component_type_ptr: type_ptr,
                    field_arrays,
                    count,
                    component_name: #component_name.to_string(),
                };

                pyo3::Py::new(py, batch).map(|p| p.into_any())
            }
        }
    } else {
        quote! {}
    };

    // Generate #[pymethods] block with from_numpy staticmethod when batch fields exist
    let from_numpy_pymethods = if all_batch_fields.is_some() && !args.no_insert {
        let snake_name = to_snake_case(&bridge_name_str);
        let from_numpy_fn_name = quote::format_ident!("{}_from_numpy", snake_name);

        quote! {
            #[pyo3::pymethods]
            impl #py_type {
                /// Create a batch of components from numpy arrays for efficient bulk spawning.
                #[staticmethod]
                #[pyo3(signature = (**kwargs))]
                pub fn from_numpy(
                    py: pyo3::Python,
                    kwargs: Option<&pyo3::Bound<pyo3::types::PyDict>>,
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    #from_numpy_fn_name(
                        py,
                        kwargs.ok_or_else(|| {
                            pyo3::exceptions::PyValueError::new_err(
                                "from_numpy() requires keyword arguments",
                            )
                        })?,
                    )
                }
            }
        }
    } else {
        quote! {}
    };

    let final_expanded = quote! {
        #expanded
        #batch_impl
        #from_numpy_pymethods
    };

    TokenStream::from(final_expanded)
}

/// Generates boilerplate implementations for PyBevy component wrapper types.
///
/// This macro supports multiple variants for different component patterns:
///
/// # Standard (default) - ComponentStorage with full extraction
/// ```rust
/// #[native_component(Transform)]
/// #[pyclass(name = "Transform", extends = PyComponent)]
/// pub struct PyTransform {
///     storage: ComponentStorage<Transform>,
/// }
/// ```
///
/// # Unit - Marker components with no data
/// ```rust
/// #[native_component(VolumetricLight, unit)]
/// #[pyclass(name = "VolumetricLight", extends = PyComponent, frozen)]
/// pub struct PyVolumetricLight;
/// ```
///
/// # NoExtract - ComponentStorage but extraction returns error
/// ```rust
/// #[native_component(AudioSink, no_extract)]
/// #[pyclass(name = "AudioSink", extends = PyComponent)]
/// pub struct PyAudioSink {
///     storage: ComponentStorage<AudioSink>,
/// }
/// ```
///
/// # Handle - Asset handle wrapper with type validation
/// ```rust
/// #[native_component(Mesh3d, handle(Mesh))]
/// #[pyclass(name = "Mesh3d", extends = PyComponent, frozen)]
/// pub struct PyMesh3d(pub(crate) PyHandle);
/// ```
///
/// # Newtype - Simple wrapper for Copy/Clone types (e.g., enums)
/// ```rust
/// #[native_component(Tonemapping, newtype)]
/// #[pyclass(name = "Tonemapping", extends = PyComponent, frozen)]
/// pub struct PyTonemapping(pub(crate) Tonemapping);
/// ```
pub fn native_component(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ComponentArgs);
    let input = parse_macro_input!(item as ItemStruct);

    match args.variant {
        ComponentVariant::Standard => generate_standard_component(&input, &args.type_variant),
        ComponentVariant::Unit => generate_unit_component(&input, &args.type_variant),
        ComponentVariant::NoExtract => generate_no_extract_component(&input, &args.type_variant),
        ComponentVariant::Handle { asset_type } => {
            generate_handle_component(&input, &args.type_variant, &args.generic_arg, &asset_type)
        }
        ComponentVariant::Newtype => generate_newtype_component(&input, &args.type_variant),
    }
}

/// Generate standard ComponentStorage-based component (original behavior)
fn generate_standard_component(input: &ItemStruct, type_variant: &Ident) -> TokenStream {
    let py_type = &input.ident;

    // Find the storage field and extract the Bevy type
    let bevy_type = match find_storage_field_type(input, "ComponentStorage") {
        Ok(ty) => ty.clone(),
        Err(e) => return e.to_compile_error().into(),
    };

    let expanded = quote! {
        #input

        impl NativeComponent for #py_type {
            type Native = #bevy_type;

            fn component_type() -> PyComponentType {
                PyComponentType::#type_variant
            }
        }

        impl From<#bevy_type> for #py_type {
            fn from(component: #bevy_type) -> Self {
                Self {
                    storage: ComponentStorage::owned(component),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(py_component: #py_type) -> PyResult<Self> {
                Ok(py_component.storage.into_owned()?)
            }
        }

        impl TryFrom<&#bevy_type> for #py_type {
            type Error = PyErr;

            fn try_from(component: &#bevy_type) -> PyResult<Self> {
                Ok(Self {
                    storage: ComponentStorage::owned(component.clone()),
                })
            }
        }

        impl #py_type {
            /// Create from an owned component value. Returns tuple for PyO3 class inheritance.
            pub(crate) fn from_owned(component: #bevy_type) -> (Self, PyComponent) {
                (Self { storage: ComponentStorage::owned(component) }, PyComponent)
            }

            /// Create from a borrowed component storage (for query iteration).
            pub(crate) fn from_borrowed(storage: ComponentStorage<#bevy_type>) -> (Self, PyComponent) {
                (Self { storage }, PyComponent)
            }

            #[inline(always)]
            pub(crate) fn as_ref(&self) -> PyResult<&#bevy_type> {
                Ok(self.storage.as_ref()?)
            }

            #[inline(always)]
            pub(crate) fn as_mut(&mut self) -> PyResult<&mut #bevy_type> {
                Ok(self.storage.as_mut()?)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generate unit/marker component (no data, no storage)
fn generate_unit_component(input: &ItemStruct, type_variant: &Ident) -> TokenStream {
    let py_type = &input.ident;

    let expanded = quote! {
        #input

        impl NativeComponent for #py_type {
            type Native = #type_variant;

            fn component_type() -> PyComponentType {
                PyComponentType::#type_variant
            }
        }

        impl From<#type_variant> for #py_type {
            fn from(_: #type_variant) -> Self {
                Self
            }
        }

        impl TryFrom<#py_type> for #type_variant {
            type Error = PyErr;

            fn try_from(_: #py_type) -> PyResult<Self> {
                Ok(#type_variant)
            }
        }

        impl TryFrom<&#type_variant> for #py_type {
            type Error = PyErr;

            fn try_from(_: &#type_variant) -> PyResult<Self> {
                Ok(Self)
            }
        }

        impl #py_type {
            /// Create from an owned component value. Returns tuple for PyO3 class inheritance.
            pub(crate) fn from_owned(_: #type_variant) -> (Self, PyComponent) {
                (Self, PyComponent)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generate no_extract component (ComponentStorage but extraction fails)
fn generate_no_extract_component(input: &ItemStruct, type_variant: &Ident) -> TokenStream {
    let py_type = &input.ident;

    // Find the storage field and extract the Bevy type
    let bevy_type = match find_storage_field_type(input, "ComponentStorage") {
        Ok(ty) => ty.clone(),
        Err(e) => return e.to_compile_error().into(),
    };

    let error_msg = format!(
        "{} cannot be extracted - it is managed by the engine",
        type_variant
    );

    let expanded = quote! {
        #input

        impl NativeComponent for #py_type {
            type Native = #bevy_type;

            fn component_type() -> PyComponentType {
                PyComponentType::#type_variant
            }
        }

        impl From<#bevy_type> for #py_type {
            fn from(component: #bevy_type) -> Self {
                Self {
                    storage: ComponentStorage::owned(component),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(_: #py_type) -> PyResult<Self> {
                Err(pyo3::exceptions::PyRuntimeError::new_err(#error_msg))
            }
        }

        impl TryFrom<&#bevy_type> for #py_type {
            type Error = PyErr;

            fn try_from(_: &#bevy_type) -> PyResult<Self> {
                Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    concat!(stringify!(#py_type), " conversion not yet implemented")
                ))
            }
        }

        impl #py_type {
            /// Create from a borrowed component storage (for query iteration).
            pub(crate) fn from_borrowed(storage: ComponentStorage<#bevy_type>) -> (Self, PyComponent) {
                (Self { storage }, PyComponent)
            }

            #[inline(always)]
            pub(crate) fn as_ref(&self) -> PyResult<&#bevy_type> {
                Ok(self.storage.as_ref()?)
            }

            #[inline(always)]
            pub(crate) fn as_mut(&mut self) -> PyResult<&mut #bevy_type> {
                Ok(self.storage.as_mut()?)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generate handle wrapper component
///
/// Supports both simple types (e.g., `Mesh3d`) and generic types (e.g., `MeshMaterial3d<StandardMaterial>`).
/// For generic types, pass the full type in the attribute: `#[native_component(MeshMaterial3d<StandardMaterial>, handle(StandardMaterial))]`
fn generate_handle_component(
    input: &ItemStruct,
    type_variant: &Ident,
    generic_arg: &Option<Type>,
    asset_type: &Ident,
) -> TokenStream {
    let py_type = &input.ident;

    // Build the full Bevy type: either `TypeVariant` or `TypeVariant<GenericArg>`
    // Use parse_quote! to properly construct generic types
    let bevy_type: Type = match generic_arg {
        Some(arg) => syn::parse_quote! { #type_variant<#arg> },
        None => syn::parse_quote! { #type_variant },
    };

    let expanded = quote! {
        #input

        impl NativeComponent for #py_type {
            type Native = #bevy_type;

            fn component_type() -> PyComponentType {
                PyComponentType::#type_variant
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(value: #py_type) -> PyResult<Self> {
                // Use Self to avoid generic type parsing issues
                let handle = bevy::asset::Handle::try_from(value.0)?;
                Ok(Self(handle))
            }
        }

        impl TryFrom<&#bevy_type> for #py_type {
            type Error = PyErr;

            fn try_from(value: &#bevy_type) -> PyResult<Self> {
                Ok(#py_type(PyHandle::from(&value.0)))
            }
        }

        #[pymethods]
        impl #py_type {
            /// Create a new handle component.
            ///
            /// If no handle is provided, creates an invalid (default) handle
            /// matching Bevy's `Default` implementation.
            #[new]
            #[pyo3(signature = (handle = None))]
            pub fn new(handle: Option<PyHandle>) -> PyResult<(Self, PyComponent)> {
                match handle {
                    Some(h) => {
                        h.asset_type_internal().ensure(&PyAssetType::#asset_type)?;
                        Ok((Self(h), PyComponent))
                    }
                    None => {
                        // Create default (invalid) handle matching Bevy's Default
                        Ok((Self(PyHandle::default_for(PyAssetType::#asset_type)), PyComponent))
                    }
                }
            }

            pub fn handle(&self) -> PyResult<PyHandle> {
                Ok(self.0.clone())
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generate newtype wrapper component (for Copy/Clone types like enums)
fn generate_newtype_component(input: &ItemStruct, type_variant: &Ident) -> TokenStream {
    let py_type = &input.ident;

    let expanded = quote! {
        #input

        impl NativeComponent for #py_type {
            type Native = #type_variant;

            fn component_type() -> PyComponentType {
                PyComponentType::#type_variant
            }
        }

        impl From<#type_variant> for #py_type {
            fn from(value: #type_variant) -> Self {
                Self(value)
            }
        }

        impl TryFrom<#py_type> for #type_variant {
            type Error = PyErr;

            fn try_from(value: #py_type) -> PyResult<Self> {
                Ok(value.0)
            }
        }

        impl TryFrom<&#type_variant> for #py_type {
            type Error = PyErr;

            fn try_from(value: &#type_variant) -> PyResult<Self> {
                Ok(Self(value.clone()))
            }
        }

        impl #py_type {
            /// Create from an owned component value. Returns tuple for PyO3 class inheritance.
            pub(crate) fn from_owned(value: #type_variant) -> (Self, PyComponent) {
                (Self(value), PyComponent)
            }
        }
    };

    TokenStream::from(expanded)
}
