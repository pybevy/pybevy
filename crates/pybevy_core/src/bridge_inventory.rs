//! Inventory-based auto-registration for bridges.
//!
//! Feature crates submit bridge registrations via `inventory::submit!()`,
//! and `collect_all()` is called once at startup to register them all.

use std::sync::Arc;

use crate::{
    plugin::{PluginBridge, plugin_registry},
    registry::{AssetBridge, ComponentBridge, MessageBridge, ResourceBridge, global_registry},
};

/// A component bridge registration collected via `inventory`.
pub struct ComponentBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn ComponentBridge>,
}

inventory::collect!(ComponentBridgeRegistration);

/// A resource bridge registration collected via `inventory`.
pub struct ResourceBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn ResourceBridge>,
}

inventory::collect!(ResourceBridgeRegistration);

/// A message bridge registration collected via `inventory`.
pub struct MessageBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn MessageBridge>,
}

inventory::collect!(MessageBridgeRegistration);

/// A plugin bridge registration collected via `inventory`.
pub struct PluginBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn PluginBridge>,
}

inventory::collect!(PluginBridgeRegistration);

/// An asset bridge registration collected via `inventory`.
pub struct AssetBridgeRegistration {
    /// Factory function that returns the bridge instance.
    pub create: fn() -> Arc<dyn AssetBridge>,
}

inventory::collect!(AssetBridgeRegistration);

/// A batch metadata registration collected via `inventory`.
pub struct BatchRegistration {
    /// Registration function that registers batch metadata.
    pub register: fn(),
}

inventory::collect!(BatchRegistration);

/// Collect all inventory-registered bridges and register them in the global registries.
///
/// Call this once at startup (e.g., in `init_module`).
pub fn collect_all() {
    for reg in inventory::iter::<ComponentBridgeRegistration> {
        let bridge = (reg.create)();
        global_registry::register_component_bridge_arc(bridge);
    }

    for reg in inventory::iter::<ResourceBridgeRegistration> {
        let bridge = (reg.create)();
        global_registry::register_resource_bridge_arc(bridge);
    }

    for reg in inventory::iter::<MessageBridgeRegistration> {
        let bridge = (reg.create)();
        global_registry::register_message_bridge_arc(bridge);
    }

    for reg in inventory::iter::<AssetBridgeRegistration> {
        let bridge = (reg.create)();
        global_registry::register_asset_bridge_arc(bridge);
    }

    for reg in inventory::iter::<PluginBridgeRegistration> {
        let bridge = (reg.create)();
        plugin_registry::register_plugin_bridge_arc(bridge);
    }

    for reg in inventory::iter::<BatchRegistration> {
        (reg.register)();
    }
}
