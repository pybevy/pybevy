"""Demonstrates how lighting is affected by different radius of point lights."""

from pybevy.prelude import *


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.2, 1.5, 2.5).looking_at(Vec3.ZERO, Vec3.Y),
    )

    # Plane
    commands.spawn(
        Mesh3d(meshes.add(Plane3d().mesh().size(100.0, 100.0))),
        MeshMaterial3d(
            materials.add(
                StandardMaterial(
                    base_color=Color.srgb(0.2, 0.2, 0.2),
                    perceptual_roughness=0.08,
                )
            )
        ),
    )

    COUNT = 6
    position_range_start = -2.0
    position_range_end = 2.0
    radius_range_start = 0.0
    radius_range_end = 0.4
    pos_len = position_range_end - position_range_start
    radius_len = radius_range_end - radius_range_start
    mesh = meshes.add(Sphere(1.0).mesh().uv(120, 64))

    for i in range(COUNT):
        percent = i / COUNT
        radius = radius_range_start + percent * radius_len

        # Sphere light
        light_radius = radius
        commands.spawn(
            Mesh3d(mesh),
            MeshMaterial3d(
                materials.add(
                    StandardMaterial(
                        base_color=Color.srgb(0.5, 0.5, 1.0),
                        unlit=True,
                    )
                )
            ),
            Transform.from_xyz(
                position_range_start + percent * pos_len, 0.3, 0.0
            ).with_scale(Vec3.splat(radius)),
        ).with_children(
            lambda parent, r=light_radius: parent.spawn(  # type: ignore[misc]
                PointLight(
                    radius=r,
                    color=Color.srgb(0.2, 0.2, 1.0),
                )
            )
        )


@entrypoint
def main(app: App) -> App:
    return (
        app.insert_resource(GlobalAmbientLight(brightness=60.0))
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
    )


if __name__ == "__main__":
    main().run()
