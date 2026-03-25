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

#[proc_macro]
pub fn asset_bridge(input: TokenStream) -> TokenStream {
    asset::asset_bridge(input)
}

#[proc_macro]
pub fn handle_bridge(input: TokenStream) -> TokenStream {
    asset::handle_bridge(input)
}

#[proc_macro_attribute]
pub fn component_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    component::component_storage(attr, item)
}

#[proc_macro]
pub fn component_bridge(input: TokenStream) -> TokenStream {
    component::component_bridge(input)
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

#[proc_macro]
pub fn message_bridge(input: TokenStream) -> TokenStream {
    message::message_bridge(input)
}

#[proc_macro_attribute]
pub fn newtype_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    newtype::newtype_storage(attr, item)
}

#[proc_macro]
pub fn newtype_bridge(input: TokenStream) -> TokenStream {
    newtype::newtype_bridge(input)
}

#[proc_macro]
pub fn plugin_bridge(input: TokenStream) -> TokenStream {
    plugin::plugin_bridge(input)
}

#[proc_macro_attribute]
pub fn native_resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    resource::native_resource(attr, item)
}

#[proc_macro_attribute]
pub fn resource_storage(attr: TokenStream, item: TokenStream) -> TokenStream {
    resource::resource_storage(attr, item)
}

#[proc_macro]
pub fn resource_bridge(input: TokenStream) -> TokenStream {
    resource::resource_bridge(input)
}

#[proc_macro]
pub fn unit_bridge(input: TokenStream) -> TokenStream {
    unit::unit_bridge(input)
}
