use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Ident, ItemStruct, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

/// Attribute proc macro for message bridges.
///
/// Goes on the struct definition and generates the MessageBridge impl + inventory registration.
///
/// # Usage
///
/// ```rust,ignore
/// #[pymessage(CursorEntered)]
/// #[pyclass(name = "CursorEntered", extends = PyMessage, frozen, eq)]
/// pub struct PyCursorEntered { ... }
/// ```
pub fn pymessage(attr: TokenStream, item: TokenStream) -> TokenStream {
    struct MessageStorageArgs {
        bevy_type: Type,
        writable: bool,
    }

    impl Parse for MessageStorageArgs {
        fn parse(input: ParseStream) -> syn::Result<Self> {
            let bevy_type: Type = input.parse()?;
            let mut writable = false;

            while input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                let ident: Ident = input.parse()?;
                if ident == "writable" {
                    writable = true;
                } else {
                    return Err(syn::Error::new_spanned(ident, "expected 'writable'"));
                }
            }

            Ok(MessageStorageArgs {
                bevy_type,
                writable,
            })
        }
    }

    let args = parse_macro_input!(attr as MessageStorageArgs);
    let input = parse_macro_input!(item as ItemStruct);
    let py_type = &input.ident;

    let bridge_tokens = generate_message_bridge_tokens(
        &args.bevy_type,
        py_type,
        None,
        args.writable,
        true, // emit inventory
    );

    let expanded = quote! {
        #input
        #bridge_tokens
    };

    TokenStream::from(expanded)
}

/// Shared message bridge code generation.
fn generate_message_bridge_tokens(
    bevy_type: &Type,
    py_type: &Ident,
    bridge_name_override: Option<&str>,
    writable: bool,
    emit_inventory: bool,
) -> proc_macro2::TokenStream {
    // Derive bridge name: either from explicit string or from py_type (strip "Py" prefix)
    let bridge_name_str = bridge_name_override.map(String::from).unwrap_or_else(|| {
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

                // Writers never insert: a structural world mutation from a
                // possibly-parallel system is unsound. The owning plugin's
                // add_message inserts the buffer, so a missing one means the
                // message is not initialized.
                let Some(mut messages) = world.get_resource_mut::<Messages<#bevy_type>>() else {
                    return Err(pyo3::exceptions::PyTypeError::new_err(
                        "Messages resource not initialized for this message type. \
                         Ensure the plugin providing this message is added before writing.",
                    ));
                };
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

                // A missing buffer reads as empty; never insert (no structural
                // world mutation from a possibly-parallel system).
                let Some(messages) = world.get_resource::<Messages<#bevy_type>>() else {
                    return Ok(Vec::new());
                };
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

            // Maintenance paths treat a missing buffer as empty; never insert
            // (no structural world mutation from a possibly-parallel system).
            fn clear(&self, world: &mut bevy::ecs::world::World) -> pyo3::PyResult<()> {
                use bevy::ecs::message::Messages;

                if let Some(mut messages) = world.get_resource_mut::<Messages<#bevy_type>>() {
                    messages.clear();
                }
                Ok(())
            }

            fn is_empty(&self, world: &mut bevy::ecs::world::World) -> pyo3::PyResult<bool> {
                use bevy::ecs::message::Messages;

                Ok(world
                    .get_resource::<Messages<#bevy_type>>()
                    .is_none_or(|messages| messages.is_empty()))
            }

            fn len(&self, world: &mut bevy::ecs::world::World) -> pyo3::PyResult<usize> {
                use bevy::ecs::message::Messages;

                Ok(world
                    .get_resource::<Messages<#bevy_type>>()
                    .map_or(0, |messages| messages.len()))
            }

            fn resource_id(&self, world: &bevy::ecs::world::World) -> Option<bevy::ecs::component::ComponentId> {
                use bevy::ecs::message::Messages;

                world.components().component_id::<Messages<#bevy_type>>()
            }

            fn register_resource_id(&self, world: &mut bevy::ecs::world::World) -> bevy::ecs::component::ComponentId {
                use bevy::ecs::message::Messages;

                world.register_component::<Messages<#bevy_type>>()
            }
        }
    };

    let inventory_submit = if emit_inventory {
        quote! {
            pybevy_core::inventory::submit!(pybevy_core::MessageBridgeRegistration {
                create: || std::sync::Arc::new(#bridge_name),
            });
        }
    } else {
        quote! {}
    };

    quote! {
        #expanded
        #inventory_submit
    }
}
