"""Showcase rectangular area lights in a dark, reflective showroom.

The scene contains a row of colored panels, a spinning diamond panel, spheres
with different roughness values, and a glowing pillar lamp. An orbiting camera
and polished checkerboard floor make the area-light reflections easy to see.
"""

import math

from pybevy.prelude import *

PANEL_ROW = [
    (Color.srgb(1.0, 0.1, 0.55), -9.0),
    (Color.srgb(1.0, 0.1, 0.1), -6.5),
    (Color.srgb(1.0, 0.75, 0.1), -4.0),
    (Color.srgb(0.15, 0.25, 1.0), -1.5),
    (Color.srgb(0.1, 1.0, 0.3), 1.0),
]

ORBIT_CENTER = Vec3(1.0, 1.0, 0.5)
ORBIT_RADIUS = 15.0
ORBIT_HEIGHT = 6.0
ORBIT_SPEED = 0.15


@component
class SpinningPanel(Component):
    """Marker for light panels that slowly rotate."""


@component
class OrbitCamera(Component):
    """Marker for the camera that orbits the scene."""


def spawn_panel(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
    color: Color,
    width: float,
    height: float,
    transform: Transform,
    intensity: float,
    spinning: bool = False,
) -> None:
    """Spawn a rectangular light with a visible emissive panel."""
    panel_material = materials.add(
        StandardMaterial(
            base_color=color,
            emissive=color.to_linear() * 4.0,
            unlit=True,
            double_sided=True,
            cull_mode=None,
        )
    )
    light = commands.spawn(
        RectLight(
            color=color,
            intensity=intensity,
            width=width,
            height=height,
            range=30.0,
        ),
        transform,
    )
    if spinning:
        light.insert(SpinningPanel())
    light.with_children(
        lambda parent: parent.spawn(
            Mesh3d(meshes.add(Cuboid(width, height, 0.05))),
            MeshMaterial3d(panel_material),
        )
    )


def spawn_floor(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    """Spawn a checkerboard floor of polished metallic tiles."""
    tile = meshes.add(Plane3d().mesh().size(1.0, 1.0))
    dark = materials.add(
        StandardMaterial(
            base_color=Color.srgb(0.10, 0.10, 0.11),
            metallic=1.0,
            perceptual_roughness=0.1,
            reflectance=0.7,
        )
    )
    light = materials.add(
        StandardMaterial(
            base_color=Color.srgb(0.45, 0.45, 0.48),
            metallic=1.0,
            perceptual_roughness=0.15,
            reflectance=0.7,
        )
    )
    for ix in range(26):
        for iz in range(18):
            commands.spawn(
                Mesh3d(tile),
                MeshMaterial3d(dark if (ix + iz) % 2 == 0 else light),
                Transform.from_xyz(ix - 12.5, 0.0, iz - 8.5),
            )


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    commands.spawn(
        Camera3d(),
        Tonemapping.TONY_MC_MAPFACE,
        Bloom.NATURAL,
        Transform.from_xyz(
            ORBIT_CENTER.x,
            ORBIT_HEIGHT,
            ORBIT_CENTER.z + ORBIT_RADIUS,
        ).looking_at(ORBIT_CENTER, Vec3.Y),
        OrbitCamera(),
    )

    spawn_floor(commands, meshes, materials)

    for color, x in PANEL_ROW:
        spawn_panel(
            commands,
            meshes,
            materials,
            color,
            width=1.1,
            height=2.4,
            transform=Transform.from_xyz(x, 2.0, -4.0).looking_at(
                Vec3(x, 0.0, 2.0), Vec3.Y
            ),
            intensity=400_000.0,
        )

    diamond = Transform.from_xyz(4.5, 2.8, -3.0).looking_at(
        Vec3(3.0, 0.0, 1.0), Vec3.Y
    )
    diamond.rotate_local_z(math.pi / 4.0)
    spawn_panel(
        commands,
        meshes,
        materials,
        Color.srgb(1.0, 0.95, 0.55),
        width=1.7,
        height=1.7,
        transform=diamond,
        intensity=250_000.0,
        spinning=True,
    )
    commands.spawn(
        Mesh3d(meshes.add(Sphere(1.0))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(0.5, 0.48, 0.45),
                    perceptual_roughness=0.5,
                )
            )
        ),
        Transform.from_xyz(4.5, 1.0, -1.0),
    )

    spawn_panel(
        commands,
        meshes,
        materials,
        Color.srgb(0.35, 0.65, 1.0),
        width=3.2,
        height=1.3,
        transform=Transform.from_xyz(8.5, 1.8, 2.5).looking_at(
            Vec3(6.5, 0.0, 5.5), Vec3.Y
        ),
        intensity=350_000.0,
    )
    spheres = [
        (0.8, Vec3(6.0, 0.8, 5.5), Color.srgb(0.08, 0.07, 0.07), 0.9, 0.0),
        (0.7, Vec3(7.8, 0.7, 4.6), Color.srgb(0.9, 0.9, 0.88), 0.35, 0.0),
        (0.6, Vec3(7.0, 0.6, 6.6), Color.srgb(0.95, 0.95, 0.95), 0.05, 1.0),
    ]
    for radius, position, base_color, roughness, metallic in spheres:
        commands.spawn(
            Mesh3d(meshes.add(Sphere(radius))),
            MeshMaterial3d(
                materials.add(
                    StandardMaterial(
                        base_color=base_color,
                        perceptual_roughness=roughness,
                        metallic=metallic,
                    )
                )
            ),
            Transform.from_translation(position),
        )

    commands.spawn(
        Mesh3d(meshes.add(Cuboid(1.0, 0.25, 1.0))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(0.35, 0.33, 0.3),
                    perceptual_roughness=0.7,
                )
            )
        ),
        Transform.from_xyz(9.5, 0.125, -3.5),
    )
    commands.spawn(
        Mesh3d(meshes.add(Cuboid(0.3, 2.6, 0.3))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.WHITE,
                    emissive=LinearRgba.rgb(6.0, 5.5, 4.5),
                )
            )
        ),
        Transform.from_xyz(9.5, 1.55, -3.5),
    )
    commands.spawn(
        RectLight(
            color=Color.srgb(1.0, 0.9, 0.7),
            intensity=200_000.0,
            width=0.5,
            height=2.4,
            range=15.0,
        ),
        Transform.from_xyz(9.5, 1.55, -3.2).with_rotation(
            Quat.from_rotation_y(math.pi)
        ),
    )


def spin_panels(
    time: Res[Time],
    query: Query[Mut[Transform], With[SpinningPanel]],
) -> None:
    """Rotate marked panels so their reflections sweep across the floor."""
    for transform in query:
        transform.rotate_y(time.delta_secs() * 0.5)


def orbit_camera(
    time: Res[Time],
    query: Query[Mut[Transform], With[OrbitCamera]],
) -> None:
    """Move the camera in a slow circle around the showroom."""
    angle = time.elapsed_secs() * ORBIT_SPEED
    x = ORBIT_CENTER.x + ORBIT_RADIUS * math.sin(angle)
    z = ORBIT_CENTER.z + ORBIT_RADIUS * math.cos(angle)
    for transform in query:
        transform.translation = Vec3(x, ORBIT_HEIGHT, z)
        transform.look_at(ORBIT_CENTER, Vec3.Y)


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(ClearColor(Color.srgb(0.12, 0.12, 0.13)))
        .insert_resource(GlobalAmbientLight(brightness=25.0))
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (spin_panels, orbit_camera))
    )


if __name__ == "__main__":
    main().run()
