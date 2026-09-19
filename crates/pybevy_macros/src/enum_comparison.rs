use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Ident, Meta, Token, punctuated::Punctuated};

pub(crate) fn identity_methods(name: &Ident) -> TokenStream {
    quote! {
        #[pyo3::pymethods]
        impl #name {
            fn __richcmp__(
                &self,
                other: &pyo3::Bound<'_, pyo3::PyAny>,
                op: pyo3::pyclass::CompareOp,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                if matches!(op, pyo3::pyclass::CompareOp::Eq | pyo3::pyclass::CompareOp::Ne) {
                    pybevy_core::enum_comparison::reject_variant_type::<Self>(other)?;
                }
                Ok(other.py().NotImplemented())
            }

            fn __hash__(slf: &pyo3::Bound<'_, Self>) -> pyo3::PyResult<isize> {
                pyo3::types::PyAnyMethods::call_method1(
                    slf.py().get_type::<pyo3::PyAny>().as_any(), "__hash__", (slf,)
                )?.extract()
            }
        }
    }
}

pub(crate) fn value_methods(attrs: &mut [Attribute], name: &Ident) -> syn::Result<TokenStream> {
    let Some(attr) = attrs
        .iter_mut()
        .find(|attr| attr.path().is_ident("pyclass"))
    else {
        return Ok(TokenStream::new());
    };
    let options = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    if !options.iter().any(|option| option.path().is_ident("eq"))
        || options
            .iter()
            .any(|option| option.path().is_ident("eq_int"))
    {
        return Ok(TokenStream::new());
    }
    let hash = options.iter().any(|option| option.path().is_ident("hash"));
    if hash
        && !options
            .iter()
            .any(|option| option.path().is_ident("frozen"))
    {
        return Err(syn::Error::new_spanned(attr, "hash requires frozen"));
    }
    let ord = options.iter().any(|option| option.path().is_ident("ord"));
    let retained: Vec<_> = options
        .into_iter()
        .filter(|option| {
            !["eq", "hash", "ord"]
                .iter()
                .any(|name| option.path().is_ident(name))
        })
        .collect();
    *attr = syn::parse_quote!(#[pyclass(#(#retained),*)]);
    let ordering = ord.then(|| {
        quote! {
            pyo3::pyclass::CompareOp::Lt => pyo3::IntoPyObjectExt::into_py_any(self < &*other, py),
            pyo3::pyclass::CompareOp::Le => pyo3::IntoPyObjectExt::into_py_any(self <= &*other, py),
            pyo3::pyclass::CompareOp::Gt => pyo3::IntoPyObjectExt::into_py_any(self > &*other, py),
            pyo3::pyclass::CompareOp::Ge => pyo3::IntoPyObjectExt::into_py_any(self >= &*other, py),
        }
    });
    let fallback = (!ord).then(|| quote! { _ => Ok(py.NotImplemented()), });
    let hashing = hash.then(|| {
        quote! {
            fn __hash__(&self) -> u64 {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                std::hash::Hash::hash(self, &mut hasher);
                std::hash::Hasher::finish(&hasher)
            }
        }
    });
    Ok(quote! {
        #[pyo3::pymethods]
        impl #name {
            fn __richcmp__(
                &self,
                other: &pyo3::Bound<'_, pyo3::PyAny>,
                op: pyo3::pyclass::CompareOp,
            ) -> pyo3::PyResult<pyo3::Py<pyo3::PyAny>> {
                let py = other.py();
                if matches!(op, pyo3::pyclass::CompareOp::Eq | pyo3::pyclass::CompareOp::Ne) {
                    pybevy_core::enum_comparison::reject_variant_type::<Self>(other)?;
                }
                let Ok(other) = pyo3::types::PyAnyMethods::extract::<pyo3::PyRef<'_, Self>>(other) else {
                    return Ok(py.NotImplemented());
                };
                match op {
                    pyo3::pyclass::CompareOp::Eq => pyo3::IntoPyObjectExt::into_py_any(self == &*other, py),
                    pyo3::pyclass::CompareOp::Ne => pyo3::IntoPyObjectExt::into_py_any(self != &*other, py),
                    #ordering
                    #fallback
                }
            }
            #hashing
        }
    })
}
