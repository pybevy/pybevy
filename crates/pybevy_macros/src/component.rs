use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Fields, Ident, ItemStruct, Path, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

use crate::{
    unit::generate_unit_bridge_tokens,
    util::{reflect_registration_tokens, to_snake_case},
};

#[derive(Clone)]
enum BatchFieldConstraint {
    Finite,
}

impl Parse for BatchFieldConstraint {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        match ident.to_string().as_str() {
            "finite" => Ok(Self::Finite),
            _ => Err(syn::Error::new_spanned(
                ident,
                "unknown batch field constraint, expected: finite",
            )),
        }
    }
}

impl BatchFieldConstraint {
    fn tokens(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Finite => {
                quote! { pybevy_core::batch_columns::BatchValueConstraint::Finite }
            }
        }
    }
}

/// A single field in view_fields/batch_only_fields.
/// Can be a named field or a tuple index with a Python alias.
#[derive(Clone)]
pub(crate) struct BridgeField {
    /// Token stream for field access: `.intensity` or `.0` or `.0.x`
    pub(crate) rust_accessor: proc_macro2::TokenStream,
    /// Token stream for offset_of!: `intensity` or `0` or `0.x`
    pub(crate) offset_path: proc_macro2::TokenStream,
    /// Python-visible name used for from_numpy kwargs and View field names
    pub(crate) python_name: Ident,
    /// Value-domain constraints applied by from_numpy after normalization.
    constraints: Vec<BatchFieldConstraint>,
}

impl Parse for BridgeField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let (rust_accessor, offset_path, python_name) = if input.peek(syn::LitInt) {
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

