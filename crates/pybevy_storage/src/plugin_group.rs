//! Interpreter-neutral plugin-group operation metadata.

use std::{
    any::{TypeId, type_name},
    marker::PhantomData,
    sync::Mutex,
};

use bevy::app::{App, Plugin, PluginGroupBuilder};

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

/// Ordered member state shared by interpreter adapters for arbitrary groups.
#[derive(Clone, Debug)]
pub struct PluginGroupMembers<K, V> {
    members: Vec<PluginGroupMember<K, V>>,
}

#[derive(Clone, Debug)]
pub struct PluginGroupMember<K, V> {
    pub key: K,
    pub value: V,
    pub enabled: bool,
}

impl<K, V> Default for PluginGroupMembers<K, V> {
    fn default() -> Self {
        Self {
            members: Vec::new(),
        }
    }
}

impl<K: PartialEq, V> PluginGroupMembers<K, V> {
    pub fn map_values<W>(&self, mut map: impl FnMut(&V) -> W) -> PluginGroupMembers<K, W>
    where
        K: Clone,
    {
        PluginGroupMembers {
            members: self
                .members
                .iter()
                .map(|member| PluginGroupMember {
                    key: member.key.clone(),
                    value: map(&member.value),
                    enabled: member.enabled,
                })
                .collect(),
        }
    }

    pub fn append(&mut self, other: Self) {
        for member in other.members {
            let key = member.key;
            let enabled = member.enabled;
            let index = self.members.iter().position(|existing| existing.key == key);
            if let Some(index) = index {
                self.members.remove(index);
            }
            self.members.push(PluginGroupMember {
                key,
                value: member.value,
                enabled,
            });
        }
    }

    pub fn members(&self) -> &[PluginGroupMember<K, V>] {
        &self.members
    }

    pub fn contains(&self, key: &K) -> bool {
        self.members.iter().any(|member| &member.key == key)
    }

    pub fn enabled(&self, key: &K) -> bool {
        self.members
            .iter()
            .any(|member| &member.key == key && member.enabled)
    }

    pub fn set(&mut self, key: &K, value: V) -> Result<(), V> {
        match self.members.iter_mut().find(|member| &member.key == key) {
            Some(member) => {
                member.value = value;
                Ok(())
            }
            None => Err(value),
        }
    }

    pub fn set_enabled(&mut self, key: &K, enabled: bool) -> bool {
        match self.members.iter_mut().find(|member| &member.key == key) {
            Some(member) => {
                member.enabled = enabled;
                true
            }
            None => false,
        }
    }

    pub fn add(&mut self, key: K, value: V, placement: PluginGroupPlacement<K>) -> Result<(), V> {
        let index = match &placement {
            PluginGroupPlacement::End => self.members.len(),
            PluginGroupPlacement::Before(target) => {
                match self.members.iter().position(|member| &member.key == target) {
                    Some(index) => index,
                    None => return Err(value),
                }
            }
            PluginGroupPlacement::After(target) => {
                match self.members.iter().position(|member| &member.key == target) {
                    Some(index) => index + 1,
                    None => return Err(value),
                }
            }
        };
        self.members.insert(
            index,
            PluginGroupMember {
                key,
                value,
                enabled: true,
            },
        );
        let key = &self.members[index].key;
        let old = self
            .members
            .iter()
            .enumerate()
            .find(|(position, member)| *position != index && &member.key == key)
            .map(|(position, _)| position);
        if let Some(old) = old {
            self.members.remove(old);
        }
        Ok(())
    }
}

pub type NativeGroupCallback = Box<dyn FnOnce(&mut App) + Send + Sync>;

/// A one-shot native marker for an interpreter adapter's group application step.
pub struct PluginGroupCallback<T, const AFTER: bool> {
    callback: Mutex<Option<NativeGroupCallback>>,
    target: PhantomData<fn() -> T>,
}

impl<T, const AFTER: bool> PluginGroupCallback<T, AFTER> {
    pub fn new(callback: impl FnOnce(&mut App) + Send + Sync + 'static) -> Self {
        Self {
            callback: Mutex::new(Some(Box::new(callback))),
            target: PhantomData,
        }
    }
}

impl<T: 'static, const AFTER: bool> Plugin for PluginGroupCallback<T, AFTER> {
    fn build(&self, app: &mut App) {
        let callback = self
            .callback
            .lock()
            .expect("plugin group callback lock")
            .take();
        if let Some(callback) = callback {
            callback(app);
        }
    }

    fn is_unique(&self) -> bool {
        false
    }
}

/// Type-erased Bevy operations for a native group member.
#[derive(Clone, Copy)]
pub struct NativePluginSlot {
    pub type_id: TypeId,
    pub name: &'static str,
    contains: fn(&PluginGroupBuilder) -> bool,
    disable: fn(PluginGroupBuilder) -> PluginGroupBuilder,
    build: fn(&mut App),
    is_added: fn(&App) -> bool,
    place: fn(PluginGroupBuilder, NativeGroupCallback, bool) -> Result<PluginGroupBuilder, ()>,
}

impl NativePluginSlot {
    pub fn of<P: Plugin + Default>() -> Self {
        Self::with_build::<P>(|app| {
            app.add_plugins(P::default());
        })
    }

