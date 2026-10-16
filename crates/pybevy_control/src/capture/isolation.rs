use std::{any::TypeId, collections::HashSet};

use bevy::{
    camera::visibility::{RenderLayers, VisibilityClass},
    ecs::world::World,
    gizmos::config::GizmoConfigStore,
    light::cluster::ClusterVisibilityClass,
    prelude::*,
};

use crate::{
    protocol::{ControlError, EntityRef},
    scene::entity::resolve_entity,
};

const ENTITY_CAPTURE_LAYER: usize = 63;

/// Wait for visibility to reflect the isolated render layers before capture.
pub(super) const ENTITY_ISOLATION_SETTLE_FRAMES: u32 = 4;

#[derive(Resource)]
pub struct EntityCaptureIsolationActive;

pub struct EntityCaptureIsolation {
    original_layers: Vec<(Entity, Option<RenderLayers>)>,
    gizmo_configs: Vec<(TypeId, bool, RenderLayers)>,
    pub(super) scope: HashSet<Entity>,
}

impl EntityCaptureIsolation {
    pub(super) fn restore_world(self, world: &mut World) {
        for (entity, original) in self.original_layers {
            let Ok(mut entity_mut) = world.get_entity_mut(entity) else {
                continue;
            };
            if let Some(layers) = original {
                entity_mut.insert(layers);
            } else {
                entity_mut.remove::<RenderLayers>();
            }
        }
        if let Some(mut store) = world.get_resource_mut::<GizmoConfigStore>() {
            for (group, enabled, layers) in self.gizmo_configs {
                if let Some((config, _)) = store.get_config_mut_dyn(&group) {
                    config.enabled = enabled;
                    config.render_layers = layers;
                }
            }
        }
        world.remove_resource::<EntityCaptureIsolationActive>();
    }
}

fn collect_entity_capture_scope(world: &World, root: Entity) -> HashSet<Entity> {
    let mut scope = HashSet::from([root]);
    let mut pending = vec![root];
    while let Some(entity) = pending.pop() {
        let Some(children) = world.get::<Children>(entity) else {
            continue;
        };
        for child in children.iter() {
            if scope.insert(child) {
                pending.push(child);
            }
        }
    }
    scope
}

pub(super) fn begin_entity_capture_isolation(
    world: &mut World,
    entity_ref: &EntityRef,
    with_gizmos: bool,
) -> Result<EntityCaptureIsolation, ControlError> {
    let root = resolve_entity(world, entity_ref)?;
    let scope = collect_entity_capture_scope(world, root);
    let support_class = TypeId::of::<ClusterVisibilityClass>();
    let snapshots = {
        let mut query = world.query::<(
            Entity,
            Option<&RenderLayers>,
            Has<Camera>,
            Option<&VisibilityClass>,
        )>();
        query
            .iter(world)
            .map(|(entity, layers, is_camera, visibility_class)| {
                let is_support =
                    visibility_class.is_some_and(|classes| classes.contains(&support_class));
                (entity, layers.cloned(), is_camera, is_support)
            })
            .collect::<Vec<_>>()
    };

    let mut original_layers = Vec::new();
    for (entity, original, is_camera, is_support) in snapshots {
        let included = scope.contains(&entity) || is_camera || is_support;
        let replacement = if included {
            Some(RenderLayers::layer(ENTITY_CAPTURE_LAYER))
        } else {
            original
                .as_ref()
                .filter(|layers| layers.iter().any(|layer| layer == ENTITY_CAPTURE_LAYER))
                .cloned()
                .map(|layers| layers.without(ENTITY_CAPTURE_LAYER))
        };
        let Some(replacement) = replacement else {
            continue;
        };
        if original.as_ref() == Some(&replacement) {
            continue;
        }
        world.entity_mut(entity).insert(replacement);
        original_layers.push((entity, original));
    }
    let mut gizmo_configs = Vec::new();
    if with_gizmos && let Some(mut store) = world.get_resource_mut::<GizmoConfigStore>() {
        for (group, config, _) in store.iter_mut() {
            gizmo_configs.push((*group, config.enabled, config.render_layers.clone()));
            config.enabled = true;
            config.render_layers = RenderLayers::layer(ENTITY_CAPTURE_LAYER);
        }
    }
    world.insert_resource(EntityCaptureIsolationActive);
    Ok(EntityCaptureIsolation {
        original_layers,
        gizmo_configs,
        scope,
    })
}

#[cfg(test)]
mod tests {
    use std::any::TypeId;

    use bevy::{
        camera::visibility::RenderLayers, gizmos::config::DefaultGizmoConfigGroup, ui::Node,
    };

    use super::*;
    use crate::capture::{hold::hide_ui_nodes, test_support::world_with_gizmos};

