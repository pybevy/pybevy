//! Interpreter-neutral plugin-group operation metadata.

/// Public `DefaultPlugins` members that PyBevy can configure or order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DefaultPluginKind {
    Audio,
    Image,
    Render,
    TaskPool,
    Window,
    Winit,
}

impl DefaultPluginKind {
    pub const ALL: [Self; 6] = [
        Self::Audio,
        Self::Image,
        Self::Render,
        Self::TaskPool,
        Self::Window,
        Self::Winit,
    ];

    /// Backend-neutral key used by interpreter-local plugin metadata.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Audio => "Audio",
            Self::Image => "Image",
            Self::Render => "Render",
            Self::TaskPool => "TaskPool",
            Self::Window => "Window",
            Self::Winit => "Winit",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }

    /// Python-facing wrapper name used in diagnostics.
    pub const fn public_name(self) -> &'static str {
        match self {
            Self::Audio => "AudioPlugin",
            Self::Image => "ImagePlugin",
            Self::Render => "RenderPlugin",
            Self::TaskPool => "TaskPoolPlugin",
            Self::Window => "WindowPlugin",
            Self::Winit => "WinitPlugin",
        }
    }
}

/// Placement requested for a plugin added to a plugin group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginGroupPlacement<K> {
    /// Append after every plugin already in the group.
    End,
    /// Insert immediately before the identified target plugin.
    Before(K),
    /// Insert immediately after the identified target plugin.
    After(K),
}

/// One retained plugin-group addition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginGroupAddition<K, V> {
    pub placement: PluginGroupPlacement<K>,
    pub plugin: V,
}

impl<K, V> PluginGroupAddition<K, V> {
    pub fn new(placement: PluginGroupPlacement<K>, plugin: V) -> Self {
        Self { placement, plugin }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_target_identity_and_plugin_handle() {
        let addition = PluginGroupAddition::new(
            PluginGroupPlacement::Before(DefaultPluginKind::Render),
            41_u32,
        );
        assert_eq!(
            addition.placement,
            PluginGroupPlacement::Before(DefaultPluginKind::Render)
        );
        assert_eq!(addition.plugin, 41);
        assert_eq!(DefaultPluginKind::Render.public_name(), "RenderPlugin");
        assert_eq!(
            DefaultPluginKind::from_key("Render"),
            Some(DefaultPluginKind::Render)
        );
        assert_eq!(DefaultPluginKind::from_key("Unknown"), None);
    }
}
