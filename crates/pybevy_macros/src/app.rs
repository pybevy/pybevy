use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;

/// A procedural macro to create a pybevy application module from a native Bevy App function.
///
/// # Example
///
/// ```rust,ignore
/// use pybevy::pybevy_app;
///
/// #[pybevy_app]
/// pub fn native_app() -> App {
///     let mut app = App::new();
///     app.add_plugins(DefaultPlugins.pythonize()).add_systems(Startup, setup);
///     app
/// }
/// ```
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
