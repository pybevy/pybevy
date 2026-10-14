"""Animate WGSL structs and a separate u32 indirection buffer on a shared material."""

import math
from dataclasses import dataclass

from pybevy.material import U32, StorageBinding, shader_type
from pybevy.mesh import MeshTag
from pybevy.pbr import ShaderMaterialPlugin
from pybevy.prelude import *
from pybevy.render import ShaderBuffer

SHADER_PATH = "shaders/examples/structured_buffer.wgsl"
RECORD_COUNT = 7


@shader_type
class Tile:
    offset: Vec3
    scale: float
    color: Vec4


@material(vertex_shader=SHADER_PATH, fragment_shader=SHADER_PATH)
class StructuredTiles(Material):
    records: StorageBinding[Tile] = StorageBinding()
    indices: StorageBinding[U32] = StorageBinding()


@resource
@dataclass
class TileBuffers(Resource):
    records: Handle[ShaderBuffer]
    indices: Handle[ShaderBuffer]


def tiles(seconds: float) -> list[Tile]:
    return [
        Tile(
            offset=Vec3(0, 0, 0.25 * math.sin(seconds * 2 + index)),
            scale=0.75 + 0.2 * math.sin(seconds * 3 + index),
            color=Vec4(
                0.5 + 0.45 * math.sin(seconds + index),
                0.5 + 0.45 * math.sin(seconds + index + 2),
                0.5 + 0.45 * math.sin(seconds + index + 4),
                1,
            ),
        )
        for index in range(RECORD_COUNT)
    ]


def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StructuredTiles]],
    buffers: ResMut[Assets[ShaderBuffer]],
) -> None:
    records = buffers.add(ShaderBuffer(tiles(0)))
    indices = buffers.add(ShaderBuffer(range(RECORD_COUNT), element=U32))
    commands.insert_resource(TileBuffers(records, indices))
    surface = materials.add(StructuredTiles(records=records, indices=indices))  # type: ignore[call-arg]
    mesh = meshes.add(Cuboid(0.4, 0.4, 0.4))
    for index, (x, y) in enumerate((x, y) for x in range(-3, 4) for y in range(-2, 3)):
        commands.spawn(
            Mesh3d(mesh),
            MeshMaterial3d[StructuredTiles](surface),  # type: ignore[misc]
            MeshTag(index % RECORD_COUNT),
            Transform.from_xyz(x * 0.5, y * 0.5, 0),
        )  # type: ignore[misc]


def setup_camera(commands: Commands) -> None:
    commands.spawn(
        Camera3d(), Transform.from_xyz(0, 0, 6).looking_at(Vec3.ZERO, Vec3.Y)
    )


def animate(
    time: Res[Time], state: Res[TileBuffers], buffers: ResMut[Assets[ShaderBuffer]]
) -> None:
    records = buffers.get_mut(state.records)
    assert records is not None
    records.set_data(tiles(time.elapsed_secs()))
    indices = buffers.get_mut(state.indices)
    assert indices is not None
    shift = int(time.elapsed_secs()) % RECORD_COUNT
    indices.set_data(
        [(i + shift) % RECORD_COUNT for i in range(RECORD_COUNT)], element=U32
    )


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_plugins(ShaderMaterialPlugin())
        .add_systems(Startup, setup)
        .add_systems(Startup, setup_camera)
        .add_systems(Update, animate)
    )


if __name__ == "__main__":
    main().run()
