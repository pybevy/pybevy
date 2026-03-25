"""Illustrates different lights of various types and colors."""

import math

from pybevy.prelude import *


@component
class Movable(Component):
    """Marker for objects that can be moved with arrow keys"""


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
):
    # Ground plane
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(10.0, 10.0).build())),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.WHITE,
                    perceptual_roughness=1.0,
                )
            )
        ),
    )

    # Left wall
    commands.spawn(
        Mesh3d(meshes.add(Cuboid.from_size(Vec3(5.0, 0.15, 5.0)))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(0.3, 0.0, 0.5),  # Indigo-ish
                    perceptual_roughness=1.0,
                )
            )
        ),
        Transform.from_xyz(2.5, 2.5, 0.0).with_rotation(
            Quat.from_rotation_z(math.pi / 2.0)
        ),
    )

    # Back wall
    commands.spawn(
        Mesh3d(meshes.add(Cuboid.from_size(Vec3(5.0, 0.15, 5.0)))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(0.3, 0.0, 0.5),  # Indigo-ish
                    perceptual_roughness=1.0,
                )
            )
        ),
        Transform.from_xyz(0.0, 2.5, -2.5).with_rotation(
            Quat.from_rotation_x(math.pi / 2.0)
        ),
    )

    # Cube (movable)
    commands.spawn(
        Mesh3d(meshes.add(Cuboid())),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(1.0, 0.08, 0.58),  # Deep pink
                )
            )
        ),
        Transform.from_xyz(0.0, 0.5, 0.0),
        Movable(),
    )

    # Sphere (movable)
    commands.spawn(
        Mesh3d(meshes.add(Sphere(0.5))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(0.2, 1.0, 0.2),  # Lime green
                )
            )
        ),
        Transform.from_xyz(1.5, 1.0, 1.5),
        Movable(),
    )

    # Global ambient light
    commands.insert_resource(
        GlobalAmbientLight(
            color=Color.srgb(1.0, 0.27, 0.0),  # Orange-red
            brightness=200.0,
        )
    )

    # Red point light with emissive sphere indicator
    _red_light = commands.spawn(
        PointLight(
            intensity=100_000.0,
            color=Color.srgb(1.0, 0.0, 0.0),
            shadows_enabled=True,
        ),
        Transform.from_xyz(1.0, 2.0, 0.0),
    )
    commands.spawn(
        Mesh3d(meshes.add(Sphere(0.1))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(1.0, 0.0, 0.0),
                    emissive=LinearRgba.rgb(4.0, 0.0, 0.0),
                )
            )
        ),
        Transform.from_xyz(1.0, 2.0, 0.0),
    )

    # Green spot light with emissive indicator
    commands.spawn(
        SpotLight(
            intensity=100_000.0,
            color=Color.srgb(0.0, 1.0, 0.0),
            shadows_enabled=True,
            inner_angle=0.6,
            outer_angle=0.8,
        ),
        Transform.from_xyz(-1.0, 2.0, 0.0).looking_at(Vec3(-1.0, 0.0, 0.0), Vec3.Z),
    )
    commands.spawn(
        Mesh3d(meshes.add(Sphere(0.1))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(0.0, 1.0, 0.0),
                    emissive=LinearRgba.rgb(0.0, 4.0, 0.0),
                )
            )
        ),
        Transform.from_xyz(-1.0, 2.0, 0.0),
    )

    # Blue point light with emissive sphere indicator
    commands.spawn(
        PointLight(
            intensity=100_000.0,
            color=Color.srgb(0.0, 0.0, 1.0),
            shadows_enabled=True,
        ),
        Transform.from_xyz(0.0, 4.0, 0.0),
    )
    commands.spawn(
        Mesh3d(meshes.add(Sphere(0.1))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(0.0, 0.0, 1.0),
                    emissive=LinearRgba.rgb(0.0, 0.0, 713.0),
                )
            )
        ),
        Transform.from_xyz(0.0, 4.0, 0.0),
    )

    # Directional 'sun' light
    commands.spawn(
        DirectionalLight(
            illuminance=500.0,  # Roughly overcast day
            shadows_enabled=True,
        ),
        Transform.from_xyz(0.0, 2.0, 0.0).with_rotation(
            Quat.from_rotation_x(-math.pi / 4.0)
        ),
    )

    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


def animate_light_direction(
    time: Res[Time], query: Query[Mut[Transform], With[DirectionalLight]]
):
    """Slowly rotate the directional light"""
    for transform in query:
        transform.rotate_y(time.delta_secs() * 0.5)


def movement(input: Res[ButtonInput], time: Res[Time], query: Query[Mut[Transform], With[Movable]]):
    """Move objects with arrow keys"""
    for transform in query:
        direction = Vec3.ZERO

        if input.pressed(KeyCode.ArrowUp):
            direction.y += 1.0
        if input.pressed(KeyCode.ArrowDown):
            direction.y -= 1.0
        if input.pressed(KeyCode.ArrowLeft):
            direction.x -= 1.0
        if input.pressed(KeyCode.ArrowRight):
            direction.x += 1.0

        transform.translation += direction * time.delta_secs() * 2.0


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(
            Update, (animate_light_direction, movement)
        )
    )


if __name__ == "__main__":
    main().run()