    #[test]
    fn entity_capture_isolates_subtree_camera_and_lighting_then_restores() {
        let mut world = World::new();
        let root = world
            .spawn((Name::new("Target"), RenderLayers::layer(2)))
            .id();
        let child = world.spawn(ChildOf(root)).id();
        let camera = world.spawn(Camera::default()).id();
        let mut light_class = VisibilityClass::default();
        light_class.push(TypeId::of::<ClusterVisibilityClass>());
        let light = world.spawn(light_class).id();
        let unrelated = world
            .spawn(RenderLayers::layer(1).with(ENTITY_CAPTURE_LAYER).with(7))
            .id();
        let default_unrelated = world.spawn_empty().id();

        let isolation = begin_entity_capture_isolation(
            &mut world,
            &EntityRef::Name("Target".to_string()),
            false,
        )
        .unwrap();

        for entity in [root, child, camera, light] {
            assert_eq!(
                world.get::<RenderLayers>(entity),
                Some(&RenderLayers::layer(ENTITY_CAPTURE_LAYER))
            );
        }
        assert_eq!(
            world
                .get::<RenderLayers>(unrelated)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec![1, 7]
        );
        assert!(world.get::<RenderLayers>(default_unrelated).is_none());
        assert!(world.contains_resource::<EntityCaptureIsolationActive>());

        isolation.restore_world(&mut world);

        assert_eq!(
            world.get::<RenderLayers>(root),
            Some(&RenderLayers::layer(2))
        );
        assert!(world.get::<RenderLayers>(child).is_none());
        assert!(world.get::<RenderLayers>(camera).is_none());
        assert!(world.get::<RenderLayers>(light).is_none());
        assert_eq!(
            world
                .get::<RenderLayers>(unrelated)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
            vec![1, 7, ENTITY_CAPTURE_LAYER]
        );
        assert!(!world.contains_resource::<EntityCaptureIsolationActive>());
    }

    #[test]
    fn entity_capture_keeps_targeted_ui_visible_when_hiding_authored_ui() {
        let mut world = World::new();
        let root = world
            .spawn((Name::new("TargetUi"), Visibility::Visible, Node::default()))
            .id();
        let child = world
            .spawn((ChildOf(root), Visibility::Visible, Node::default()))
            .id();
        let unrelated = world.spawn((Visibility::Visible, Node::default())).id();
        let isolation = begin_entity_capture_isolation(
            &mut world,
            &EntityRef::Name("TargetUi".to_string()),
            false,
        )
        .unwrap();

        let hidden = hide_ui_nodes(&mut world, Some(&isolation.scope));

        assert_eq!(*world.get::<Visibility>(root).unwrap(), Visibility::Visible);
        assert_eq!(
            *world.get::<Visibility>(child).unwrap(),
            Visibility::Visible
        );
        assert_eq!(
            *world.get::<Visibility>(unrelated).unwrap(),
            Visibility::Hidden
        );
        assert_eq!(hidden, vec![(unrelated, Visibility::Visible)]);
        isolation.restore_world(&mut world);
    }

    #[test]
    fn missing_entity_capture_does_not_change_render_layers() {
        let mut world = World::new();
        let camera = world
            .spawn((Camera::default(), RenderLayers::layer(4)))
            .id();

        let result = begin_entity_capture_isolation(
            &mut world,
            &EntityRef::Name("Missing".to_string()),
            false,
        );

        assert!(result.is_err());
        assert_eq!(
            world.get::<RenderLayers>(camera),
            Some(&RenderLayers::layer(4))
        );
        assert!(!world.contains_resource::<EntityCaptureIsolationActive>());
    }

    #[test]
    fn entity_capture_with_gizmos_restores_group_render_layers() {
        let mut world = world_with_gizmos();
        world.spawn(Name::new("Target"));
        {
            let mut store = world.resource_mut::<GizmoConfigStore>();
            let (config, _) = store.config_mut::<DefaultGizmoConfigGroup>();
            config.enabled = false;
            config.render_layers = RenderLayers::layer(5);
        }

        let isolation = begin_entity_capture_isolation(
            &mut world,
            &EntityRef::Name("Target".to_string()),
            true,
        )
        .unwrap();
        {
            let store = world.resource::<GizmoConfigStore>();
            let config = store.config::<DefaultGizmoConfigGroup>().0;
            assert!(config.enabled);
            assert_eq!(
                config.render_layers,
                RenderLayers::layer(ENTITY_CAPTURE_LAYER)
            );
        }

        isolation.restore_world(&mut world);
        let store = world.resource::<GizmoConfigStore>();
        let config = store.config::<DefaultGizmoConfigGroup>().0;
        assert!(!config.enabled);
        assert_eq!(config.render_layers, RenderLayers::layer(5));
    }
}
