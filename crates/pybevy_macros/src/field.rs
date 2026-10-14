use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, Type, parse_macro_input};

use crate::{
    domain::{StorageTypeArgs, domain_impl_tokens, owner_name},
    util::find_storage_field_type,
};

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

    let args = parse_macro_input!(attr as StorageTypeArgs);
    let bevy_type: Type = match args.bevy_type {
        Some(ty) => ty,
        None => match find_storage_field_type(&input, "FieldStorage") {
            Ok(ty) => ty.clone(),
            Err(e) => return e.to_compile_error().into(),
        },
    };
    let domain_tokens = domain_impl_tokens(
        &bevy_type,
        py_type,
        &owner_name(py_type, None),
        &args.domains,
    );

    let expanded = quote! {
        #input

        #domain_tokens

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
            pub(crate) fn as_ref(&self) -> PyResult<pybevy_core::StorageRef<'_, #bevy_type>> {
                Ok(self.storage.as_ref()?)
            }

            #[inline(always)]
            pub(crate) fn as_mut(&mut self) -> PyResult<pybevy_core::StorageMut<'_, #bevy_type>> {
                Ok(self.storage.as_mut()?)
            }
        }
    };

    TokenStream::from(expanded)
}
