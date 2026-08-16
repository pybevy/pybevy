"""Illustrates rectangular area lights and surface roughness.

The Rust original uses a fly-around camera; PyBevy does not expose that
controller yet, so this port uses a fixed camera. Arrow Up/Down adjusts the
floor roughness, and G toggles the light gizmos.
"""

import math

from pybevy.prelude import *

CONTROLS_TEXT = (
    "Controls\n"
    "Arrow Up/Down: Adjust floor roughness\n"
    "G: Toggle light gizmos\n"
    "\n"
    "Roughness: {:.2f}"
)


@resource
class FloorMaterial(Resource):
    """Handle to the floor material adjusted at runtime."""

    def __init__(self, handle: Handle[StandardMaterial]) -> None:
        self.handle = handle


@component
class RoughnessDisplay(Component):
    """Marker for the roughness text display."""


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Spawn a sphere and reflective floor lit by two rectangular lights."""
    floor_material = materials.add(
        StandardMaterial(
            base_color=Color.WHITE,
            metallic=1.0,
            perceptual_roughness=0.6,
        )
    )
    commands.insert_resource(FloorMaterial(floor_material))
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(20.0, 20.0))),
        MeshMaterial3d(floor_material),
    )

    commands.spawn(
        Mesh3d(meshes.add(Sphere(1.0))),
        MeshMaterial3d(materials.add(StandardMaterial(base_color=Color.WHITE))),
        Transform.from_xyz(0.0, 1.0, 0.0),
    )

    commands.spawn(
        RectLight(
            color=Color.srgb(1.0, 0.3, 0.2),
            intensity=100_000.0,
            width=2.0,
            height=1.0,
            range=20.0,
        ),
        ShowLightGizmo(),
        Transform.from_xyz(1.0, 3.0, 1.0).looking_at(Vec3.Y, Vec3.Y),
    )

    commands.spawn(
        RectLight(
            color=Color.srgb(0.5, 0.7, 1.0),
            intensity=800_000.0,
            width=1.5,
            height=4.0,
            range=20.0,
        ),
        ShowLightGizmo(),
        Transform.from_xyz(-2.0, 1.5, -3.0).with_rotation(
            Quat.from_rotation_y(math.pi)
        ),
    )

    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-8.0, 5.0, 8.0).looking_at(Vec3.Y, Vec3.Y),
    )

    node = Node()
    node.position_type = PositionType.Absolute
    node.top = Val.px(12.0)
    node.left = Val.px(12.0)
    commands.spawn(
        Text(CONTROLS_TEXT.format(0.6)),
        TextFont(font_size=18.0),
        TextColor(Color.srgb(0.9, 0.9, 0.9)),
        node,
        RoughnessDisplay(),
    )


def adjust_roughness(
    keyboard: Res[ButtonInput],
    floor: Res[FloorMaterial],
    materials: ResMut[Assets[StandardMaterial]],
    text_query: Query[Mut[Text], With[RoughnessDisplay]],
) -> None:
    """Adjust the floor material and update the on-screen value."""
    if keyboard.pressed(KeyCode.ArrowUp):
        delta = 0.005
    elif keyboard.pressed(KeyCode.ArrowDown):
        delta = -0.005
    else:
        return

    material = materials.get_mut(floor.handle)
    if material is None:
        return

    material.perceptual_roughness = min(
        1.0, max(0.0, material.perceptual_roughness + delta)
    )
    for text in text_query:
        text.content = CONTROLS_TEXT.format(material.perceptual_roughness)


def toggle_gizmos(
    keyboard: Res[ButtonInput],
    config_store: ResMut[GizmoConfigStore],
) -> None:
    """Toggle Bevy's light gizmo config group."""
    if keyboard.just_pressed(KeyCode.KeyG):
        config, _light_config = config_store.config_mut(LightGizmoConfigGroup)
        config.enabled = not config.enabled


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (toggle_gizmos, adjust_roughness))
    )


if __name__ == "__main__":
    main().run()
