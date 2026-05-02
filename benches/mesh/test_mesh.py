import numpy as np
from pytest_benchmark.fixture import BenchmarkFixture

from pybevy.app import App, MinimalPlugins, Startup, Update
from pybevy.assets import AssetPlugin, Assets
from pybevy.ecs import Commands, Query, ResMut
from pybevy.mesh import (
    Mesh,
    Mesh3d,
    MeshPlugin,
    PrimitiveTopology,
    VertexAttributeValues,
)

vertex_count = 10_000_000
indices_count = vertex_count * 3


def setup(meshes: ResMut[Assets[Mesh]], commands: Commands) -> None:
    mesh = (
        Mesh(PrimitiveTopology.TriangleList)
        .with_inserted_attribute(
            Mesh.ATTRIBUTE_POSITION,
            np.random.rand(vertex_count, 3).astype(np.float32),
        )
        .with_inserted_attribute(
            Mesh.ATTRIBUTE_NORMAL,
            np.random.rand(vertex_count, 3).astype(np.float32),
        )
        .with_inserted_attribute(
            Mesh.ATTRIBUTE_COLOR,
            np.random.rand(vertex_count, 4).astype(np.float32),
        )
        .with_inserted_indices(
            np.random.randint(0, vertex_count, size=(indices_count,), dtype=np.uint32)
        )
    )

    commands.spawn(
        Mesh3d(meshes.add(mesh)),
    )


def create_app() -> App:
    return (
        App()
        .add_plugins(MinimalPlugins, AssetPlugin, MeshPlugin)
        .add_systems(Startup, setup)
    )


def test_mesh_system_get_primitive_topology(benchmark: BenchmarkFixture) -> None:
    def update_system(assets: Assets[Mesh], query: Query[Mesh3d]) -> None:
        for mesh_3d in query:
            mesh = assets.get(mesh_3d.handle)
            assert mesh is not None
            assert mesh.primitive_topology() == PrimitiveTopology.TriangleList

    app = create_app().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_mesh_system_get_ref_vertices(benchmark: BenchmarkFixture) -> None:
    def update_system(assets: Assets[Mesh], query: Query[Mesh3d]) -> None:
        for mesh_3d in query:
            mesh = assets.get(mesh_3d.handle)
            assert mesh is not None

            with mesh.attribute(Mesh.ATTRIBUTE_POSITION) as v:
                assert v.shape == (vertex_count, 3)

    app = create_app().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_mesh_system_get_mut_vertices(benchmark: BenchmarkFixture) -> None:
    def update_system(assets: ResMut[Assets[Mesh]], query: Query[Mesh3d]) -> None:
        mesh_3d = query.single()
        assert mesh_3d is not None

        mesh = assets.get_mut(mesh_3d.handle)
        assert mesh is not None

        with mesh.attribute_mut(Mesh.ATTRIBUTE_POSITION) as v:
            assert v.shape == (vertex_count, 3)

    app = create_app().add_systems(Update, update_system)

    app.initialize()
    benchmark(app.update)


def test_mesh_vertex_attribute_values_new(benchmark: BenchmarkFixture) -> None:
    vertices = np.random.rand(vertex_count, 3).astype(np.float32)

    def do_bench() -> None:
        VertexAttributeValues(vertices)

    benchmark(do_bench)
