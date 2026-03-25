use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

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
