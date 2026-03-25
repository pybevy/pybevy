use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

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
