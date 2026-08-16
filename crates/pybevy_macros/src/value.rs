use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, Type, parse_macro_input};

use crate::util::find_storage_field_type;

/// Generates the shared storage boilerplate for `ValueStorage`-backed wrappers.
///
/// This is the `ValueStorage` counterpart to [`pyfield`](crate::pyfield), which
/// covers `FieldStorage`. Use it for wrappers over Copy Bevy types (`Vec3`,
/// `Rect`, `Mat4`, `Quat`); use `#[pyfield]` for non-Copy Clone types.
///
/// This macro generates:
/// - `impl FromBorrowedStorage<ValueStorage<BevyType>> for PyType`
/// - `from_owned()`, `from_borrowed()`, `as_ref()`, `as_mut()`, `to_bevy()`
///
/// It deliberately generates **no** conversion impls. Most `ValueStorage`
/// wrappers carry an infallible `From<PyType> for BevyType`, and the standard
/// library's blanket `impl<T, U: Into<T>> TryFrom<U> for T` makes a generated
/// `TryFrom` a coherence error on those types. `From` and `TryFrom` stay with
/// the author because the choice is semantic, not mechanical. See
/// `docs/PLAN-pyvalue-macro.md`.
///
/// `to_bevy()` is fallible on purpose: `ValueStorage::get` fails when a borrowed
/// value's validity flag has been cleared, and substituting a default there
/// would report a plausible wrong value instead of the expired borrow.
///
/// # Usage
///
/// ```rust,ignore
/// #[pyvalue]  // Bevy type inferred from the storage field
/// #[pyclass(name = "Rect")]
/// #[derive(Debug, Clone)]
/// pub struct PyRect {
///     pub(crate) storage: ValueStorage<Rect>,
/// }
/// ```
///
/// For a type the inference cannot spell (generics, fully-qualified paths):
///
/// ```rust,ignore
/// #[pyvalue(bevy::math::Isometry3d)]
/// ```
pub fn pyvalue(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    let bevy_type: Type = if attr.is_empty() {
        match find_storage_field_type(&input, "ValueStorage") {
            Ok(ty) => ty.clone(),
            Err(e) => return e.to_compile_error().into(),
        }
    } else {
        parse_macro_input!(attr as Type)
    };

    let expanded = quote! {
        #input

        impl FromBorrowedStorage<ValueStorage<#bevy_type>> for #py_type {
            fn from_borrowed(storage: ValueStorage<#bevy_type>) -> Self {
                Self { storage }
            }
        }

        impl #py_type {
            /// Create from an owned value.
            pub(crate) fn from_owned(value: #bevy_type) -> Self {
                Self {
                    storage: ValueStorage::owned(value),
                }
            }

            /// Create from a borrowed value storage.
            pub(crate) fn from_borrowed(storage: ValueStorage<#bevy_type>) -> Self {
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

            /// Read the wrapped value.
            ///
            /// Fails when the underlying borrow has expired, rather than
            /// substituting a default.
            #[inline(always)]
            pub(crate) fn to_bevy(&self) -> PyResult<#bevy_type> {
                Ok(self.storage.get()?)
            }
        }
    };

    TokenStream::from(expanded)
}
