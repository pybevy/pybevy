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
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use std::collections::HashSet;

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

    #[test]
    fn default_plugin_kinds_keep_their_literal_keys_and_public_names() {
        // The literal table is the independent oracle: deriving the expected
        // name from the actual key would let a coordinated rename pass.
        const KINDS: [(DefaultPluginKind, &str, &str); 6] = [
            (DefaultPluginKind::Audio, "Audio", "AudioPlugin"),
            (DefaultPluginKind::Image, "Image", "ImagePlugin"),
            (DefaultPluginKind::Render, "Render", "RenderPlugin"),
            (DefaultPluginKind::TaskPool, "TaskPool", "TaskPoolPlugin"),
            (DefaultPluginKind::Window, "Window", "WindowPlugin"),
            (DefaultPluginKind::Winit, "Winit", "WinitPlugin"),
        ];

        let all: HashSet<DefaultPluginKind> = DefaultPluginKind::ALL.iter().copied().collect();
        assert_eq!(DefaultPluginKind::ALL.len(), KINDS.len());
        assert_eq!(all.len(), KINDS.len());

        for (kind, key, name) in KINDS {
            assert!(all.contains(&kind));
            assert_eq!(kind.key(), key);
            assert_eq!(kind.public_name(), name);
            assert_eq!(DefaultPluginKind::from_key(key), Some(kind));
            // Public names are diagnostic-only and never lookup keys.
            assert_eq!(DefaultPluginKind::from_key(name), None);
        }
        assert_eq!(DefaultPluginKind::from_key("Unknown"), None);
    }

    #[test]
    fn placement_variants_retain_target_and_payload() {
        let end: PluginGroupAddition<DefaultPluginKind, u8> =
            PluginGroupAddition::new(PluginGroupPlacement::End, 1_u8);
        let after: PluginGroupAddition<DefaultPluginKind, u8> =
            PluginGroupAddition::new(PluginGroupPlacement::After(DefaultPluginKind::Window), 2_u8);
        let before: PluginGroupAddition<DefaultPluginKind, u8> =
            PluginGroupAddition::new(PluginGroupPlacement::Before(DefaultPluginKind::Audio), 3_u8);

        assert_eq!(end.placement, PluginGroupPlacement::End);
        assert_eq!(
            after.placement,
            PluginGroupPlacement::After(DefaultPluginKind::Window)
        );
        assert_eq!(
            before.placement,
            PluginGroupPlacement::Before(DefaultPluginKind::Audio)
        );
        assert_eq!(end.plugin, 1);
        assert_eq!(after.plugin, 2);
        assert_eq!(before.plugin, 3);
        assert_ne!(
            PluginGroupPlacement::End,
            PluginGroupPlacement::Before(DefaultPluginKind::Audio)
        );
    }
}
