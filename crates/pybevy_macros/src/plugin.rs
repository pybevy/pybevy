use proc_macro::TokenStream;
use quote::quote;
use syn::{Ident, ItemStruct, Token, Type, parse::Parse, parse_macro_input};

struct PluginArgs {
    bevy_type: Type,
    default_plugin: Option<Ident>,
}

impl Parse for PluginArgs {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let bevy_type = input.parse()?;
        let default_plugin = if input.is_empty() {
            None
        } else {
            input.parse::<Token![,]>()?;
            let option: Ident = input.parse()?;
            if option != "default_plugin" {
                return Err(syn::Error::new(
                    option.span(),
                    "expected `default_plugin = Variant`",
                ));
            }
            input.parse::<Token![=]>()?;
            let kind = input.parse()?;
            if !input.is_empty() {
                return Err(input.error("unexpected pyplugin option"));
            }
            Some(kind)
        };

        Ok(Self {
            bevy_type,
            default_plugin,
        })
    }
}

/// Attribute proc macro for plugin wrappers.
///
/// Generates the PluginBridge impl and inventory registration for a struct
/// that implements `PluginBuild`.
///
/// # Usage
///
/// ```rust,ignore
/// #[pyplugin(bevy::window::WindowPlugin, default_plugin = Window)]
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
pub fn pyplugin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let PluginArgs {
        bevy_type,
        default_plugin,
    } = parse_macro_input!(attr as PluginArgs);
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
    let default_plugin_kind = match default_plugin {
        Some(kind) => quote! {
            fn default_plugin_kind(&self) -> Option<pybevy_core::DefaultPluginKind> {
                Some(pybevy_core::DefaultPluginKind::#kind)
            }
        },
        None => quote! {},
    };

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

            #default_plugin_kind

            fn build(&self, py_plugin: &pyo3::Bound<'_, pyo3::PyAny>, app: &mut bevy::app::App) -> pyo3::PyResult<()> {
                <#py_type as pybevy_core::PluginBuild>::build(py_plugin, app)
            }

            fn is_added(&self, app: &bevy::app::App) -> bool {
                app.is_plugin_added::<#bevy_type>()
            }
        }

        pybevy_core::inventory::submit!(pybevy_core::PluginBridgeRegistration {
            create: || std::sync::Arc::new(#bridge_name),
        });
    };

    TokenStream::from(expanded)
}
