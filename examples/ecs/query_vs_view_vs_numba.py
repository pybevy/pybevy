"""
Minimal example demonstrating syntaxes for modifying components.

Shows different ways to update components:
1. Query iteration - Traditional ECS query with Mut[Transform]
2. View column API - Batch operations via view.column_mut()
3. View + Numba JIT - Zero-copy pointer access with compiled kernels
4. View cross-component - Expressions across multiple components
"""

import math

try:
    import numba  # type: ignore[import-untyped]
except ImportError:
    print("ERROR: Numba required for JIT example - install with: pip install numba")
    exit(1)

from dataclasses import dataclass

from pybevy import expr
from pybevy.prelude import *


@component
class QueryCube(Component):
    pass


@component
class ViewCube(Component):
    pass


@component
class JitCube(Component):
    pass


@component
class CrossCube(Component):
    pass


@component
@dataclass
class Energy(Component):
    value: float = 1.0


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    cube_mesh = meshes.add(Cuboid.from_length(1.0))

    commands.spawn(
        QueryCube(),
        Mesh3d(cube_mesh),
        MeshMaterial3d(materials.add(Color.srgb(1.0, 0.3, 0.3))),
        Transform.from_xyz(-3.0, 0.0, 0.0),
    )

    commands.spawn(
        ViewCube(),
        Mesh3d(cube_mesh),
        MeshMaterial3d(materials.add(Color.srgb(0.3, 1.0, 0.3))),
        Transform.from_xyz(0.0, 0.0, 0.0),
    )

    commands.spawn(
        JitCube(),
        Mesh3d(cube_mesh),
        MeshMaterial3d(materials.add(Color.srgb(0.3, 0.3, 1.0))),
        Transform.from_xyz(3.0, 0.0, 0.0),
    )

    commands.spawn(
        CrossCube(),
        Energy(value=2.0),
        Mesh3d(cube_mesh),
        MeshMaterial3d(materials.add(Color.srgb(1.0, 1.0, 0.3))),
        Transform.from_xyz(6.0, 0.0, 0.0),
        PointLight(intensity=0.0, range=15.0),
    )

    commands.spawn(
        DirectionalLight(illuminance=5000.0),
        Transform.IDENTITY.looking_at(Vec3(-1.0, -1.0, -1.0), Vec3.Y),
    )

    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 5.0, 10.0).looking_at(Vec3.ZERO, Vec3.Y),
    )


# Syntax 1: Query iteration - simple syntax suitable for small number of entities (<1000)
def update_query_cube(
    time: Res[Time], query: Query[Mut[Transform], With[QueryCube]]
) -> None:
    for transform in query:
        transform.rotate_y(time.delta_secs())


# Syntax 2: View column API - very fast vectorized access for larger numbers of entities - no complex logic
def update_view_cube(time: Res[Time], view: View[Mut[Transform], With[ViewCube]]) -> None:
    t = time.elapsed_secs()

    transform = view.column_mut(Transform)
    transform.translation.y = expr.sin(t * 2.0) * 2.0


# Syntax 3 helper: Numba JIT kernel for fast Transform updates
@numba.jit(nopython=True)
def fast_transform(translation, time: float) -> None:
    for i in range(len(translation.x)):
        translation.y[i] = math.sin(time + translation.x[i]) * 1.5


# Syntax 3: View + Numba JIT - compiled zero-copy access for maximum performance, complex logic, large numbers of entities
def update_jit_cube(time: Res[Time], view: View[Mut[Transform], With[JitCube]]) -> None:
    t = time.elapsed_secs()

    for batch in view.iter_batches():
        transform = batch.column_mut(Transform)
        fast_transform(transform.translation, t)


def update_cross_cube(
    time: Res[Time],
    view: View[tuple[Mut[Transform], Mut[PointLight], Energy], With[CrossCube]],
) -> None:
    t = time.elapsed_secs()
    pos = view.column_mut(Transform)
    light = view.column_mut(PointLight)
    energy = view.column(Energy)

    pos.translation.y = expr.sin(t * 1.5) * energy.value
    light.intensity = 500.0 + 300.0 * expr.sin(pos.translation.x + t) * energy.value


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            update_query_cube,
            update_view_cube,
            update_jit_cube,
            update_cross_cube,
        )
    )


if __name__ == "__main__":
    print("Query vs View vs Numba: red=query, green=view, blue=numba, yellow=cross-component.")
    main().run()