            (accessor_tokens, offset_tokens, python_name)
        } else {
            // Named field: `intensity`
            let ident: Ident = input.parse()?;
            let python_name = ident.clone();
            (quote! { .#ident }, quote! { #ident }, python_name)
        };

        let constraints = if input.peek(Token![=>]) {
            input.parse::<Token![=>]>()?;
            let content;
            syn::bracketed!(content in input);
            content
                .parse_terminated(BatchFieldConstraint::parse, Token![,])?
                .into_iter()
                .collect()
        } else {
            Vec::new()
        };

        Ok(BridgeField {
            rust_accessor,
            offset_path,
            python_name,
            constraints,
        })
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

/// Generates storage boilerplate for component wrappers in feature crates.
///
/// This macro generates:
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - `impl TryFrom<&BevyType> for PyType`
/// - Helper methods: `from_owned()`, `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage in feature crates (e.g., pybevy_transform)
///
/// ```rust,ignore
/// #[pycomponent(Transform)]
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
/// ```rust,ignore
/// #[pycomponent(AudioSink, no_clone)]
/// #[pyclass(name = "AudioSink", extends = PyComponent)]
/// pub struct PyAudioSink {
///     pub(crate) storage: ComponentStorage<AudioSink>,
/// }
/// ```
///
/// This skips the `From`, `TryFrom`, and `from_owned` implementations that require `Clone`.
/// For a value-like component that can be reconstructed from `&T`, use
/// `clone_with = path::to_fn` to keep Python insertion available.
fn extra_field_defaults(input: &ItemStruct) -> proc_macro2::TokenStream {
    let Fields::Named(fields) = &input.fields else {
        return proc_macro2::TokenStream::new();
    };
    let inits = fields
        .named
        .iter()
        .filter_map(|field| field.ident.as_ref())
        .filter(|name| *name != "storage")
        .map(|name| quote! { #name: Default::default() });
    quote! { #(#inits,)* }
}

pub fn pycomponent(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct ComponentStorageArgs {
        bevy_type: Type,
        no_clone: bool,
        no_insert: bool,
        unit: bool,
        bridge: bool,
        no_reflect: bool,
        bridge_name: Option<String>,
        view_fields: Option<Vec<BridgeField>>,
        batch_only_fields: Option<Vec<BridgeField>>,
        view_only_fields: Option<Vec<ViewOnlyField>>,
        materialize: Option<Path>,
        clone_with: Option<Path>,
    }

    impl Parse for ComponentStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let mut no_clone = false;
            let mut no_insert = false;
            let mut unit = false;
            let mut bridge = false;
            let mut no_reflect = false;
            let bridge_name = None;
            let mut view_fields = None;
            let mut batch_only_fields = None;
            let mut view_only_fields = None;
            let mut materialize = None;
            let mut clone_with = None;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if !input.peek(syn::Ident) {
                    break;
                }
                let ident: Ident = input.parse()?;
                match ident.to_string().as_str() {
                    "no_clone" => no_clone = true,
                    "no_insert" => no_insert = true,
                    "unit" => unit = true,
                    "bridge" => bridge = true,
                    "no_reflect" => no_reflect = true,
                    "materialize" => {
                        bridge = true;
                        input.parse::<Token![=]>()?;
                        materialize = Some(input.parse()?);
                    }
                    "clone_with" => {
                        bridge = true;
                        no_clone = true;
                        input.parse::<Token![=]>()?;
                        clone_with = Some(input.parse()?);
                    }
                    "view_fields" => {
                        bridge = true;
                        input.parse::<Token![=]>()?;
                        let content;
                        syn::bracketed!(content in input);
                        let fields = content.parse_terminated(BridgeField::parse, Token![,])?;
                        view_fields = Some(fields.into_iter().collect());
                    }
                    "batch_only_fields" => {
                        bridge = true;
                        input.parse::<Token![=]>()?;
                        let content;
                        syn::bracketed!(content in input);
                        let fields = content.parse_terminated(BridgeField::parse, Token![,])?;
                        batch_only_fields = Some(fields.into_iter().collect());
                    }
                    "view_only_fields" => {
                        bridge = true;
                        input.parse::<Token![=]>()?;
                        let content;
                        syn::bracketed!(content in input);
                        let fields = content.parse_terminated(ViewOnlyField::parse, Token![,])?;
                        view_only_fields = Some(fields.into_iter().collect());
                    }
                    other => {
                        return Err(syn::Error::new_spanned(
                            ident,
                            format!(
                                "unknown option '{}', expected one of: no_clone, no_insert, unit, bridge, no_reflect, materialize, clone_with, view_fields, batch_only_fields, view_only_fields",
                                other
                            ),
                        ));
                    }
                }
            }

            Ok(ComponentStorageArgs {
                bevy_type,
                no_clone,
                no_insert,
                unit,
                bridge,
                no_reflect,
                bridge_name,
                view_fields,
                batch_only_fields,
                view_only_fields,
                materialize,
                clone_with,
            })
        }
    }

    let args = parse_macro_input!(attr as ComponentStorageArgs);
    let bevy_type = &args.bevy_type;
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    // Fields beyond `storage` are wrapper-local state, defaulted by every generated
    // constructor so query-built instances do not inherit a stale value.
    let extra_field_inits = extra_field_defaults(&input);

    // Unit component: no storage field, just the struct + optional bridge
    if args.unit {
        let bridge_tokens = if args.bridge {
            generate_unit_bridge_tokens(
                bevy_type,
                py_type,
                args.bridge_name.as_deref(),
                args.no_reflect,
            )
        } else {
            quote! {}
        };

        let expanded = quote! {
            #input
            #bridge_tokens
        };
        return TokenStream::from(expanded);
    }

    // Generate bridge code if bridge flag is set
    let bridge_tokens = if args.bridge {
        generate_bridge_tokens(
            bevy_type,
            py_type,
            args.bridge_name.as_deref(),
            args.no_insert || (args.no_clone && args.clone_with.is_none()),
            args.no_reflect,
            args.view_fields.as_ref(),
            args.batch_only_fields.as_ref(),
            args.view_only_fields.as_ref(),
            args.materialize.as_ref(),
            args.clone_with.as_ref(),
            true, // emit inventory registration
        )
    } else {
        quote! {}
    };

    let copy_methods = if let Some(materialize) = args.materialize.as_ref() {
        quote! {
            #[pyo3::pymethods]
            impl #py_type {
                pub fn __copy__(&self, py: Python) -> PyResult<Py<PyAny>> {
                    let owned = pybevy_core::ComponentStorage::owned(self.storage.as_ref()?.clone());
                    #materialize(py, owned)
                }

                pub fn __deepcopy__(&self, py: Python, _memo: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
                    let owned = pybevy_core::ComponentStorage::owned(self.storage.as_ref()?.clone());
                    #materialize(py, owned)
                }
            }
        }
    } else {
        quote! {
            #[pyo3::pymethods]
            impl #py_type {
                pub fn __copy__(&self, py: Python) -> PyResult<Py<Self>> {
                    let owned = pybevy_core::ComponentStorage::owned(self.storage.as_ref()?.clone());
                    Py::new(py, (Self { storage: owned, #extra_field_inits }, pybevy_core::PyComponent))
                }

                pub fn __deepcopy__(&self, py: Python, _memo: &Bound<'_, PyAny>) -> PyResult<Py<Self>> {
                    let owned = pybevy_core::ComponentStorage::owned(self.storage.as_ref()?.clone());
                    Py::new(py, (Self { storage: owned, #extra_field_inits }, pybevy_core::PyComponent))
                }
            }
        }
    };

    if args.no_clone {
        // Non-Clone component: only generate borrowed access
        let expanded = quote! {
            #input

            impl #py_type {
                /// Create from a borrowed component storage (for query iteration).
                pub fn from_borrowed(storage: ComponentStorage<#bevy_type>) -> (Self, pybevy_core::PyComponent) {
                    (Self { storage, #extra_field_inits }, pybevy_core::PyComponent)
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
    } else {
        // Clone component: full implementation
        let expanded = quote! {
            #input

            impl From<#bevy_type> for #py_type {
                fn from(component: #bevy_type) -> Self {
                    Self {
                        storage: ComponentStorage::owned(component),
                        #extra_field_inits
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
                        #extra_field_inits
                    })
                }
            }

            impl #py_type {
                /// Create from an owned component value. Returns tuple for PyO3 class inheritance.
                pub fn from_owned(component: #bevy_type) -> (Self, pybevy_core::PyComponent) {
                    (Self { storage: ComponentStorage::owned(component), #extra_field_inits }, pybevy_core::PyComponent)
                }

                /// Create from a borrowed component storage (for query iteration).
                pub fn from_borrowed(storage: ComponentStorage<#bevy_type>) -> (Self, pybevy_core::PyComponent) {
                    (Self { storage, #extra_field_inits }, pybevy_core::PyComponent)
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

            #copy_methods

            #bridge_tokens
        };
        TokenStream::from(expanded)
    }
}

/// Shared bridge code generation used by `#[pycomponent(..., bridge)]`.
// Each argument represents an independent macro capability; grouping them would
// obscure the generated surface without improving call-site safety.
#[allow(clippy::too_many_arguments)]
fn generate_bridge_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    bridge_name_override: Option<&str>,
    no_insert: bool,
    no_reflect: bool,
    view_fields: Option<&Vec<BridgeField>>,
    batch_only_fields: Option<&Vec<BridgeField>>,
    view_only_fields: Option<&Vec<ViewOnlyField>>,
    materialize: Option<&Path>,
    clone_with: Option<&Path>,
    emit_inventory: bool,
) -> proc_macro2::TokenStream {
    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = bridge_name_override.map(String::from).unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let component_name = &bridge_name_str;

    let materialize_storage = if let Some(materialize) = materialize {
        quote! { #materialize(py, storage)? }
    } else {
        quote! { pyo3::Py::new(py, #py_type::from_borrowed(storage))?.into_any() }
    };
    let can_insert = !no_insert;

    // Generate view_bridge method if view_fields (or view_only_fields) is specified
    let has_view_fields = view_fields.is_some() || view_only_fields.is_some();
    let view_bridge_impl = if has_view_fields {
        // Collect all view field names + match arms from view_fields
        let vf_entries: Vec<_> = view_fields
            .iter()
            .flat_map(|fs| fs.iter())
            .map(|f| {
                let name_str = f.python_name.to_string();
                let accessor = &f.rust_accessor;
                let offset = &f.offset_path;
                quote! {
                    #name_str => {
                        let field_type = pybevy_core::field_type_of(&default_val #accessor);
                        Some(pybevy_core::FieldOffset {
                            offset: std::mem::offset_of!(#bevy_type, #offset),
                            field_type,
                        })
                    }
                }
            })
            .collect();

        // Collect view_only_fields entries (use explicit type instead of type inference)
        let vof_entries: Vec<_> = view_only_fields.iter().flat_map(|fs| fs.iter()).map(|f| {
            let name_str = f.python_name.to_string();
            let offset = &f.offset_path;
            let field_type = &f.field_type;
            quote! {
                #name_str => {
                    Some(pybevy_core::FieldOffset {
                        offset: std::mem::offset_of!(#bevy_type, #offset),
                        field_type: <#field_type as pybevy_core::BatchableField>::VIEW_FIELD_TYPE,
                    })
                }
            }
        }).collect();

        // All view field name strings (union of view_fields + view_only_fields)
        let all_view_names: Vec<String> = view_fields
            .iter()
            .flat_map(|fs| fs.iter())
            .map(|f| f.python_name.to_string())
            .chain(
                view_only_fields
                    .iter()
                    .flat_map(|fs| fs.iter())
                    .map(|f| f.python_name.to_string()),
            )
            .collect();
        let all_view_names_slice = &all_view_names;
        let field_names_array = quote! { &[#(#all_view_names_slice),*] };

        // Only emit `default_val` when there are view_fields (which need type inference)
        let default_val_line = if view_fields.is_some_and(|vf| !vf.is_empty()) {
            quote! { let default_val = <#bevy_type>::default(); }
        } else {
            quote! {}
        };

        quote! {
            fn view_bridge(&self) -> Option<pybevy_core::ViewBridge> {
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

                unsafe fn column_data_ptr_fn(
                    column: &bevy::ecs::storage::Column,
                    entity_count: usize,
                ) -> *const u8 {
                    // SAFETY: caller guarantees entity_count == column.len(); #bevy_type matches the column's component type.
                    let data_slice = column.get_data_slice::<#bevy_type>(entity_count);
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
    let insert_impl = if no_insert {
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

            fn prepare_uniform(
                &self,
                _component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<Box<dyn pybevy_core::PreparedUniformComponent>> {
                Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    concat!(#component_name, " cannot be spawned from Python")
                ))
            }
        }
    } else if let Some(clone_with) = clone_with {
        quote! {
            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                world.entity_mut(entity).insert(#clone_with(py_component.storage.as_ref()?.reborrow()));
                Ok(())
            }

            fn insert_into_entity(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                entity.insert(#clone_with(py_component.storage.as_ref()?.reborrow()));
                Ok(())
            }

            fn insert_bulk_uniform(
                &self,
                component: &pyo3::Bound<pyo3::PyAny>,
                entities: &[bevy::ecs::entity::Entity],
                world: &mut bevy::ecs::world::World,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let source = py_component.storage.as_ref()?;
                for &entity_id in entities {
                    world.entity_mut(entity_id).insert(#clone_with(source.reborrow()));
                }
                Ok(())
            }

            fn prepare_uniform(
                &self,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<Box<dyn pybevy_core::PreparedUniformComponent>> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native = #clone_with(py_component.storage.as_ref()?.reborrow());
                Ok(Box::new(pybevy_core::PreparedNativeUniformWith::new(native, #clone_with)))
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

            fn prepare_uniform(
                &self,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<Box<dyn pybevy_core::PreparedUniformComponent>> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.storage.as_ref()?.clone();
                Ok(Box::new(pybevy_core::PreparedNativeUniform::new(native)))
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

            fn can_insert(&self) -> bool {
                #can_insert
            }

            fn register(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<#bevy_type>()
            }

            #[inline(always)]
            fn extract(
                &self,
                entity: &mut pybevy_core::FilteredEntityAccess,
                component_id: bevy::ecs::component::ComponentId,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let storage = if validity.access_mode() == pybevy_core::AccessMode::Write {
                    let ptr = entity.get_mut_ptr_by_id_unchanged(component_id).ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                    })?.cast::<#bevy_type>();
                    unsafe {
                        pybevy_core::ComponentStorage::borrowed(ptr, validity)
                    }
                } else {
                    let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                    })?;
                    let ptr = unsafe {
                        untyped.deref::<#bevy_type>() as *const #bevy_type
                    };
                    unsafe {
                        pybevy_core::ComponentStorage::borrowed_ref(ptr, validity.flag)
                    }
                };

                let obj = #materialize_storage;
                Ok(obj)
            }

            #insert_impl

            fn extract_fn(&self) -> pybevy_core::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut pybevy_core::FilteredEntityAccess,
                    component_id: bevy::ecs::component::ComponentId,
                    validity: pybevy_core::ValidityFlagWithMode,
                    py: pyo3::Python,
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    let storage = if validity.access_mode() == pybevy_core::AccessMode::Write {
                        let ptr = entity.get_mut_ptr_by_id_unchanged(component_id).ok_or_else(|| {
                            pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                        })?.cast::<#bevy_type>();
                        unsafe {
                            pybevy_core::ComponentStorage::borrowed(ptr, validity)
                        }
                    } else {
                        let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                            pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                        })?;
                        let ptr = unsafe {
                            untyped.deref::<#bevy_type>() as *const #bevy_type
                        };
                        unsafe {
                            pybevy_core::ComponentStorage::borrowed_ref(ptr, validity.flag)
                        }
                    };

                    let obj = #materialize_storage;
                    Ok(obj)
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
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                // SAFETY: caller guarantees world_ptr validity (trait contract). The handle
                // re-resolves the component's address per access, so a structural move does
                // not dangle it.
                match unsafe {
                    pybevy_core::resolve_revalidating_component::<#bevy_type>(
                        entity_id, world_ptr, validity,
                    )
                } {
                    Some(storage) => {
                        let obj = #materialize_storage;
                        Ok(Some(obj))
                    }
                    None => Ok(None),
                }
            }

            unsafe fn extract_from_entity_mut(
                &self,
                entity_id: bevy::ecs::entity::Entity,
                world_ptr: *mut bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                // SAFETY: caller guarantees world_ptr validity (trait contract). `validity`
                // carries write access so mutations land on the live component.
                match unsafe {
                    pybevy_core::resolve_revalidating_component::<#bevy_type>(
                        entity_id, world_ptr, validity,
                    )
                } {
                    Some(storage) => {
                        let obj = #materialize_storage;
                        Ok(Some(obj))
                    }
                    None => Ok(None),
                }
            }

            #view_bridge_impl
        }
    };

    // Generate batch-related functions when view_fields or batch_only_fields is present.
    // Batch code uses the union of view_fields + batch_only_fields (NOT view_only_fields).
    let all_batch_fields: Option<Vec<BridgeField>> = match (view_fields, batch_only_fields) {
        (Some(vf), Some(bo)) => {
            let mut all = vf.to_vec();
            all.extend(bo.iter().cloned());
            Some(all)
        }
        (Some(vf), None) => Some(vf.to_vec()),
        (None, Some(bo)) => Some(bo.to_vec()),
        (None, None) => None,
    };
    let batch_impl = if let Some(fields) = &all_batch_fields {
        let field_strs: Vec<String> = fields.iter().map(|f| f.python_name.to_string()).collect();

        // Generate function names using snake_case
        let snake_name = to_snake_case(&bridge_name_str);
        let meta_fn_name = quote::format_ident!("{}_batch_field_meta", snake_name);
        let insert_fn_name = quote::format_ident!("{}_batch_insert", snake_name);
        let prepare_fn_name = quote::format_ident!("{}_batch_prepare", snake_name);
        let register_fn_name = quote::format_ident!("register_{}_batch", snake_name);
        let from_numpy_fn_name = quote::format_ident!("{}_from_numpy", snake_name);

        // Generate field_meta entries using type-inference trick
        let field_meta_entries: Vec<_> = fields
            .iter()
            .map(|field| {
                let accessor = &field.rust_accessor;
                let name_str = field.python_name.to_string();
                let constraints = field.constraints.iter().map(BatchFieldConstraint::tokens);
                quote! {
                    pybevy_core::batch_field_meta_for(&default_val #accessor, #name_str)
                        .with_constraints(&[#(#constraints),*])
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
            let constraints: Vec<_> = field
                .constraints
                .iter()
                .map(BatchFieldConstraint::tokens)
                .collect();
            let value_validation = if constraints.is_empty() {
                quote! {}
            } else {
                quote! {
                    let normalized_array: numpy::PyReadonlyArray2<f32> = normalized.extract()?;
                    let normalized_values = normalized_array.as_slice()?;
                    pybevy_core::batch_columns::validate_f32_values(
                        #name_str,
                        normalized_values,
                        &[#(#constraints),*],
                    )
                    .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                }
            };
            quote! {
                if let Some(arr) = kwargs.get_item(#name_str)? {
                    let np = py.import("numpy")?;
                    // Accept real NumPy, the bounded `pybevy.array` array (via its
                    // `__array__`), and (nested) lists/tuples of numbers.
                    let arr = np.call_method1("asarray", (arr,))?;
                    let arr_bound = &arr;
                    let ndim: usize = arr_bound.getattr("ndim")?.extract()?;
                    let shape: Vec<usize> = arr_bound.getattr("shape")?.extract()?;
                    // Neutral shape validation -> row count (shared error strings).
                    let rows = pybevy_core::batch_columns::plan_column(#name_str, #cols_var, ndim, &shape)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                    // Data work: normalize to a contiguous float32 `(rows, cols)` array.
                    let reshaped = arr_bound.call_method1("reshape", ((rows, #cols_var),))?;
                    let contiguous = np.call_method1("ascontiguousarray", (&reshaped,))?;
                    let normalized = contiguous.call_method1("astype", (np.getattr("float32")?,))?;
                    #value_validation
                    count_agreement.observe(#name_str, rows)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
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

            fn #prepare_fn_name(
                py: pyo3::Python,
                batch: &pybevy_core::PyRustComponentBatch,
            ) -> pyo3::PyResult<Box<dyn pybevy_core::PreparedBatchComponent>> {
                #(#array_extract_stmts)*
                let mut values = Vec::with_capacity(batch.count);
                for i in 0..batch.count {
                    let mut component = <#bevy_type>::default();
                    #(#field_assignments)*
                    values.push(component);
                }
                Ok(Box::new(pybevy_core::PreparedNativeBatch::new(values)))
            }

            fn #insert_fn_name(
                py: pyo3::Python,
                batch: &pybevy_core::PyRustComponentBatch,
                entities: &[bevy::ecs::entity::Entity],
                world: &mut bevy::ecs::world::World,
            ) -> pyo3::PyResult<()> {
                let mut prepared = #prepare_fn_name(py, batch)?;
                let component_id = world.register_component::<#bevy_type>();
                prepared.insert(component_id, entities, world);
                Ok(())
            }

            pub fn #register_fn_name() {
                let meta = Box::leak(Box::new(pybevy_core::ComponentBatchMeta {
                    component_name: #component_name,
                    fields: Box::leak(#meta_fn_name().into_boxed_slice()),
                    insert_fn: #insert_fn_name,
                    prepare_fn: #prepare_fn_name,
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

                // Validate field names (shared error string)
                for key in kwargs.keys() {
                    let key_str: String = key.extract()?;
                    pybevy_core::batch_columns::check_known_field(&key_str, valid_fields)
                        .map_err(|e| pyo3::exceptions::PyValueError::new_err(e.to_string()))?;
                }

                let default_val = <#bevy_type>::default();
                #(#field_validations)*

                let mut field_arrays = std::collections::HashMap::new();
                let mut count_agreement = pybevy_core::batch_columns::CountAgreement::default();

                #(#field_normalize_stmts)*

                // A provided zero-row array is a valid empty batch; only the
                // absence of any field array is an error.
                let Some(count) = count_agreement.count() else {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        pybevy_core::batch_columns::BatchColumnError::NoFields.to_string(),
                    ));
                };

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
    let from_numpy_pymethods = if let (Some(fields), false) = (&all_batch_fields, no_insert) {
        let snake_name = to_snake_case(&bridge_name_str);
        let from_numpy_fn_name = quote::format_ident!("{}_from_numpy", snake_name);
        let parameters = fields.iter().map(|field| {
            let name = &field.python_name;
            quote! { #name: Option<&pyo3::Bound<pyo3::PyAny>> }
        });
        let signature_defaults = fields.iter().map(|field| {
            let name = &field.python_name;
            quote! { #name = None }
        });
        let dictionary_entries = fields.iter().map(|field| {
            let name = &field.python_name;
            let name_string = field.python_name.to_string();
            quote! {
                if let Some(value) = #name {
                    kwargs.set_item(#name_string, value)?;
                }
            }
        });

        quote! {
            #[pyo3::pymethods]
            impl #py_type {
                /// Create a batch of components from numpy arrays for efficient bulk spawning.
                #[staticmethod]
                #[pyo3(signature = (*, #(#signature_defaults),*))]
                pub fn from_numpy(
                    py: pyo3::Python,
                    #(#parameters),*
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    use pyo3::types::PyDictMethods;

                    let kwargs = pyo3::types::PyDict::new(py);
                    #(#dictionary_entries)*
                    #from_numpy_fn_name(py, &kwargs)
                }
            }
        }
    } else {
        quote! {}
    };

    // Add inventory registration (only when called from pycomponent with bridge flag)
    let inventory_submit = if emit_inventory {
        let batch_submit = if all_batch_fields.is_some() {
            let snake_name = to_snake_case(&bridge_name_str);
            let register_fn_name = quote::format_ident!("register_{}_batch", snake_name);
            quote! {
                pybevy_core::inventory::submit!(pybevy_core::BatchRegistration {
                    register: #register_fn_name,
                });
            }
        } else {
            quote! {}
        };

        let reflect_submit = reflect_registration_tokens(bevy_type, no_reflect);

        quote! {
            pybevy_core::inventory::submit!(pybevy_core::ComponentBridgeRegistration {
                create: || std::sync::Arc::new(#bridge_name),
            });
            #reflect_submit
            #batch_submit
        }
    } else {
        quote! {}
    };

    quote! {
        #expanded
        #batch_impl
        #from_numpy_pymethods
        #inventory_submit
    }
}
