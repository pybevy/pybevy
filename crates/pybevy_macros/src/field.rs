use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, Type, parse_macro_input};

use crate::util::find_storage_field_type;

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
/// ```rust,ignore
/// #[pyfield]  // Bevy type inferred from storage field
/// #[pyclass(name = "BloomPrefilter")]
/// pub struct PyBloomPrefilter {
///     storage: FieldStorage<BloomPrefilter>,
/// }
/// ```
///
/// For explicit type specification (e.g., complex generics):
///
/// ```rust,ignore
/// #[pyfield(SomeComplexType<T>)]
/// #[pyclass(name = "MyType")]
/// pub struct PyMyType {
///     storage: FieldStorage<SomeComplexType<T>>,
/// }
/// ```
pub fn pyfield(attr: TokenStream, item: TokenStream) -> TokenStream {
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
