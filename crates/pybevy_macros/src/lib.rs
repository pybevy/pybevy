extern crate proc_macro;

use proc_macro::TokenStream;

mod app;
mod asset;
mod component;
mod derive;
mod enum_component;
mod enum_message;
mod enum_spec;
mod enums;
mod field;
mod message;
mod newtype;
mod plugin;
mod resource;
mod unit;
mod util;
mod value;

#[proc_macro_attribute]
pub fn pybevy_app(attr: TokenStream, item: TokenStream) -> TokenStream {
    app::pybevy_app(attr, item)
}

#[proc_macro_attribute]
pub fn native_asset(attr: TokenStream, item: TokenStream) -> TokenStream {
    asset::native_asset(attr, item)
}

#[proc_macro_attribute]
pub fn pyasset(attr: TokenStream, item: TokenStream) -> TokenStream {
    asset::pyasset(attr, item)
}

#[proc_macro_attribute]
pub fn pyhandle(attr: TokenStream, item: TokenStream) -> TokenStream {
    asset::pyhandle(attr, item)
}

#[proc_macro_attribute]
pub fn pycomponent(attr: TokenStream, item: TokenStream) -> TokenStream {
    component::pycomponent(attr, item)
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
pub fn pyenum(attr: TokenStream, item: TokenStream) -> TokenStream {
    enums::pyenum(attr, item)
}

#[proc_macro_attribute]
pub fn pyfield(attr: TokenStream, item: TokenStream) -> TokenStream {
    field::pyfield(attr, item)
}

#[proc_macro_attribute]
pub fn pyvalue(attr: TokenStream, item: TokenStream) -> TokenStream {
    value::pyvalue(attr, item)
}

#[proc_macro_attribute]
pub fn pymessage(attr: TokenStream, item: TokenStream) -> TokenStream {
    message::pymessage(attr, item)
}

#[proc_macro_attribute]
pub fn pywrap(attr: TokenStream, item: TokenStream) -> TokenStream {
    newtype::pywrap(attr, item)
}

#[proc_macro_attribute]
pub fn pyplugin(attr: TokenStream, item: TokenStream) -> TokenStream {
    plugin::pyplugin(attr, item)
}

#[proc_macro_attribute]
pub fn native_resource(attr: TokenStream, item: TokenStream) -> TokenStream {
    resource::native_resource(attr, item)
}

#[proc_macro_attribute]
pub fn pyresource(attr: TokenStream, item: TokenStream) -> TokenStream {
    resource::pyresource(attr, item)
}
