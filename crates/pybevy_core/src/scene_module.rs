use bevy::prelude::Resource;

/// Importable identity of the scene module currently driving an app.
///
/// This interpreter-neutral resource stores only a name. Interpreter adapters
/// resolve that name on demand, so hot reload can replace the live module
/// without retaining a stale interpreter object in the Bevy World.
#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct ActiveSceneModule {
    name: String,
}

impl ActiveSceneModule {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use bevy::ecs::world::World;

    use super::ActiveSceneModule;

    #[test]
    fn active_scene_module_round_trips_through_a_world_resource() {
        let names = ["pybevy.scene.game", "", "scene.m\u{20ac}de"];

        for name in names {
            let mut world = World::new();
            assert!(
                world.get_resource::<ActiveSceneModule>().is_none(),
                "a fresh world carries no active scene module"
            );

            world.insert_resource(ActiveSceneModule::new(name));

            let identity = world
                .get_resource::<ActiveSceneModule>()
                .expect("the module identity resource must be inserted");
            assert_eq!(
                identity.name(),
                name,
                "the importable name must round-trip exactly"
            );
        }
    }

    #[test]
    fn active_scene_module_clone_and_equality_are_exact() {
        let first = ActiveSceneModule::new("pybevy.scene.first");
        let second = ActiveSceneModule::new("pybevy.scene.second");
        let clone = first.clone();

        assert_eq!(clone.name(), first.name());
        assert_eq!(first, clone);
        assert_ne!(
            first, second,
            "distinct importable identities must not compare equal"
        );
    }
}
