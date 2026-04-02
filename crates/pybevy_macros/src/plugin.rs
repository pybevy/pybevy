use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, Type, parse_macro_input};

/// Attribute proc macro for plugin wrappers.
///
/// Generates the PluginBridge impl and inventory registration for a struct
/// that implements `PluginBuild`.
///
/// # Usage
///
/// ```rust
/// #[plugin_storage(bevy::window::WindowPlugin)]
/// #[pyclass(name = "WindowPlugin", extends = PyPlugin)]
/// pub struct PyWindowPlugin { ... }
///
/// impl PluginBuild for PyWindowPlugin {
///     fn build(py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
///         let config: PyRef<'_, PyWindowPlugin> = py_plugin.extract()?;
///         app.add_plugins(bevy::window::WindowPlugin::try_from(&*config)?);
///         Ok(())
///     }
/// }
/// ```
pub fn plugin_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    let bevy_type: Type = parse_macro_input!(attr as Type);
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    // Extract simple name for bridge struct
    let bevy_type_name = match &bevy_type {
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

    let expanded = quote! {
        #input

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

            fn build(&self, py_plugin: &pyo3::Bound<'_, pyo3::PyAny>, app: &mut bevy::app::App) -> pyo3::PyResult<()> {
                <#py_type as pybevy_core::PluginBuild>::build(py_plugin, app)
            }
        }

        pybevy_core::inventory::submit!(pybevy_core::PluginBridgeRegistration {
            create: || std::sync::Arc::new(#bridge_name),
        });
    };

    TokenStream::from(expanded)
}
