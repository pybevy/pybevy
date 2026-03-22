extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::quote;
use syn::{
    Attribute, Fields, GenericArgument, Ident, ItemEnum, ItemStruct, PathArguments, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Resolve the path to the pybevy crate, handling both internal
/// (within pybevy workspace) and external (user crate) usage.
///
/// Returns token streams for `pybevy_core` and `pyo3` paths.
///
/// Note: External crates must also add `pyo3` as a direct dependency
/// because PyO3's proc macros (`#[pyclass]`, `#[pymethods]`) generate
/// code that references `::pyo3::` with absolute paths.
fn pybevy_crate_paths() -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    match crate_name("pybevy") {
        Ok(FoundCrate::Itself) => {
            // We're inside the pybevy crate itself — use direct deps
            (quote! { pybevy_core }, quote! { pyo3 })
        }
        Ok(FoundCrate::Name(name)) => {
            // External crate — use re-exports for pybevy_core,
            // bare pyo3 (must be a direct dependency for PyO3 macros)
            let ident = Ident::new(&name, Span::call_site());
            (quote! { #ident::pybevy_core }, quote! { pyo3 })
        }
        Err(_) => {
            // Fallback: try direct crate names (works in workspace members)
            (quote! { pybevy_core }, quote! { pyo3 })
        }
    }
}

/// A procedural macro to create a pybevy application module from a native Bevy App function.
///
/// # Example
///
/// ```rust
/// use pybevy::pybevy_app;
///
/// #[pybevy_app]
/// pub fn native_app() -> App {
///     let mut app = App::new();
///     app.add_plugins(DefaultPlugins.pythonize()).add_systems(Startup, setup);
///     app
/// }
/// ```
#[proc_macro_attribute]
pub fn pybevy_app(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as syn::ItemFn);
    let fn_name = &input_fn.sig.ident;
    let load_app_name = quote::format_ident!("_py_{}", fn_name);
    let module_name = quote::format_ident!("pybevy_interop");
    let fn_name_str = fn_name.to_string();

    let expanded = quote! {
        use pybevy::pyo3::prelude::*;

        #input_fn

        #[pyfunction(name = #fn_name_str)]
        pub fn #load_app_name() -> PyApp {
            #fn_name().into()
        }

        #[pymodule]
        fn #module_name(m: &Bound<'_, PyModule>) -> PyResult<()> {
            m.add_function(wrap_pyfunction!(#load_app_name, m)?)?;
            pybevy::init_module(m)
        }
    };

    TokenStream::from(expanded)
}

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

/// Arguments for native_asset attribute macro (unchanged)
/// The variant is parsed for validation but the actual type is extracted from the storage field.
#[allow(dead_code)]
struct AssetArgs {
    variant: Ident,
}

impl Parse for AssetArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let variant: Ident = input.parse()?;
        Ok(AssetArgs { variant })
    }
}

/// Extract the inner type from a generic storage type like `AssetStorage<T>` or `ComponentStorage<T>`
fn extract_generic_inner_type<'a>(ty: &'a Type, expected_wrapper: &str) -> Option<&'a Type> {
    if let Type::Path(type_path) = ty {
        let segment = type_path.path.segments.last()?;
        if segment.ident == expected_wrapper
            && let PathArguments::AngleBracketed(args) = &segment.arguments
            && let Some(GenericArgument::Type(inner_type)) = args.args.first()
        {
            return Some(inner_type);
        }
    }
    None
}

/// Find the storage field in a struct and extract the inner type
fn find_storage_field_type<'a>(
    input: &'a ItemStruct,
    expected_wrapper: &str,
) -> Result<&'a Type, syn::Error> {
    match &input.fields {
        Fields::Named(fields) => {
            let storage_field = fields
                .named
                .iter()
                .find(|f| f.ident.as_ref().map(|i| i == "storage").unwrap_or(false));

            match storage_field {
                Some(field) => {
                    extract_generic_inner_type(&field.ty, expected_wrapper).ok_or_else(|| {
                        syn::Error::new_spanned(
                            &field.ty,
                            format!("storage field must be of type {}<T>", expected_wrapper),
                        )
                    })
                }
                None => Err(syn::Error::new_spanned(
                    input,
                    "struct must have a field named 'storage'",
                )),
            }
        }
        _ => Err(syn::Error::new_spanned(
            input,
            "macro requires a struct with named fields",
        )),
    }
}

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
/// ```rust
/// #[native_asset(Shader)]  // PyAssetType variant name
/// #[pyclass(name = "Shader", extends = PyAsset)]
/// #[derive(Debug, Clone)]
/// pub struct PyShader {
///     storage: AssetStorage<Shader>,  // Bevy type is inferred from here
/// }
/// ```
///
/// The macro parses the `storage` field to extract the Bevy asset type (`Shader` in this case).
#[proc_macro_attribute]
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
}

/// Generates boilerplate implementations for PyBevy resource wrapper types.
///
/// This macro generates:
/// - `impl Clone for PyType`
/// - `impl From<BevyType> for PyType`
/// - Helper methods: `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage
///
/// ```rust
/// #[native_resource]
/// #[pyclass(name = "Time", extends = PyResource)]
/// pub struct PyTime {
///     pub(crate) storage: ResourceStorage<Time>,
/// }
/// ```
///
/// For generic Bevy types (e.g., `Time<Fixed>`), specify the full type:
///
/// ```rust
/// #[native_resource(Time<Fixed>)]
/// #[pyclass(name = "TimeFixed", extends = PyResource)]
/// pub struct PyTimeFixed {
///     pub(crate) storage: ResourceStorage<Time<Fixed>>,
/// }
/// ```
#[proc_macro_attribute]
pub fn native_resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    // Find the storage field and extract the Bevy type
    let bevy_type: Type = if attr.is_empty() {
        // Infer from storage field
        match find_storage_field_type(&input, "ResourceStorage") {
            Ok(ty) => ty.clone(),
            Err(e) => return e.to_compile_error().into(),
        }
    } else {
        // Parse explicit type from attribute (for generics like Time<Fixed>)
        parse_macro_input!(attr as Type)
    };

    let expanded = quote! {
        #input

        impl Clone for #py_type {
            fn clone(&self) -> Self {
                #py_type {
                    storage: self.storage.clone(),
                }
            }
        }

        impl From<#bevy_type> for #py_type {
            fn from(resource: #bevy_type) -> Self {
                #py_type {
                    storage: ResourceStorage::owned(resource),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(value: #py_type) -> PyResult<Self> {
                Ok(value.storage.into_owned()?)
            }
        }

        impl #py_type {
            /// Create from a borrowed resource storage (for Res/ResMut access).
            pub(crate) fn from_borrowed(storage: ResourceStorage<#bevy_type>) -> (Self, PyResource) {
                (Self { storage }, PyResource)
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

/// Generates boilerplate implementations for PyBevy field wrapper types.
///
/// This macro generates:
/// - `impl Clone for PyType`
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - `impl TryFrom<&PyType> for BevyType`
/// - `impl FromBorrowedStorage<FieldStorage<BevyType>> for PyType`
/// - Helper methods: `from_owned()`, `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage
///
/// ```rust
/// #[native_field]  // Bevy type inferred from storage field
/// #[pyclass(name = "BloomPrefilter")]
/// pub struct PyBloomPrefilter {
///     storage: FieldStorage<BloomPrefilter>,
/// }
/// ```
///
/// For explicit type specification (e.g., complex generics):
///
/// ```rust
/// #[native_field(SomeComplexType<T>)]
/// #[pyclass(name = "MyType")]
/// pub struct PyMyType {
///     storage: FieldStorage<SomeComplexType<T>>,
/// }
/// ```
#[proc_macro_attribute]
pub fn native_field(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    // Find the storage field and extract the Bevy type
    let bevy_type: Type = if attr.is_empty() {
        // Infer from storage field
        match find_storage_field_type(&input, "FieldStorage") {
            Ok(ty) => ty.clone(),
            Err(e) => return e.to_compile_error().into(),
        }
    } else {
        // Parse explicit type from attribute
        parse_macro_input!(attr as Type)
    };

    let expanded = quote! {
        #input

        impl Clone for #py_type {
            fn clone(&self) -> Self {
                Self {
                    storage: self.storage.clone(),
                }
            }
        }

        impl From<#bevy_type> for #py_type {
            fn from(value: #bevy_type) -> Self {
                Self {
                    storage: FieldStorage::owned(value),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(value: #py_type) -> PyResult<Self> {
                Ok(value.storage.get()?)
            }
        }

        impl TryFrom<&#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(value: &#py_type) -> PyResult<Self> {
                Ok(value.storage.get()?)
            }
        }

        impl FromBorrowedStorage<FieldStorage<#bevy_type>> for #py_type {
            fn from_borrowed(storage: FieldStorage<#bevy_type>) -> Self {
                Self { storage }
            }
        }

        impl #py_type {
            /// Create from an owned value
            pub(crate) fn from_owned(value: #bevy_type) -> Self {
                Self {
                    storage: FieldStorage::owned(value),
                }
            }

            /// Create from a borrowed field storage
            pub(crate) fn from_borrowed(storage: FieldStorage<#bevy_type>) -> Self {
                Self { storage }
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
#[proc_macro_attribute]
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
#[proc_macro]
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
#[proc_macro_attribute]
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

/// Arguments for bevy_enum attribute macro
struct BevyEnumArgs {
    /// The Bevy enum type to convert to/from
    bevy_type: Type,
    /// If true, only generate From impls (no #[pymethods])
    from_only: bool,
    /// If true, map empty tuple variants Variant() to Bevy's unit variants Variant
    empty_tuple: bool,
}

impl Parse for BevyEnumArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let bevy_type: Type = input.parse()?;

        let mut from_only = false;
        let mut empty_tuple = false;
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            let option: Ident = input.parse()?;
            match option.to_string().as_str() {
                "from_only" => from_only = true,
                "empty_tuple" => empty_tuple = true,
                other => {
                    return Err(syn::Error::new_spanned(
                        option,
                        format!(
                            "unknown option '{}', expected: from_only, empty_tuple",
                            other
                        ),
                    ));
                }
            }
        }

        Ok(BevyEnumArgs {
            bevy_type,
            from_only,
            empty_tuple,
        })
    }
}

