use std::any::TypeId;

use bevy::{
    asset::{Assets, RenderAssetUsages},
    camera::{Camera, RenderTarget},
    ecs::{entity::Entity, world::World},
    gizmos::config::{DefaultGizmoConfigGroup, GizmoConfig, GizmoConfigStore},
    image::Image,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use super::hold::GizmoEnabledRestore;

pub(crate) fn world_with_gizmos() -> World {
    let mut world = World::new();
    let mut store = GizmoConfigStore::default();
    store.insert(GizmoConfig::default(), DefaultGizmoConfigGroup);
    world.insert_resource(store);
    world
}

pub(crate) fn gizmos_enabled(world: &World) -> bool {
    let store = world.resource::<GizmoConfigStore>();
    let (config, _) = store.config::<DefaultGizmoConfigGroup>();
    config.enabled
}

pub(crate) fn default_group_restore(was_enabled: bool) -> GizmoEnabledRestore {
    vec![(TypeId::of::<DefaultGizmoConfigGroup>(), was_enabled)]
}

pub(crate) fn make_test_image(width: u32, height: u32) -> Image {
    let size = Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };
    let data = vec![255u8; (width * height * 4) as usize];
    Image::new(
        size,
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

pub(crate) fn spawn_structurally_valid_headless_camera(world: &mut World) -> Entity {
    let mut images = Assets::<Image>::default();
    let target = images.add(make_test_image(1, 1));
    world
        .spawn((Camera::default(), RenderTarget::Image(target.into())))
        .id()
}
