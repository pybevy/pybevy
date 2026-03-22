"""Demonstrates 2D shape primitives with Circle and Rectangle.

This example shows how to:
- Create 2D shape primitives (Circle, Rectangle)
- Generate meshes from primitives
- Use Mesh2d and ColorMaterial for 2D rendering
- Position shapes in 2D space

Controls:
- Close window to exit
"""

from pybevy.app import App, DefaultPlugins
from pybevy.assets import Assets
from pybevy.camera import Camera2d
from pybevy.color import Color
from pybevy.decorators import entrypoint
from pybevy.ecs import Commands
from pybevy.math import Circle, Rectangle, Vec3
from pybevy.mesh import Mesh, Mesh2d, MeshMaterial2d
from pybevy.prelude import ResMut, Startup, Transform
from pybevy.sprite import ColorMaterial


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[ColorMaterial]],
) -> None:
    """Spawn camera and 2D shapes."""
    # Camera
    commands.spawn(Camera2d())

    # Circle (red) - positioned at (-150, 0)
    circle = Circle(radius=50.0)
    circle_mesh = meshes.add(circle.mesh().build())
    circle_material = materials.add(ColorMaterial(color=Color.srgb(1.0, 0.0, 0.0)))

    commands.spawn(
        Mesh2d(circle_mesh),
        MeshMaterial2d(circle_material),
        Transform(translation=Vec3(-150.0, 0.0, 0.0)),
    )

    # Rectangle (green) - positioned at (150, 0)
    rectangle = Rectangle(width=100.0, height=80.0)
    rect_mesh = meshes.add(rectangle.mesh().build())
    rect_material = materials.add(ColorMaterial(color=Color.srgb(0.0, 1.0, 0.0)))

    commands.spawn(
        Mesh2d(rect_mesh),
        MeshMaterial2d(rect_material),
        Transform(translation=Vec3(150.0, 0.0, 0.0)),
    )

    # Small blue circle at origin
    small_circle = Circle(radius=25.0)
    small_mesh = meshes.add(small_circle.mesh().build())
    small_material = materials.add(ColorMaterial(color=Color.srgb(0.0, 0.0, 1.0)))

    commands.spawn(
        Mesh2d(small_mesh),
        MeshMaterial2d(small_material),
        Transform(translation=Vec3(0.0, 0.0, 0.0)),
    )


@entrypoint
def main(app: App) -> App:
    """Entry point for 2D shapes example."""
    return app.add_plugins(DefaultPlugins).add_systems(Startup, setup)


if __name__ == "__main__":
    main().run()