/// Generates boilerplate implementations for PyBevy enum wrapper types.
///
/// This macro generates:
/// - `impl From<BevyType> for PyType` (by matching variant names)
/// - `impl From<PyType> for BevyType` (by matching variant names)
/// - `#[pymethods]` with `__repr__` (unless `from_only` is specified)
///
/// **Important**: This macro only works for enums with unit variants (no data).
/// For enums with tuple/struct variants, implement conversions manually.
///
/// # Usage
///
/// ```rust
/// // Full generation (From impls + __repr__)
/// #[bevy_enum(BevyCursorGrabMode)]
/// #[pyclass(name = "CursorGrabMode", eq)]
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// pub enum PyCursorGrabMode {
///     None,
///     Confined,
///     Locked,
/// }
///
/// // From impls only (when you have custom #[pymethods])
/// #[bevy_enum(BevyTimerMode, from_only)]
/// #[pyclass(name = "TimerMode")]
/// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// pub enum PyTimerMode {
///     Once,
///     Repeating,
/// }
/// ```
///
/// The macro assumes variant names match between the Py and Bevy types.
/// Extract the pyo3 name from variant attributes if present
/// Looks for #[pyo3(name = "...")] attribute
fn get_pyo3_variant_name(variant: &syn::Variant) -> Option<String> {
    for attr in &variant.attrs {
        if attr.path().is_ident("pyo3")
            && let syn::Meta::List(meta_list) = &attr.meta
        {
            let tokens = meta_list.tokens.to_string();
            // Parse "name = \"SomeName\""
            if let Some(start) = tokens.find("name") {
                let rest = &tokens[start..];
                if let Some(eq_pos) = rest.find('=') {
                    let after_eq = rest[eq_pos + 1..].trim();
                    // Extract string between quotes
                    if let Some(first_quote) = after_eq.find('"') {
                        let after_first = &after_eq[first_quote + 1..];
                        if let Some(second_quote) = after_first.find('"') {
                            return Some(after_first[..second_quote].to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Variant kind for bevy_enum processing
enum VariantKind {
    /// Unit variant: `Variant`
    Unit,
    /// Empty tuple variant: `Variant()` - maps to Bevy's unit variant
    EmptyTuple,
    /// Single-field tuple variant: `Variant(T)` - passes through the value
    DataTuple,
}

#[proc_macro_attribute]
pub fn bevy_enum(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as BevyEnumArgs);
    let input = parse_macro_input!(item as ItemEnum);

    let py_type = &input.ident;
    let bevy_type = &args.bevy_type;

    // Collect variant info (name, optional pyo3 rename, and variant kind)
    struct VariantInfo<'a> {
        ident: &'a Ident,
        repr_name: String,
        kind: VariantKind,
    }

    let mut variants: Vec<VariantInfo> = Vec::new();
    for variant in &input.variants {
        let repr_name = get_pyo3_variant_name(variant).unwrap_or_else(|| variant.ident.to_string());

        let kind = match &variant.fields {
            syn::Fields::Unit => Some(VariantKind::Unit),
            syn::Fields::Unnamed(f) if f.unnamed.is_empty() && args.empty_tuple => {
                Some(VariantKind::EmptyTuple)
            }
            syn::Fields::Unnamed(f) if f.unnamed.len() == 1 => {
                // Auto-detect single-field tuple variants like Other(u16)
                Some(VariantKind::DataTuple)
            }
            _ => None,
        };

        match kind {
            Some(k) => {
                variants.push(VariantInfo {
                    ident: &variant.ident,
                    repr_name,
                    kind: k,
                });
            }
            None => {
                let msg = if args.empty_tuple {
                    "bevy_enum only supports unit, empty tuple, and single-field tuple variants"
                } else {
                    "bevy_enum only supports unit and single-field tuple variants (use empty_tuple for Variant() style)"
                };
                return syn::Error::new_spanned(variant, msg)
                    .to_compile_error()
                    .into();
            }
        }
    }

    // Generate From<BevyType> for PyType
    let from_bevy_arms = variants.iter().map(|v| {
        let name = v.ident;
        match v.kind {
            VariantKind::Unit => quote! { #bevy_type::#name => #py_type::#name },
            VariantKind::EmptyTuple => quote! { #bevy_type::#name => #py_type::#name() },
            VariantKind::DataTuple => quote! { #bevy_type::#name(v) => #py_type::#name(v) },
        }
    });

    // Generate From<PyType> for BevyType
    let from_py_arms = variants.iter().map(|v| {
        let name = v.ident;
        match v.kind {
            VariantKind::Unit => quote! { #py_type::#name => #bevy_type::#name },
            VariantKind::EmptyTuple => quote! { #py_type::#name() => #bevy_type::#name },
            VariantKind::DataTuple => quote! { #py_type::#name(v) => #bevy_type::#name(v) },
        }
    });

    // Generate pymethods block if not from_only
    let pymethods_block = if args.from_only {
        quote! {}
    } else {
        // Extract the type name for __repr__ (strip "Py" prefix if present)
        let type_name = py_type.to_string();
        let type_repr_name = type_name.strip_prefix("Py").unwrap_or(&type_name);

        // Generate __repr__ match arms (using pyo3 name if present)
        let repr_arms = variants.iter().map(|v| {
            let ident = v.ident;
            match v.kind {
                VariantKind::Unit => {
                    let repr_str = format!("{}.{}", type_repr_name, v.repr_name);
                    quote! { #py_type::#ident => #repr_str.to_string() }
                }
                VariantKind::EmptyTuple => {
                    let repr_str = format!("{}.{}", type_repr_name, v.repr_name);
                    quote! { #py_type::#ident() => #repr_str.to_string() }
                }
                VariantKind::DataTuple => {
                    let repr_prefix = format!("{}.{}(", type_repr_name, v.repr_name);
                    quote! { #py_type::#ident(v) => format!("{}{})", #repr_prefix, v) }
                }
            }
        });

        quote! {
            #[pymethods]
            impl #py_type {
                pub fn __repr__(&self) -> String {
                    match self {
                        #(#repr_arms,)*
                    }
                }
            }
        }
    };

    let expanded = quote! {
        #input

        impl From<#bevy_type> for #py_type {
            fn from(value: #bevy_type) -> Self {
                match value {
                    #(#from_bevy_arms,)*
                }
            }
        }

        impl From<#py_type> for #bevy_type {
            fn from(value: #py_type) -> Self {
                match value {
                    #(#from_py_arms,)*
                }
            }
        }

        #pymethods_block
    };

    TokenStream::from(expanded)
}

/// Generates a PluginBridge struct and implementation for feature crates.
///
/// This macro generates:
/// - A bridge struct (e.g., `TransformPluginBridge`)
/// - `impl PluginBridge for XBridge` with all required methods
///
/// # Usage
///
/// Simple plugin (uses BevyType::default()):
/// ```rust
/// // In pybevy_transform/src/lib.rs
/// plugin_bridge!(PyTransformPlugin, TransformPlugin);
/// ```
///
/// Plugin with custom build logic:
/// ```rust
/// plugin_bridge!(PyAudioPlugin, AudioPlugin, |py_plugin: &Bound<'_, PyAudioPlugin>, app: &mut App| {
///     let config = py_plugin.extract::<PyRef<PyAudioPlugin>>()?;
///     // Use config to customize the Bevy plugin
///     app.add_plugins(bevy::audio::AudioPlugin::default());
///     Ok(())
/// });
/// ```
///
/// The bridge name is derived from the Bevy type (e.g., `TransformPluginBridge`).
#[proc_macro]
pub fn plugin_bridge(input: TokenStream) -> TokenStream {
    struct PluginBridgeArgs {
        py_type: Ident,
        bevy_type: Type,
        custom_build: Option<syn::ExprClosure>,
    }

    impl Parse for PluginBridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let py_type: Ident = input.parse()?;
            input.parse::<Token![,]>()?;
            let bevy_type: Type = input.parse()?;

            let custom_build = if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                Some(input.parse()?)
            } else {
                None
            };

            Ok(PluginBridgeArgs {
                py_type,
                bevy_type,
                custom_build,
            })
        }
    }

    let args = parse_macro_input!(input as PluginBridgeArgs);
    let py_type = &args.py_type;
    let bevy_type = &args.bevy_type;

    // Extract simple name for bridge struct (strip generic args if present)
    let bevy_type_name = match &args.bevy_type {
        Type::Path(type_path) => type_path
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_else(|| "Plugin".to_string()),
        _ => "Plugin".to_string(),
    };

    let bridge_name = quote::format_ident!("{}Bridge", bevy_type_name);
    let plugin_name = &bevy_type_name;

    // Generate build method based on whether custom logic is provided
    let build_impl = match &args.custom_build {
        Some(closure) => {
            quote! {
                fn build(&self, py_plugin: &pyo3::Bound<'_, pyo3::PyAny>, app: &mut bevy::app::App) -> pyo3::PyResult<()> {
                    let build_fn: fn(&pyo3::Bound<'_, pyo3::PyAny>, &mut bevy::app::App) -> pyo3::PyResult<()> = #closure;
                    build_fn(py_plugin, app)
                }
            }
        }
        None => {
            quote! {
                fn build(&self, _py_plugin: &pyo3::Bound<'_, pyo3::PyAny>, app: &mut bevy::app::App) -> pyo3::PyResult<()> {
                    app.add_plugins(#bevy_type::default());
                    Ok(())
                }
            }
        }
    };

    let expanded = quote! {
        /// Bridge for #plugin_name plugin
        pub struct #bridge_name;

        impl pybevy_core::PluginBridge for #bridge_name {
            fn py_type_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<#py_type>()
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
                #plugin_name
            }

            #build_impl
        }
    };

    TokenStream::from(expanded)
}

/// Generates storage boilerplate for resource wrappers in feature crates.
///
/// Unlike `native_resource`, this macro does NOT generate PyResourceType enum handling,
/// making it usable in feature crates that don't have access to `PyResourceType`.
///
/// This macro generates:
/// - `impl Clone for PyType`
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - Helper methods: `from_owned()`, `from_borrowed()`, `as_ref()`, `as_mut()`
///
/// # Usage in feature crates (e.g., pybevy_audio)
///
/// ```rust
/// #[resource_storage(GlobalVolume)]
/// #[pyclass(name = "GlobalVolume", extends = PyResource)]
/// pub struct PyGlobalVolume {
///     pub(crate) storage: ResourceStorage<GlobalVolume>,
/// }
/// ```
///
/// The main crate then uses `resource_bridge!` to add runtime dispatch.
#[proc_macro_attribute]
pub fn resource_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    let bevy_type: Type = parse_macro_input!(attr as Type);
    let input = parse_macro_input!(item as ItemStruct);

    let py_type = &input.ident;

    let expanded = quote! {
        #input

        impl Clone for #py_type {
            fn clone(&self) -> Self {
                Self {
                    storage: self.storage.clone(),
                }
            }
        }

        impl From<#bevy_type> for #py_type {
            fn from(resource: #bevy_type) -> Self {
                Self {
                    storage: pybevy_core::ResourceStorage::owned(resource),
                }
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(py_resource: #py_type) -> PyResult<Self> {
                Ok(py_resource.storage.into_owned()?)
            }
        }

        impl #py_type {
            /// Create from an owned resource value. Returns tuple for PyO3 class inheritance.
            pub fn from_owned(resource: #bevy_type) -> (Self, pybevy_core::PyResource) {
                (Self { storage: pybevy_core::ResourceStorage::owned(resource) }, pybevy_core::PyResource)
            }

            /// Create from a borrowed resource storage (for Res/ResMut access).
            pub fn from_borrowed(storage: pybevy_core::ResourceStorage<#bevy_type>) -> (Self, pybevy_core::PyResource) {
                (Self { storage }, pybevy_core::PyResource)
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
}

/// Generates a ComponentBridge struct for unit/marker components in feature crates.
///
/// Unlike `component_bridge!`, this macro handles unit components that have no data/storage.
/// It generates a simpler bridge that just inserts/extracts the marker component.
///
/// # Usage
///
/// ```rust
/// // In pybevy_light/src/lib.rs
/// unit_bridge!(NotShadowCaster, PyNotShadowCaster);
/// ```
///
/// This generates `NotShadowCasterBridge` struct with `ComponentBridge` impl.
#[proc_macro]
pub fn unit_bridge(input: TokenStream) -> TokenStream {
    struct BridgeArgs {
        bevy_type: Type,
        py_type: Ident,
        bridge_name: Option<String>,
    }

    impl Parse for BridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            input.parse::<Token![,]>()?;
            let py_type: Ident = input.parse()?;

            let bridge_name = if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                Some(input.parse::<syn::LitStr>()?.value())
            } else {
                None
            };

            Ok(BridgeArgs {
                bevy_type,
                py_type,
                bridge_name,
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
                entity: &mut bevy::ecs::world::FilteredEntityMut,
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
                    entity: &mut bevy::ecs::world::FilteredEntityMut,
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

    TokenStream::from(expanded)
}

/// Generates a ResourceBridge struct and implementation for feature crates.
///
/// This macro generates:
/// - A bridge struct (e.g., `GlobalVolumeBridge`)
/// - `impl ResourceBridge for XBridge` with all required methods
///
/// # Usage
///
/// ```rust
/// // In pybevy_audio/src/lib.rs
/// resource_bridge!(GlobalVolume, PyGlobalVolume);
/// ```
///
/// This generates `GlobalVolumeBridge` struct with full `ResourceBridge` impl.
///
/// For resources where the Bevy type name differs from the bridge prefix:
/// ```rust
/// resource_bridge!(Time<Fixed>, PyTimeFixed, "TimeFixed");
/// ```
///
/// For read-only resources that cannot be inserted from Python:
/// ```rust
/// resource_bridge!(SystemResource, PySystemResource, no_insert);
/// ```
///
/// For resources where get_mut returns read-only access:
/// ```rust
/// resource_bridge!(ButtonInput<KeyCode>, PyButtonInput, "ButtonInput", no_mut, default_insert);
/// ```
///
/// For system-managed resources (no insert, no remove):
/// ```rust
/// resource_bridge!(SceneSpawner, PySceneSpawner, no_insert, no_remove);
/// ```
#[proc_macro]
pub fn resource_bridge(input: TokenStream) -> TokenStream {
    struct BridgeArgs {
        bevy_type: Type,
        py_type: Ident,
        bridge_name: Option<String>,
        no_insert: bool,
        no_mut: bool,
        no_remove: bool,
        default_insert: bool,
    }

    impl Parse for BridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            input.parse::<Token![,]>()?;
            let py_type: Ident = input.parse()?;

            let mut bridge_name = None;
            let mut no_insert = false;
            let mut no_mut = false;
            let mut no_remove = false;
            let mut default_insert = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                } else if input.peek(syn::Ident) {
                    let ident: Ident = input.parse()?;
                    if ident == "no_insert" {
                        no_insert = true;
                    } else if ident == "no_mut" {
                        no_mut = true;
                    } else if ident == "no_remove" {
                        no_remove = true;
                    } else if ident == "default_insert" {
                        default_insert = true;
                    } else {
                        return Err(syn::Error::new_spanned(
                            ident,
                            "expected 'no_insert', 'no_mut', 'no_remove', 'default_insert', or a string literal for bridge name",
                        ));
                    }
                }
            }

            Ok(BridgeArgs {
                bevy_type,
                py_type,
                bridge_name,
                no_insert,
                no_mut,
                no_remove,
                default_insert,
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
    let resource_name = &bridge_name_str;

    // Generate insert method based on flags
    let insert_impl = if args.no_insert {
        quote! {
            fn insert(
                &self,
                _world: &mut bevy::ecs::world::World,
                _resource: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    concat!(#resource_name, " cannot be inserted from Python")
                ))
            }
        }
    } else if args.default_insert {
        quote! {
            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                _resource: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                world.insert_resource(<#bevy_type>::default());
                Ok(())
            }
        }
    } else {
        quote! {
            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                resource: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_resource: #bevy_type = resource.extract::<#py_type>()?.try_into()?;
                world.insert_resource(py_resource);
                Ok(())
            }
        }
    };

    // Generate get_mut method based on no_mut flag
    let get_mut_impl = if args.no_mut {
        // Read-only: get_mut returns read-only access (same as get)
        quote! {
            fn get_mut(
                &self,
                world: &mut bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let resource = world.get_resource::<#bevy_type>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?;

                let ptr = resource as *const #bevy_type as *mut #bevy_type;
                // Override to read mode even though caller requested write
                let read_validity = validity.flag.with_access_mode(pybevy_core::AccessMode::Read);
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed(ptr, read_validity)
                };

                let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                Ok(obj.into_any())
            }
        }
    } else {
        quote! {
            fn get_mut(
                &self,
                world: &mut bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let resource = world.get_resource_mut::<#bevy_type>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?;

                let ptr = resource.into_inner() as *mut #bevy_type;
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed(ptr, validity)
                };

                let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                Ok(obj.into_any())
            }
        }
    };

    // Generate remove method based on no_remove flag
    let remove_impl = if args.no_remove {
        quote! {
            fn remove(&self, _world: &mut bevy::ecs::world::World) {
                // System-managed resource, removal not supported
            }
        }
    } else {
        quote! {
            fn remove(&self, world: &mut bevy::ecs::world::World) {
                world.remove_resource::<#bevy_type>();
            }
        }
    };

    let expanded = quote! {
        /// Bridge for #resource_name resource
        pub struct #bridge_name;

        impl pybevy_core::ResourceBridge for #bridge_name {
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
                #resource_name
            }

            fn get(
                &self,
                world: &bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let resource = world.get_resource::<#bevy_type>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#resource_name, " resource not found"))
                })?;

                let ptr = resource as *const #bevy_type as *mut #bevy_type;
                let storage = unsafe {
                    pybevy_core::ResourceStorage::borrowed(ptr, validity)
                };

                let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                Ok(obj.into_any())
            }

            #get_mut_impl

            #insert_impl

            #remove_impl

            fn contains_in_world(&self, world: &bevy::ecs::world::World) -> bool {
                world.contains_resource::<#bevy_type>()
            }

            fn resource_id(&self, world: &bevy::ecs::world::World) -> Option<bevy::ecs::component::ComponentId> {
                world.components().resource_id::<#bevy_type>()
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
/// ```rust
/// #[asset_storage(AudioSource)]
/// #[pyclass(name = "AudioSource", extends = PyAsset)]
/// pub struct PyAudioSource {
///     pub(crate) storage: AssetStorage<AudioSource>,
/// }
/// ```
///
/// For assets where Clone is not implemented, use `no_clone`:
///
/// ```rust
/// #[asset_storage(SomeAsset, no_clone)]
/// #[pyclass(name = "SomeAsset", extends = PyAsset)]
/// pub struct PySomeAsset {
///     pub(crate) storage: AssetStorage<SomeAsset>,
/// }
/// ```
///
/// The main crate then uses `asset_bridge!` to add runtime dispatch.
#[proc_macro_attribute]
pub fn asset_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct AssetStorageArgs {
        bevy_type: Type,
        no_clone: bool,
    }

    impl Parse for AssetStorageArgs {
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
            Ok(AssetStorageArgs {
                bevy_type,
                no_clone,
            })
        }
    }

    let args = parse_macro_input!(attr as AssetStorageArgs);
    let bevy_type = &args.bevy_type;
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

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

    let expanded = quote! {
        #input

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
            /// Create from an owned asset value. Returns tuple for PyO3 class inheritance.
            pub fn from_owned(asset: #bevy_type) -> (Self, pybevy_core::PyAsset) {
                (Self { storage: pybevy_core::AssetStorage::owned(asset) }, pybevy_core::PyAsset)
            }

            /// Create from a borrowed asset storage (for asset iteration).
            pub fn from_borrowed(storage: pybevy_core::AssetStorage<#bevy_type>) -> (Self, pybevy_core::PyAsset) {
                (Self { storage }, pybevy_core::PyAsset)
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
}

/// Generates storage boilerplate for newtype component wrappers in feature crates.
///
/// Unlike `native_component(X, newtype)`, this macro does NOT generate `NativeComponent` impl,
/// making it usable in feature crates that don't have access to `PyComponentType`.
///
/// This macro generates:
/// - `impl From<BevyType> for PyType`
/// - `impl TryFrom<PyType> for BevyType`
/// - `impl TryFrom<&BevyType> for PyType`
/// - Helper method: `from_owned()`
///
/// # Usage in feature crates
///
/// ```rust
/// #[newtype_storage(Tonemapping)]
/// #[pyclass(name = "Tonemapping", extends = PyComponent, frozen)]
/// #[derive(Clone, Debug)]
/// pub struct PyTonemapping(pub(crate) Tonemapping);
/// ```
///
/// The main crate then uses `newtype_bridge!` to add runtime dispatch.
#[proc_macro_attribute]
pub fn newtype_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    let bevy_type: Type = parse_macro_input!(attr as Type);
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    let expanded = quote! {
        #input

        impl From<#bevy_type> for #py_type {
            fn from(value: #bevy_type) -> Self {
                Self(value)
            }
        }

        impl TryFrom<#py_type> for #bevy_type {
            type Error = PyErr;

            fn try_from(value: #py_type) -> PyResult<Self> {
                Ok(value.0)
            }
        }

        impl TryFrom<&#bevy_type> for #py_type {
            type Error = PyErr;

            fn try_from(value: &#bevy_type) -> PyResult<Self> {
                Ok(Self(value.clone()))
            }
        }

        impl #py_type {
            /// Create from an owned component value. Returns tuple for PyO3 class inheritance.
            pub fn from_owned(value: #bevy_type) -> (Self, pybevy_core::PyComponent) {
                (Self(value), pybevy_core::PyComponent)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generates a ComponentBridge struct for newtype/wrapper components in feature crates.
///
/// Unlike `component_bridge!`, this macro handles newtype components that wrap a Bevy type
/// directly (like enums or simple structs) without using ComponentStorage.
///
/// # Usage
///
/// ```rust
/// // In pybevy_camera/src/lib.rs
/// newtype_bridge!(Tonemapping, PyTonemapping);
/// ```
///
/// This generates `TonemappingBridge` struct with `ComponentBridge` impl.
///
/// For types where Clone is Copy (more efficient):
/// ```rust
/// newtype_bridge!(Msaa, PyMsaa, copy);
/// ```
#[proc_macro]
pub fn newtype_bridge(input: TokenStream) -> TokenStream {
    struct BridgeArgs {
        bevy_type: Type,
        py_type: Ident,
        bridge_name: Option<String>,
        is_copy: bool,
    }

    impl Parse for BridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            input.parse::<Token![,]>()?;
            let py_type: Ident = input.parse()?;

            let mut bridge_name = None;
            let mut is_copy = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                } else if input.peek(syn::Ident) {
                    let ident: Ident = input.parse()?;
                    if ident == "copy" {
                        is_copy = true;
                    } else {
                        return Err(syn::Error::new_spanned(
                            ident,
                            "expected 'copy' or a string literal for bridge name",
                        ));
                    }
                }
            }

            Ok(BridgeArgs {
                bevy_type,
                py_type,
                bridge_name,
                is_copy,
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

    // Clone vs Copy for extraction
    let clone_expr = if args.is_copy {
        quote! { *component }
    } else {
        quote! { component.clone() }
    };

    let expanded = quote! {
        /// Bridge for #component_name newtype component
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
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                })?;

                let component = unsafe {
                    untyped.deref::<#bevy_type>()
                };

                // Newtype: clone/copy the value (no borrowed storage for simple wrappers)
                let obj = pyo3::Py::new(py, #py_type::from_owned(#clone_expr))?;
                Ok(obj.into_any())
            }

            fn insert(
                &self,
                world: &mut bevy::ecs::world::World,
                entity: bevy::ecs::entity::Entity,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.0.clone();

                world.entity_mut(entity).insert(native);
                Ok(())
            }

            fn insert_into_entity(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                component: &pyo3::Bound<pyo3::PyAny>,
            ) -> pyo3::PyResult<()> {
                let py_component = component.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = py_component.0.clone();

                entity.insert(native);
                Ok(())
            }

            fn extract_fn(&self) -> pybevy_core::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut bevy::ecs::world::FilteredEntityMut,
                    component_id: bevy::ecs::component::ComponentId,
                    _validity: pybevy_core::ValidityFlagWithMode,
                    py: pyo3::Python,
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err("Component not found")
                    })?;

                    let component = unsafe {
                        untyped.deref::<#bevy_type>()
                    };

                    let obj = pyo3::Py::new(py, #py_type::from_owned(#clone_expr))?;
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
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                if let Some(component) = entity.get::<#bevy_type>() {
                    let obj = pyo3::Py::new(py, #py_type::from_owned(#clone_expr))?;
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
                if let Some(component) = entity.get::<#bevy_type>() {
                    let obj = pyo3::Py::new(py, #py_type::from_owned(#clone_expr))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generates an AssetBridge struct and implementation for feature crates.
///
/// This macro generates:
/// - A bridge struct (e.g., `AudioSourceBridge`)
/// - `impl AssetBridge for XBridge` with all required methods
///
/// # Usage
///
/// ```rust
/// // In pybevy_audio/src/lib.rs
/// asset_bridge!(AudioSource, PyAudioSource);
/// ```
///
/// This generates `AudioSourceBridge` struct with full `AssetBridge` impl.
///
/// For assets where the Bevy type name differs from the bridge prefix:
/// ```rust
/// asset_bridge!(bevy::audio::AudioSource, PyAudioSource, "AudioSource");
/// ```
///
/// For assets that cannot be loaded from files (only created programmatically):
/// ```rust
/// asset_bridge!(TextureAtlasLayout, PyTextureAtlasLayout, not_loadable);
/// ```
#[proc_macro]
pub fn asset_bridge(input: TokenStream) -> TokenStream {
    struct BridgeArgs {
        bevy_type: Type,
        py_type: Ident,
        bridge_name: Option<String>,
        not_loadable: bool,
    }

    impl Parse for BridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            input.parse::<Token![,]>()?;
            let py_type: Ident = input.parse()?;

            let mut bridge_name = None;
            let mut not_loadable = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if input.peek(syn::Ident) {
                    let ident: syn::Ident = input.parse()?;
                    if ident == "not_loadable" {
                        not_loadable = true;
                    } else {
                        return Err(syn::Error::new(ident.span(), "expected `not_loadable`"));
                    }
                } else if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                }
            }

            Ok(BridgeArgs {
                bevy_type,
                py_type,
                bridge_name,
                not_loadable,
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
    let asset_name = &bridge_name_str;

    let not_loadable = args.not_loadable;

    let is_loadable_impl = if not_loadable {
        quote! {
            fn is_loadable(&self) -> bool {
                false
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

            #is_loadable_impl

            fn get(
                &self,
                world: &bevy::ecs::world::World,
                handle: &bevy::asset::UntypedHandle,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                use bevy::asset::Assets;

                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                match assets.get(&typed_handle) {
                    Some(asset) => {
                        let ptr = asset as *const #bevy_type;
                        let storage = unsafe {
                            pybevy_core::AssetStorage::borrowed_readonly(
                                ptr,
                                validity,
                                handle.clone(),
                            )
                        };
                        let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                        Ok(Some(obj.into_any()))
                    }
                    None => Ok(None),
                }
            }

            fn get_mut(
                &self,
                world: &mut bevy::ecs::world::World,
                handle: &bevy::asset::UntypedHandle,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                use bevy::asset::Assets;

                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                match assets.get_mut(&typed_handle) {
                    Some(asset) => {
                        let ptr = asset as *mut #bevy_type;
                        let storage = unsafe {
                            pybevy_core::AssetStorage::borrowed_mut(
                                ptr,
                                validity,
                                handle.clone(),
                            )
                        };
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

                let mut py_asset = asset.extract::<pyo3::PyRefMut<#py_type>>()?;
                let native_asset = py_asset.take()?;

                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let handle = assets.add(native_asset);
                Ok(handle.untyped())
            }

            fn remove(
                &self,
                world: &mut bevy::ecs::world::World,
                handle: &bevy::asset::UntypedHandle,
            ) -> pyo3::PyResult<bool> {
                use bevy::asset::Assets;

                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                Ok(assets.remove(&typed_handle).is_some())
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
                handle: &bevy::asset::UntypedHandle,
            ) -> pyo3::PyResult<bool> {
                use bevy::asset::Assets;

                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                Ok(assets.contains(&typed_handle))
            }

            fn iter_pairs(
                &self,
                world: &bevy::ecs::world::World,
                validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Vec<(bevy::asset::UntypedHandle, pyo3::Py<pyo3::PyAny>)>> {
                use bevy::asset::{Assets, AssetId};

                let assets = world.get_resource::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let type_id = std::any::TypeId::of::<#bevy_type>();
                let mut result = Vec::new();
                for (id, asset) in assets.iter() {
                    let untyped_handle = match id {
                        AssetId::Uuid { uuid } => bevy::asset::UntypedHandle::Uuid { uuid, type_id },
                        AssetId::Index { index, .. } => {
                            let index_bits = index.to_bits();
                            let synthetic_uuid = pybevy_core::uuid::Uuid::from_u128(
                                0xDEAD_BEEF_0000_0000_u128 << 64 | index_bits as u128
                            );
                            bevy::asset::UntypedHandle::Uuid {
                                uuid: synthetic_uuid,
                                type_id,
                            }
                        }
                    };
                    let ptr = asset as *const #bevy_type;
                    let storage = unsafe {
                        pybevy_core::AssetStorage::borrowed_readonly(
                            ptr,
                            validity.clone(),
                            untyped_handle.clone(),
                        )
                    };
                    let obj = pyo3::Py::new(py, #py_type::from_borrowed(storage))?;
                    result.push((untyped_handle, obj.into_any()));
                }
                Ok(result)
            }

            fn remove_and_return(
                &self,
                world: &mut bevy::ecs::world::World,
                handle: &bevy::asset::UntypedHandle,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                use bevy::asset::Assets;

                let mut assets = world.get_resource_mut::<Assets<#bevy_type>>().ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(
                        concat!("Assets<", #asset_name, "> resource not found")
                    )
                })?;

                let typed_handle = handle.clone().typed::<#bevy_type>();
                match assets.remove(&typed_handle) {
                    Some(asset) => {
                        let py_asset = #py_type::from(asset);
                        let obj = pyo3::Py::new(py, (py_asset, pybevy_core::PyAsset))?;
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
        }
    };

    TokenStream::from(expanded)
}

/// Generates a ComponentBridge for handle wrapper types in feature crates.
///
/// This macro generates the full ComponentBridge implementation for components
/// that wrap a Handle<Asset> (like Mesh3d wrapping Handle<Mesh>).
///
/// The Python type must:
/// - Have a single field `PyHandle` at position 0
/// - Implement `From<&BevyType> for PyType`
/// - Have `TryFrom<&PyType> for BevyType` that converts handle
///
/// # Usage
///
/// ```rust
/// // In pybevy_mesh/src/lib.rs
/// handle_bridge!(Mesh3d, PyMesh3d);
/// ```
///
/// This generates `Mesh3dBridge` struct with full `ComponentBridge` impl.
///
/// For generic types:
/// ```rust
/// handle_bridge!(MeshMaterial3d<StandardMaterial>, PyMeshMaterial3d, "MeshMaterial3d");
/// ```
#[proc_macro]
pub fn handle_bridge(input: TokenStream) -> TokenStream {
    struct BridgeArgs {
        bevy_type: Type,
        py_type: Ident,
        bridge_name: Option<String>,
    }

    impl Parse for BridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            input.parse::<Token![,]>()?;
            let py_type: Ident = input.parse()?;

            let bridge_name = if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                Some(input.parse::<syn::LitStr>()?.value())
            } else {
                None
            };

            Ok(BridgeArgs {
                bevy_type,
                py_type,
                bridge_name,
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

            fn register(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                world.register_component::<#bevy_type>()
            }

            fn extract(
                &self,
                entity: &mut bevy::ecs::world::FilteredEntityMut,
                component_id: bevy::ecs::component::ComponentId,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                    pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                })?;

                let component = unsafe { untyped.deref::<#bevy_type>() };
                let py_component = #py_type::from(component);
                let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                Ok(obj.into_any())
            }

            fn extract_fn(&self) -> pybevy_core::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut bevy::ecs::world::FilteredEntityMut,
                    component_id: bevy::ecs::component::ComponentId,
                    _validity: pybevy_core::ValidityFlagWithMode,
                    py: pyo3::Python,
                ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                    let untyped = entity.get_by_id(component_id).ok_or_else(|| {
                        pyo3::exceptions::PyRuntimeError::new_err(concat!(#component_name, " not found"))
                    })?;

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

            fn entity_contains(&self, entity: &bevy::ecs::world::EntityRef) -> bool {
                entity.contains::<#bevy_type>()
            }

            fn extract_from_entity_ref(
                &self,
                entity: &bevy::ecs::world::EntityRef,
                _validity: pybevy_core::ValidityFlagWithMode,
                py: pyo3::Python,
            ) -> pyo3::PyResult<Option<pyo3::Py<pyo3::PyAny>>> {
                if let Some(component) = entity.get::<#bevy_type>() {
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
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
                if let Some(component) = entity.get::<#bevy_type>() {
                    let py_component = #py_type::from(component);
                    let obj = pyo3::Py::new(py, (py_component, pybevy_core::PyComponent))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generates a MessageBridge for event/message types in feature crates.
///
/// This macro generates the full MessageBridge implementation for Bevy events
/// that are exposed as PyBevy messages.
///
/// The Python type must:
/// - Implement `From<&BevyType> for PyType` (for reading events)
/// - Optionally implement `TryFrom<&PyType> for BevyType` (for writable events)
///
/// # Usage
///
/// For read-only events (most input events):
/// ```rust
/// // In pybevy_input/src/lib.rs
/// message_bridge!(KeyboardInput, PyKeyboardInput);
/// ```
///
/// For writable events (like AppExit):
/// ```rust
/// message_bridge!(AppExit, PyAppExit, writable);
/// ```
///
/// For types with different bridge names:
/// ```rust
/// message_bridge!(bevy::input::keyboard::KeyboardInput, PyKeyboardInput, "KeyboardInput");
/// ```
#[proc_macro]
pub fn message_bridge(input: TokenStream) -> TokenStream {
    struct BridgeArgs {
        bevy_type: Type,
        py_type: Ident,
        bridge_name: Option<String>,
        writable: bool,
    }

    impl Parse for BridgeArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            input.parse::<Token![,]>()?;
            let py_type: Ident = input.parse()?;

            let mut bridge_name = None;
            let mut writable = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                if input.peek(syn::LitStr) {
                    bridge_name = Some(input.parse::<syn::LitStr>()?.value());
                } else {
                    let ident: Ident = input.parse()?;
                    if ident == "writable" {
                        writable = true;
                    } else {
                        return Err(syn::Error::new_spanned(
                            ident,
                            "expected 'writable' or a string literal",
                        ));
                    }
                }
            }

            Ok(BridgeArgs {
                bevy_type,
                py_type,
                bridge_name,
                writable,
            })
        }
    }

    let args = parse_macro_input!(input as BridgeArgs);
    let bevy_type = &args.bevy_type;
    let py_type = &args.py_type;
    let writable = args.writable;

    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = args.bridge_name.unwrap_or_else(|| {
        let name = py_type.to_string();
        name.strip_prefix("Py").unwrap_or(&name).to_string()
    });
    let bridge_name = quote::format_ident!("{}Bridge", bridge_name_str);
    let message_name = &bridge_name_str;

    // Generate is_read_only based on writable flag
    let is_read_only_impl = if writable {
        quote! { false }
    } else {
        quote! { true }
    };

    // Generate write_message implementation
    let write_message_impl = if writable {
        quote! {
            fn write_message(
                &self,
                py: pyo3::Python,
                world: &mut bevy::ecs::world::World,
                message: &pyo3::Bound<'_, pyo3::PyAny>,
            ) -> pyo3::PyResult<Box<dyn std::any::Any + Send + Sync>> {
                use bevy::ecs::message::Messages;

                let py_message = message.extract::<pyo3::PyRef<#py_type>>()?;
                let native: #bevy_type = #bevy_type::try_from(&*py_message)?;

                let mut messages = world.get_resource_or_insert_with(|| Messages::<#bevy_type>::default());
                let id = messages.write(native);
                Ok(Box::new(id))
            }
        }
    } else {
        quote! {
            // Uses default implementation which returns read-only error
        }
    };

    let expanded = quote! {
        /// Bridge for #message_name message/event
        pub struct #bridge_name;

        impl pybevy_core::MessageBridge for #bridge_name {
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
                #message_name
            }

            fn is_read_only(&self) -> bool {
                #is_read_only_impl
            }

            fn iter_to_python(
                &self,
                py: pyo3::Python,
                world: &mut bevy::ecs::world::World,
            ) -> pyo3::PyResult<Vec<pyo3::Py<pyo3::PyAny>>> {
                self.iter_to_python_with_cursor(py, world, &mut None)
            }

            fn iter_to_python_with_cursor(
                &self,
                py: pyo3::Python,
                world: &mut bevy::ecs::world::World,
                cursor_storage: &mut Option<Box<dyn std::any::Any + Send + Sync>>,
            ) -> pyo3::PyResult<Vec<pyo3::Py<pyo3::PyAny>>> {
                use bevy::ecs::message::{MessageCursor, Messages};

                let messages = world.get_resource_or_insert_with(|| Messages::<#bevy_type>::default());
                let mut cursor = cursor_storage
                    .as_ref()
                    .and_then(|s| s.downcast_ref::<MessageCursor<#bevy_type>>())
                    .cloned()
                    .unwrap_or_else(|| messages.get_cursor());
                let mut result = Vec::new();
                for event in cursor.read(&*messages) {
                    let py_event = #py_type::from(event);
                    // Use tuple form for types extending PyMessage
                    let py_obj = pyo3::Py::new(py, (py_event, pybevy_core::PyMessage))?;
                    result.push(py_obj.into_any());
                }
                *cursor_storage = Some(Box::new(cursor));
                Ok(result)
            }

            #write_message_impl

            fn clear(&self, world: &mut bevy::ecs::world::World) -> pyo3::PyResult<()> {
                use bevy::ecs::message::Messages;

                let mut messages = world.get_resource_or_insert_with(|| Messages::<#bevy_type>::default());
                messages.clear();
                Ok(())
            }

            fn is_empty(&self, world: &mut bevy::ecs::world::World) -> pyo3::PyResult<bool> {
                use bevy::ecs::message::Messages;

                let messages = world.get_resource_or_insert_with(|| Messages::<#bevy_type>::default());
                Ok(messages.is_empty())
            }

            fn len(&self, world: &mut bevy::ecs::world::World) -> pyo3::PyResult<usize> {
                use bevy::ecs::message::Messages;

                let messages = world.get_resource_or_insert_with(|| Messages::<#bevy_type>::default());
                Ok(messages.len())
            }

            fn resource_id(&self, world: &bevy::ecs::world::World) -> Option<bevy::ecs::component::ComponentId> {
                use bevy::ecs::message::Messages;

                world.components().resource_id::<Messages<#bevy_type>>()
            }
        }
    };

    TokenStream::from(expanded)
}

// ============================================================================
// Field accessor generation (used by #[derive(PyComponent)])
// ============================================================================

/// Field annotation parsed from `#[color]`, `#[borrowed]`, `#[read_only]` attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldAnnotation {
    /// No special annotation - auto-detect based on type
    None,
    /// `#[color]` - Use PyColor::from_color() for getter
    Color,
    /// `#[borrowed]` - Use borrow_field_as() for getter
    Borrowed,
    /// `#[read_only]` - Generate getter only, no setter
    ReadOnly,
}

/// Parsed field definition for accessor generation
struct FieldDef {
    name: Ident,
    ty: Type,
    annotation: FieldAnnotation,
}

/// Check if a type is a primitive (i.e., getter returns `Ok(self.as_ref()?.field)` directly)
fn is_primitive_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let name = segment.ident.to_string();
            return matches!(
                name.as_str(),
                "f32"
                    | "f64"
                    | "i8"
                    | "i16"
                    | "i32"
                    | "i64"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "bool"
                    | "usize"
                    | "isize"
            );
        }
    }
    false
}

/// Check if a type is `Option<T>` and return the inner type if so
fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            if segment.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

/// Generate getter/setter method pairs for a list of field definitions.
///
/// `wrapper_type` is the PyO3 wrapper struct (e.g., `PyPointLight`).
/// Returns a TokenStream of getter/setter methods (without the surrounding `impl` block).
fn generate_field_accessors(
    _wrapper_type: &Ident,
    fields: &[FieldDef],
) -> proc_macro2::TokenStream {
    let mut methods = proc_macro2::TokenStream::new();

    for field in fields {
        let field_name = &field.name;
        let field_ty = &field.ty;
        let setter_name = quote::format_ident!("set_{}", field_name);

        match field.annotation {
            FieldAnnotation::Color => {
                // Color field: getter uses PyColor::from_color(), setter uses .into()
                methods.extend(quote! {
                    #[getter]
                    pub fn #field_name(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::Py<#field_ty>> {
                        #field_ty::from_color(self.as_ref()?.#field_name, py)
                    }

                    #[setter]
                    pub fn #setter_name(&mut self, value: #field_ty) -> pyo3::PyResult<()> {
                        self.as_mut()?.#field_name = value.into();
                        Ok(())
                    }
                });
            }
            FieldAnnotation::Borrowed => {
                // Borrowed field: getter uses borrow_field_as(), setter uses .into()
                methods.extend(quote! {
                    #[getter]
                    pub fn #field_name(&self) -> pyo3::PyResult<#field_ty> {
                        Ok(self.storage.borrow_field_as(|c| &c.#field_name)?)
                    }

                    #[setter]
                    pub fn #setter_name(&mut self, value: #field_ty) -> pyo3::PyResult<()> {
                        self.as_mut()?.#field_name = value.into();
                        Ok(())
                    }
                });
            }
            FieldAnnotation::ReadOnly => {
                // Read-only: getter only, no setter
                if is_primitive_type(field_ty) {
                    methods.extend(quote! {
                        #[getter]
                        pub fn #field_name(&self) -> pyo3::PyResult<#field_ty> {
                            Ok(self.as_ref()?.#field_name)
                        }
                    });
                } else {
                    methods.extend(quote! {
                        #[getter]
                        pub fn #field_name(&self) -> pyo3::PyResult<#field_ty> {
                            Ok(self.as_ref()?.#field_name.into())
                        }
                    });
                }
            }
            FieldAnnotation::None => {
                if let Some(inner_ty) = extract_option_inner(field_ty) {
                    // Option<T> field
                    if is_primitive_type(inner_ty) {
                        methods.extend(quote! {
                            #[getter]
                            pub fn #field_name(&self) -> pyo3::PyResult<Option<#inner_ty>> {
                                Ok(self.as_ref()?.#field_name)
                            }

                            #[setter]
                            pub fn #setter_name(&mut self, value: Option<#inner_ty>) -> pyo3::PyResult<()> {
                                self.as_mut()?.#field_name = value;
                                Ok(())
                            }
                        });
                    } else {
                        methods.extend(quote! {
                            #[getter]
                            pub fn #field_name(&self) -> pyo3::PyResult<Option<#inner_ty>> {
                                Ok(self.as_ref()?.#field_name.as_ref().map(|v| v.into()))
                            }

                            #[setter]
                            pub fn #setter_name(&mut self, value: Option<#inner_ty>) -> pyo3::PyResult<()> {
                                self.as_mut()?.#field_name = value.map(Into::into);
                                Ok(())
                            }
                        });
                    }
                } else if is_primitive_type(field_ty) {
                    // Primitive: direct copy
                    methods.extend(quote! {
                        #[getter]
                        pub fn #field_name(&self) -> pyo3::PyResult<#field_ty> {
                            Ok(self.as_ref()?.#field_name)
                        }

                        #[setter]
                        pub fn #setter_name(&mut self, value: #field_ty) -> pyo3::PyResult<()> {
                            self.as_mut()?.#field_name = value;
                            Ok(())
                        }
                    });
                } else {
                    // Non-primitive: use .into() conversions
                    methods.extend(quote! {
                        #[getter]
                        pub fn #field_name(&self) -> pyo3::PyResult<#field_ty> {
                            Ok(self.as_ref()?.#field_name.into())
                        }

                        #[setter]
                        pub fn #setter_name(&mut self, value: #field_ty) -> pyo3::PyResult<()> {
                            self.as_mut()?.#field_name = value.into();
                            Ok(())
                        }
                    });
                }
            }
        }
    }

    methods
}

/// Parse field annotations from attributes
fn parse_field_annotation(attrs: &[Attribute]) -> FieldAnnotation {
    for attr in attrs {
        if attr.path().is_ident("color") {
            return FieldAnnotation::Color;
        }
        if attr.path().is_ident("borrowed") {
            return FieldAnnotation::Borrowed;
        }
        if attr.path().is_ident("read_only") {
            return FieldAnnotation::ReadOnly;
        }
    }
    FieldAnnotation::None
}

// ============================================================================
// #[derive(PyComponent)] macro
// ============================================================================

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
/// ```rust
/// #[derive(Component, PyComponent)]
/// struct Health {
///     value: f32,
///     max: f32,
/// }
/// ```
///
/// Generates `PyHealth`, `HealthBridge`, and `register_health()`.
#[proc_macro_derive(PyComponent, attributes(color, borrowed, read_only, py_name))]
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
            if is_primitive_type(ty) || extract_option_inner(ty).is_some() {
                quote! { #name: #ty }
            } else {
                quote! { #name: #ty }
            }
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
        // 1. Wrapper struct
        #[#pyo3_path::pyclass(name = #py_name_str, extends = #core_path::PyComponent)]
        #[derive(Debug, Clone)]
        pub struct #py_type {
            storage: #core_path::ComponentStorage<#bevy_type>,
        }

        // 2. From/TryFrom impls and helper methods
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

        // 3. #[pymethods] with constructor and field accessors
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

        // 4. Bridge struct implementing ComponentBridge
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
                entity: &mut bevy::ecs::world::FilteredEntityMut,
                component_id: bevy::ecs::component::ComponentId,
                validity: #core_path::ValidityFlagWithMode,
                py: #pyo3_path::Python,
            ) -> #pyo3_path::PyResult<#pyo3_path::Py<#pyo3_path::PyAny>> {
                let mut untyped = entity.get_mut_by_id(component_id).ok_or_else(|| {
                    #pyo3_path::exceptions::PyRuntimeError::new_err(concat!(#bevy_type_str, " not found"))
                })?;

                let ptr = unsafe {
                    untyped.as_mut().deref_mut::<#bevy_type>() as *mut #bevy_type
                };

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

            fn extract_fn(&self) -> #core_path::ExtractFn {
                #[inline(always)]
                fn extract_impl(
                    entity: &mut bevy::ecs::world::FilteredEntityMut,
                    component_id: bevy::ecs::component::ComponentId,
                    validity: #core_path::ValidityFlagWithMode,
                    py: #pyo3_path::Python,
                ) -> #pyo3_path::PyResult<#pyo3_path::Py<#pyo3_path::PyAny>> {
                    let mut untyped = entity.get_mut_by_id(component_id).ok_or_else(|| {
                        #pyo3_path::exceptions::PyRuntimeError::new_err("Component not found")
                    })?;

                    let ptr = unsafe {
                        untyped.as_mut().deref_mut::<#bevy_type>() as *mut #bevy_type
                    };

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

            fn extract_from_entity_ref(
                &self,
                entity: &bevy::ecs::world::EntityRef,
                validity: #core_path::ValidityFlagWithMode,
                py: #pyo3_path::Python,
            ) -> #pyo3_path::PyResult<Option<#pyo3_path::Py<#pyo3_path::PyAny>>> {
                if let Some(component) = entity.get::<#bevy_type>() {
                    let ptr = component as *const #bevy_type as *mut #bevy_type;
                    let storage = unsafe {
                        #core_path::ComponentStorage::borrowed(ptr, validity)
                    };
                    let obj = #pyo3_path::Py::new(py, #py_type::from_borrowed(storage))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }

            fn extract_from_entity_mut(
                &self,
                entity: &mut bevy::ecs::world::EntityWorldMut,
                validity: #core_path::ValidityFlagWithMode,
                py: #pyo3_path::Python,
            ) -> #pyo3_path::PyResult<Option<#pyo3_path::Py<#pyo3_path::PyAny>>> {
                if let Some(mut component) = entity.get_mut::<#bevy_type>() {
                    let ptr = component.as_mut() as *mut #bevy_type;
                    let storage = unsafe {
                        #core_path::ComponentStorage::borrowed(ptr, validity)
                    };
                    let obj = #pyo3_path::Py::new(py, #py_type::from_borrowed(storage))?;
                    Ok(Some(obj.into_any()))
                } else {
                    Ok(None)
                }
            }
        }

        // 5. Registration function
        pub fn #register_fn() {
            #core_path::registry::global_registry::register_component_bridge(#bridge_type);
        }
    };

    TokenStream::from(expanded)
}

/// Convert CamelCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }
    result
}
