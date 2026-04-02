extern crate proc_macro;

use proc_macro::TokenStream;

mod app;
mod asset;
mod component;
mod derive;
mod enums;
mod field;
mod message;
mod newtype;
mod plugin;
mod resource;
mod unit;
mod util;

#[proc_macro_attribute]
pub fn pybevy_app(attr: TokenStream, item: TokenStream) -> TokenStream {
    app::pybevy_app(attr, item)
}

#[proc_macro_attribute]
pub fn native_asset(attr: TokenStream, item: TokenStream) -> TokenStream {
    asset::native_asset(attr, item)
}

#[proc_macro_attribute]
pub fn asset_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    asset::asset_storage(attr, item)
}

#[proc_macro_attribute]
pub fn handle_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    asset::handle_storage(attr, item)
}

#[proc_macro_attribute]
pub fn component_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    component::component_storage(attr, item)
}

#[proc_macro_attribute]
pub fn native_component(attr: TokenStream, item: TokenStream) -> TokenStream {
    component::native_component(attr, item)
}

#[proc_macro_derive(PyComponent, attributes(color, borrowed, read_only, py_name))]
pub fn derive_py_component(input: TokenStream) -> TokenStream {
    derive::derive_py_component(input)
}

#[proc_macro_attribute]
pub fn bevy_enum(attr: TokenStream, item: TokenStream) -> TokenStream {
    enums::bevy_enum(attr, item)
}

#[proc_macro_attribute]
pub fn native_field(attr: TokenStream, item: TokenStream) -> TokenStream {
    field::native_field(attr, item)
}

#[proc_macro_attribute]
pub fn message_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    message::message_storage(attr, item)
}

#[proc_macro_attribute]
pub fn newtype_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    newtype::newtype_storage(attr, item)
}

#[proc_macro_attribute]
pub fn plugin_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    plugin::plugin_storage(attr, item)
}

#[proc_macro_attribute]
pub fn native_resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    resource::native_resource(attr, item)
}

#[proc_macro_attribute]
pub fn resource_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    resource::resource_storage(attr, item)
}
