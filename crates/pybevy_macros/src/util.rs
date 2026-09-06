use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::Span;
use quote::quote;
use syn::{Attribute, Fields, GenericArgument, Ident, ItemStruct, PathArguments, Type};

/// Resolve the path to the pybevy crate, handling both internal
/// (within pybevy workspace) and external (user crate) usage.
///
/// Returns token streams for `pybevy_core` and `pyo3` paths.
///
/// Note: External crates must also add `pyo3` as a direct dependency
/// because PyO3's proc macros (`#[pyclass]`, `#[pymethods]`) generate
/// code that references `::pyo3::` with absolute paths.
pub(crate) fn pybevy_crate_paths() -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    match crate_name("pybevy") {
        Ok(FoundCrate::Itself) => {
            // We're inside the pybevy crate itself: use direct deps
            (quote! { pybevy_core }, quote! { pyo3 })
        }
        Ok(FoundCrate::Name(name)) => {
            // External crate: use re-exports for pybevy_core,
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

/// Inventory submission registering `bevy_type` into the app's TypeRegistry,
/// so bridged types stay reflect-reachable without bevy's
/// `reflect_auto_register`. Suppressed by the macros' `no_reflect` option
/// (for bevy types without a `Reflect` derive).
pub(crate) fn reflect_registration_tokens(
    bevy_type: &Type,
    no_reflect: bool,
) -> proc_macro2::TokenStream {
    if no_reflect {
        quote! {}
    } else {
        quote! {
            pybevy_core::inventory::submit!(pybevy_core::ReflectTypeRegistration {
                register: |registry| {
                    registry.register::<#bevy_type>();
                },
            });
        }
    }
}

/// Extract the inner type from a generic storage type like `AssetStorage<T>` or `ComponentStorage<T>`
pub(crate) fn extract_generic_inner_type<'a>(
    ty: &'a Type,
    expected_wrapper: &str,
) -> Option<&'a Type> {
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
pub(crate) fn find_storage_field_type<'a>(
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

/// Field annotation parsed from `#[color]`, `#[borrowed]`, `#[read_only]` attributes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldAnnotation {
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
pub(crate) struct FieldDef {
    pub(crate) name: Ident,
    pub(crate) ty: Type,
    pub(crate) annotation: FieldAnnotation,
}

/// Check if a type is a primitive (i.e., getter returns `Ok(self.as_ref()?.field)` directly)
pub(crate) fn is_primitive_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
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
    false
}

/// Check if a type is an owned Rust string.
pub(crate) fn is_string_type(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
    {
        return segment.ident == "String";
    }
    false
}

/// Check if a type is `Option<T>` and return the inner type if so
pub(crate) fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty
        && let Some(segment) = type_path.path.segments.last()
        && segment.ident == "Option"
        && let PathArguments::AngleBracketed(args) = &segment.arguments
        && let Some(GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

/// Generate getter/setter method pairs for a list of field definitions.
///
/// `wrapper_type` is the PyO3 wrapper struct (e.g., `PyPointLight`).
/// Returns a TokenStream of getter/setter methods (without the surrounding `impl` block).
pub(crate) fn generate_field_accessors(
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
                methods.extend(quote! {
                    #[getter]
                    pub fn #field_name(&self, py: pyo3::Python) -> pyo3::PyResult<pyo3::Py<#field_ty>> {
                        #field_ty::from_color(self.as_ref()?.#field_name, py)
                    }

                    #[setter]
                    pub fn #setter_name(&mut self, value: #field_ty) -> pyo3::PyResult<()> {
                        let value = value.resolved_copy()?;
                        self.as_mut()?.#field_name = value;
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
                } else if is_string_type(field_ty) {
                    methods.extend(quote! {
                        #[getter]
                        pub fn #field_name(&self) -> pyo3::PyResult<String> {
                            Ok(self.as_ref()?.#field_name.clone())
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
                    } else if is_string_type(inner_ty) {
                        methods.extend(quote! {
                            #[getter]
                            pub fn #field_name(&self) -> pyo3::PyResult<Option<String>> {
                                Ok(self.as_ref()?.#field_name.clone())
                            }

                            #[setter]
                            pub fn #setter_name(&mut self, value: Option<String>) -> pyo3::PyResult<()> {
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
                } else if is_string_type(field_ty) {
                    methods.extend(quote! {
                        #[getter]
                        pub fn #field_name(&self) -> pyo3::PyResult<String> {
                            Ok(self.as_ref()?.#field_name.clone())
                        }

                        #[setter]
                        pub fn #setter_name(&mut self, value: String) -> pyo3::PyResult<()> {
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
pub(crate) fn parse_field_annotation(attrs: &[Attribute]) -> FieldAnnotation {
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

/// Convert CamelCase to snake_case
pub(crate) fn to_snake_case(s: &str) -> String {
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