    pub fn with_build<P: Plugin>(build: fn(&mut App)) -> Self {
        Self {
            type_id: TypeId::of::<P>(),
            name: type_name::<P>(),
            contains: |group| group.contains::<P>(),
            disable: |group| group.disable::<P>(),
            build,
            is_added: |app| app.is_plugin_added::<P>(),
            place: |group, callback, after| {
                if after {
                    group
                        .try_add_after_overwrite::<P, _>(PluginGroupCallback::<P, true>::new(
                            callback,
                        ))
                        .map_err(|_| ())
                } else {
                    group
                        .try_add_before_overwrite::<P, _>(PluginGroupCallback::<P, false>::new(
                            callback,
                        ))
                        .map_err(|_| ())
                }
            },
        }
    }

    pub fn contains(self, builder: &PluginGroupBuilder) -> bool {
        (self.contains)(builder)
    }
    pub fn disable(self, builder: PluginGroupBuilder) -> PluginGroupBuilder {
        (self.disable)(builder)
    }
    pub fn build(self, app: &mut App) {
        if !(self.is_added)(app) {
            (self.build)(app);
        }
    }
    pub fn is_added(self, app: &App) -> bool {
        (self.is_added)(app)
    }
    pub fn place(
        self,
        builder: PluginGroupBuilder,
        callback: NativeGroupCallback,
        after: bool,
    ) -> Result<PluginGroupBuilder, ()> {
        (self.place)(builder, callback, after)
    }
    pub fn order_name(self) -> &'static str {
        self.name
            .split('<')
            .next()
            .unwrap()
            .rsplit("::")
            .next()
            .unwrap()
    }
}

/// Instantiate the public native slots in Bevy 0.19's default-group order.
#[macro_export]
macro_rules! default_plugin_slots {
    () => {{
        let mut slots = vec![
            $crate::NativePluginSlot::of::<bevy::app::PanicHandlerPlugin>(),
            $crate::NativePluginSlot::of::<bevy::log::LogPlugin>(),
            $crate::NativePluginSlot::of::<bevy::app::TaskPoolPlugin>(),
            $crate::NativePluginSlot::of::<bevy::diagnostic::FrameCountPlugin>(),
            $crate::NativePluginSlot::of::<bevy::time::TimePlugin>(),
            $crate::NativePluginSlot::of::<bevy::transform::TransformPlugin>(),
            $crate::NativePluginSlot::of::<bevy::diagnostic::DiagnosticsPlugin>(),
            $crate::NativePluginSlot::of::<bevy::input::InputPlugin>(),
            $crate::NativePluginSlot::of::<bevy::window::WindowPlugin>(),
            $crate::NativePluginSlot::of::<bevy::a11y::AccessibilityPlugin>(),
        ];
        #[cfg(any(all(unix, not(target_os = "horizon")), windows))]
        slots.push($crate::NativePluginSlot::of::<
            bevy::app::TerminalCtrlCHandlerPlugin,
        >());
        slots.extend([
            $crate::NativePluginSlot::of::<bevy::asset::AssetPlugin>(),
            $crate::NativePluginSlot::of::<bevy::world_serialization::WorldSerializationPlugin>(),
            $crate::NativePluginSlot::of::<bevy::winit::WinitPlugin>(),
            $crate::NativePluginSlot::of::<bevy::render::RenderPlugin>(),
            $crate::NativePluginSlot::of::<bevy::image::ImagePlugin>(),
            $crate::NativePluginSlot::of::<bevy::mesh::MeshPlugin>(),
            $crate::NativePluginSlot::of::<bevy::camera::CameraPlugin>(),
            $crate::NativePluginSlot::of::<bevy::light::LightPlugin>(),
        ]);
        #[cfg(not(target_arch = "wasm32"))]
        slots.push($crate::NativePluginSlot::of::<
            bevy::render::pipelined_rendering::PipelinedRenderingPlugin,
        >());
        slots.extend([
            $crate::NativePluginSlot::of::<bevy::core_pipeline::CorePipelinePlugin>(),
            $crate::NativePluginSlot::of::<bevy::post_process::PostProcessPlugin>(),
            $crate::NativePluginSlot::of::<bevy::sprite::SpritePlugin>(),
            $crate::NativePluginSlot::of::<bevy::sprite_render::SpriteRenderPlugin>(),
            $crate::NativePluginSlot::of::<bevy::text::TextPlugin>(),
            $crate::NativePluginSlot::of::<bevy::ui::UiPlugin>(),
            $crate::NativePluginSlot::of::<bevy::ui_render::UiRenderPlugin>(),
            $crate::NativePluginSlot::of::<bevy::gltf::GltfPlugin>(),
            $crate::NativePluginSlot::of::<bevy::pbr::PbrPlugin>(),
            $crate::NativePluginSlot::of::<bevy::audio::AudioPlugin>(),
            $crate::NativePluginSlot::of::<bevy::animation::AnimationPlugin>(),
            $crate::NativePluginSlot::of::<bevy::gizmos::GizmoPlugin>(),
            $crate::NativePluginSlot::of::<bevy::gizmos_render::GizmoRenderPlugin>(),
            $crate::NativePluginSlot::of::<bevy::state::app::StatesPlugin>(),
            $crate::NativePluginSlot::with_build::<bevy::picking::input::PointerInputPlugin>(
                |app| {
                    app.add_plugins(bevy::picking::input::PointerInputPlugin);
                },
            ),
            $crate::NativePluginSlot::with_build::<bevy::picking::PickingPlugin>(|app| {
                app.add_plugins(bevy::picking::PickingPlugin);
            }),
            $crate::NativePluginSlot::of::<bevy::picking::InteractionPlugin>(),
        ]);
        slots
    }};
}
