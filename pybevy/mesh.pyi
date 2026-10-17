from collections.abc import Callable, Iterable, Iterator, Sequence
from types import TracebackType
from typing import ClassVar, Literal

import numpy as np

from pybevy import array as xp
from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle, RenderAssetUsages
from pybevy.collections import LiveArray, LiveList
from pybevy.ecs import Batchable, Component, Entity
from pybevy.math import Dir3, Mat4, Quat, Range, Vec2, Vec3
from pybevy.render import VertexFormat
from pybevy.transform import Transform

class MeshPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class MeshBuilder:
    def __init__(self) -> None: ...
    def build(self) -> Mesh: ...

class Meshable:
    def __init__(self) -> None: ...
    def mesh(self) -> MeshBuilder: ...

class MeshBoundedContextMut:
    """Context manager yielding an in-place mutable bounded array over a mesh
    attribute. Writes land directly in the mesh; the array is closed on exit."""
    def __enter__(self) -> xp.Array:
        """Enter the context and return the mutable bounded array."""

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> bool:
        """Exit the context: close the array and release the write-lock."""

class Mesh(Asset):
    def __copy__(self) -> Mesh: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Mesh: ...
    ATTRIBUTE_POSITION: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_NORMAL: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_UV_0: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_UV_1: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_TANGENT: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_COLOR: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_JOINT_WEIGHT: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_JOINT_INDEX: ClassVar[MeshVertexAttribute]

    def __init__(self, primitive_topology: PrimitiveTopology, asset_usage: RenderAssetUsages = ...) -> None: ...
    def primitive_topology(self) -> PrimitiveTopology: ...
    @property
    def asset_usage(self) -> RenderAssetUsages:
        """The asset usage flags, borrowed from the mesh.

        In-place mutation writes through to a mesh fetched with `get_mut`, and
        raises on a read-only or Python-constructed mesh. Whole-field assignment
        requires an owned mesh or mutable asset access.
        """
    @asset_usage.setter
    def asset_usage(self, value: RenderAssetUsages) -> None: ...
    @property
    def enable_raytracing(self) -> bool: ...
    @enable_raytracing.setter
    def enable_raytracing(self, value: bool) -> None: ...
    def get_vertex_buffer_size(self) -> int: ...
    def count_vertices(self) -> int:
        """Effective vertex count. With differing per-vertex attribute row
        counts, the shortest attribute's row count is returned; stored
        attribute buffers keep their full row counts, and only the packed
        vertex buffer and the rendered mesh contain the first
        `count_vertices()` rows of each attribute."""
    def create_packed_vertex_buffer_data(self) -> bytes:
        """GPU-friendly interleaved vertex bytes: the first `count_vertices()`
        rows of each attribute."""

    def compute_custom_smooth_normals(
        self,
        per_triangle: Callable[
            [
                tuple[int, int, int],
                tuple[tuple[float, float, float], ...],
                list[tuple[float, float, float]],
            ],
            None,
        ],
    ) -> None:
        """Smooth normals with a per-triangle callback adding to the normal sums.

        Every call receives the same positions tuple and the same normals list.
        """
    def skinned_mesh_bounds(self) -> SkinnedMeshBounds | None:
        """This mesh's skinned bounds as a read-only view, or None.

        A stored mesh's view follows the asset and expires with the system;
        replace the bounds with `set_skinned_mesh_bounds`."""

    def set_skinned_mesh_bounds(
        self, skinned_mesh_bounds: SkinnedMeshBounds | None
    ) -> None: ...
    def with_skinned_mesh_bounds(
        self, skinned_mesh_bounds: SkinnedMeshBounds | None
    ) -> Mesh: ...
    def generate_skinned_mesh_bounds(self) -> None:
        """Compute the bounds from the position, joint index and joint weight
        attributes; raises ValueError for missing or incompatible attributes
        or an out-of-range weighted vertex. All-zero weights produce empty bounds."""

    def with_generated_skinned_mesh_bounds(self) -> Mesh: ...
    def contains_attribute(self, attribute: MeshVertexAttribute) -> bool: ...
    def remove_attribute(
        self, attribute: MeshVertexAttribute
    ) -> VertexAttributeValues | None: ...
    def with_removed_attribute(self, attribute: MeshVertexAttribute) -> Mesh: ...
    def with_inserted_attribute(
        self,
        attribute: MeshVertexAttribute,
        values: VertexAttributeValues
        | np.ndarray
        | xp.Array
        | Sequence[float]
        | Sequence[Sequence[float]],
    ) -> Mesh: ...
    def insert_attribute(
        self,
        attribute: MeshVertexAttribute,
        values: VertexAttributeValues
        | np.ndarray
        | xp.Array
        | Sequence[float]
        | Sequence[Sequence[float]],
    ) -> None: ...
    def with_inserted_indices(
        self, indices: Indices | np.ndarray | xp.Array | list[int]
    ) -> Mesh: ...
    def insert_indices(
        self, indices: Indices | np.ndarray | xp.Array | list[int]
    ) -> None: ...
    def remove_indices(self) -> Indices | None: ...
    def with_removed_indices(self) -> Mesh: ...
    def merge_duplicate_vertices(self) -> None: ...
    def with_merge_duplicate_vertices(self) -> Mesh: ...
    def normalize_joint_weights(self) -> None: ...

    def indices(self) -> xp.Array | None:
        """The index buffer as a read-only `uint16`/`uint32` bounded array, or
        None if the mesh is non-indexed.

        A live zero-copy view: the mesh cannot be mutated while it is alive,
        writes through it are refused, and it expires with its owner. Call
        `.tolist()` or `.copy()` for an independent snapshot, and write an
        edited buffer back with `insert_indices`.
        """

    def get_index_buffer_bytes(self) -> xp.Array | None:
        """The index buffer's raw bytes as a read-only `uint8` view, or None if
        the mesh is non-indexed.

        Two bytes per index for `uint16` indices, four for `uint32`, in native
        byte order. The view is borrowed like `indices()`.
        """

    def positions(self) -> xp.Array:
        """Read-only zero-copy bounded array of vertex positions, shape (N, 3).

        Returns the portable ``pybevy.array.Array`` type on both
        backends. The array borrows the mesh data: mutating the mesh is blocked
        while it is alive, and access after the owning system ends raises. Call
        ``.to_numpy()`` (or ``.copy()``) for an independent snapshot.

        Example:
            ```python
            pos = mesh.positions()
            mean_pos = pos.mean(axis=0)
            snapshot = pos.to_numpy()  # detached NumPy copy
            ```
        """

    def positions_mut(self) -> MeshBoundedContextMut:
        """In-place mutable bounded array of positions via a context manager.

        Writes land directly in the mesh (zero-copy); the array is closed on
        exit.

        Example:
            ```python
            with mesh.positions_mut() as pos:
                pos[:, 1] += 1.0  # Move all vertices up by 1
            ```
        """

    def normals(self) -> xp.Array:
        """Read-only zero-copy bounded array of vertex normals, shape (N, 3)."""

    def normals_mut(self) -> MeshBoundedContextMut:
        """In-place mutable bounded array of normals via a context manager."""

    def uvs(self) -> xp.Array:
        """Read-only zero-copy bounded array of UV coordinates, shape (N, 2)."""

    def uvs_mut(self) -> MeshBoundedContextMut:
        """In-place mutable bounded array of UVs via a context manager."""

    def attribute(self, id: MeshVertexAttribute) -> xp.Array | None:
        """Read-only zero-copy bounded array of any float32 vertex attribute."""

    def attribute_mut(self, id: MeshVertexAttribute) -> MeshBoundedContextMut:
        """In-place mutable bounded array of any float32 attribute."""

    def set_positions(self, positions: np.typing.ArrayLike | xp.Array) -> None:
        """Copy vertex positions from an (N, 3) array-like (numpy array, bounded
        pybevy.array array, or nested list). Each row must have 3 lanes.
        N may differ from other attributes' row counts. Readback keeps all N
        rows; `count_vertices()` and rendering use the shortest attribute."""

    def set_normals(self, normals: np.typing.ArrayLike | xp.Array) -> None:
        """Copy vertex normals from an (N, 3) array-like (numpy array, bounded
        pybevy.array array, or nested list). Each row must have 3 lanes.
        N may differ from other attributes' row counts. Readback keeps all N
        rows; `count_vertices()` and rendering use the shortest attribute."""

    def with_generated_tangents(self) -> Mesh: ...
    def generate_tangents(self) -> None: ...
    def compute_normals(self) -> None: ...
    def compute_flat_normals(self) -> None: ...
    def compute_smooth_normals(self) -> None: ...
    def compute_area_weighted_normals(self) -> None: ...
    def with_computed_normals(self) -> Mesh: ...
    def with_computed_flat_normals(self) -> Mesh: ...
    def with_computed_smooth_normals(self) -> Mesh: ...
    def with_computed_area_weighted_normals(self) -> Mesh: ...
    def duplicate_vertices(self) -> None: ...
    def with_duplicated_vertices(self) -> Mesh: ...
    def invert_winding(self) -> None: ...
    def with_inverted_winding(self) -> Mesh: ...
    def merge(self, other: Mesh) -> None:
        """Append a mesh with matching topology, vertex attributes, and index mode.

        Raises ValueError if either mesh has inconsistent attribute lengths,
        their attribute sets differ, or only one mesh has indices.
        """
    def transform_by(self, transform: Transform) -> None: ...
    def transformed_by(self, transform: Transform) -> Mesh: ...
    def translate_by(self, translation: Vec3) -> None: ...
    def translated_by(self, translation: Vec3) -> Mesh: ...
    def rotate_by(self, rotation: Quat) -> None: ...
    def rotated_by(self, rotation: Quat) -> Mesh: ...
    def scale_by(self, scale: Vec3) -> None: ...
    def scaled_by(self, scale: Vec3) -> Mesh: ...
    def has_morph_targets(self) -> bool: ...

class PrimitiveTopology:

    def __copy__(self) -> PrimitiveTopology: ...
    def __deepcopy__(self, memo: dict[int, object]) -> PrimitiveTopology: ...

    PointList: ClassVar[PrimitiveTopology]
    LineList: ClassVar[PrimitiveTopology]
    LineStrip: ClassVar[PrimitiveTopology]
    TriangleList: ClassVar[PrimitiveTopology]
    TriangleStrip: ClassVar[PrimitiveTopology]

class UvChannel:
    """UV channel selection for texture mapping."""

    def __copy__(self) -> UvChannel: ...
    def __deepcopy__(self, memo: dict[int, object]) -> UvChannel: ...


    Uv0: ClassVar[UvChannel]
    Uv1: ClassVar[UvChannel]

class CapsuleUvProfile:
    Aspect: ClassVar[CapsuleUvProfile]
    Uniform: ClassVar[CapsuleUvProfile]
    Fixed: ClassVar[CapsuleUvProfile]

class CircularMeshUvMode:
    def __copy__(self) -> CircularMeshUvMode: ...
    def __deepcopy__(self, memo: dict[int, object]) -> CircularMeshUvMode: ...
    class Mask(CircularMeshUvMode):
        __match_args__: ClassVar[tuple[Literal["angle"]]]
        @property
        def angle(self) -> float: ...
        def __init__(self, *, angle: float) -> None: ...

class MeshVertexAttributeId:
    """Opaque attribute id; obtained from MeshVertexAttribute.id, not constructed."""

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class MeshVertexAttribute:
    def __init__(self, name: str, id: int, format: VertexFormat) -> None:
        """Build a custom attribute; use a large id to avoid built-in collisions."""

    @property
    def name(self) -> str: ...
    @property
    def id(self) -> MeshVertexAttributeId: ...
    @property
    def format(self) -> VertexFormat: ...
    def __eq__(self, other: object) -> bool: ...

class Indices:
    """A mesh index buffer, stored as `U16` or `U32` indices.

    Unlike Bevy, `push` and `extend` never change the variant of an existing
    value: adding an index of 65535 or more to `Indices.U16` raises
    `OverflowError` and leaves it unchanged. Build `Indices.U32` for such
    indices.
    """

    class U16(Indices):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: np.ndarray | xp.Array | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...

    class U32(Indices):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: np.ndarray | xp.Array | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...

    def len(self) -> int: ...
    def is_empty(self) -> bool: ...
    def push(self, index: int) -> None: ...
    def extend(self, values: Sequence[int]) -> None: ...
    def iter(self) -> list[int]: ...
    def __iter__(self) -> IndicesIterator: ...
    def __eq__(self, other: object) -> bool: ...
    def __len__(self) -> int: ...
    def __copy__(self) -> Indices: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Indices: ...

class IndicesIterator:
    def __iter__(self) -> IndicesIterator: ...
    def __next__(self) -> int: ...

class VertexAttributeValues:
    def __copy__(self) -> VertexAttributeValues: ...
    def __deepcopy__(self, memo: dict[int, object]) -> VertexAttributeValues: ...
    def len(self) -> int: ...
    def is_empty(self) -> bool: ...
    def __len__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    class Uint8(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Uint8x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Uint8x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint8(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint8x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint8x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Unorm8(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Unorm8x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Unorm8x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Snorm8(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Snorm8x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Snorm8x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Uint16(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Uint16x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Uint16x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint16(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint16x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint16x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Unorm16(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Unorm16x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Unorm16x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Snorm16(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Snorm16x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Snorm16x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float16(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[float]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float16x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[float]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float16x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[float]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float32(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[float]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float32x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[float]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float32x3(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[float]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float32x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[float]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Uint32(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Uint32x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Uint32x3(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Uint32x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint32(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint32x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint32x3(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Sint32x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float64(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[float]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float64x2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[float]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float64x3(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[float]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Float64x4(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[float]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Unorm10_10_10_2(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[int]) -> None: ...
        @property
        def value(self) -> xp.Array: ...
    class Unorm8x4Bgra(VertexAttributeValues):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        def __init__(self, value: xp.Array | np.ndarray | Sequence[Sequence[int]]) -> None: ...
        @property
        def value(self) -> xp.Array: ...

class CylinderAnchor:
    MidPoint: ClassVar[CylinderAnchor]
    Top: ClassVar[CylinderAnchor]
    Bottom: ClassVar[CylinderAnchor]
    def __copy__(self) -> CylinderAnchor: ...
    def __deepcopy__(self, memo: dict[int, object]) -> CylinderAnchor: ...

class ConeAnchor:
    MidPoint: ClassVar[ConeAnchor]
    Tip: ClassVar[ConeAnchor]
    Base: ClassVar[ConeAnchor]
    def __copy__(self) -> ConeAnchor: ...
    def __deepcopy__(self, memo: dict[int, object]) -> ConeAnchor: ...

class CylinderMeshBuilder(MeshBuilder):
    def resolution(self, resolution: int) -> CylinderMeshBuilder: ...
    def segments(self, segments: int) -> CylinderMeshBuilder: ...
    def without_caps(self) -> CylinderMeshBuilder: ...
    def anchor(self, anchor: CylinderAnchor) -> CylinderMeshBuilder: ...
    def build(self) -> Mesh: ...

class Polyline2dMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...

class Polyline3dMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...

class ConeMeshBuilder(MeshBuilder):
    def resolution(self, resolution: int) -> ConeMeshBuilder: ...
    def anchor(self, anchor: ConeAnchor) -> ConeMeshBuilder: ...
    def build(self) -> Mesh:
        """Build the cone mesh.

        The apex vertex carries a zero-length normal, because a cone has no
        single well-defined normal there and bevy declines to pick one. A shader
        that normalizes it gets NaN. `mesh.compute_normals()` after building
        replaces it with the averaged face normal.
        """

class ConicalFrustumMeshBuilder(MeshBuilder):
    def resolution(self, resolution: int) -> ConicalFrustumMeshBuilder: ...
    def segments(self, segments: int) -> ConicalFrustumMeshBuilder: ...
    def build(self) -> Mesh: ...

class CuboidMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...

class RectangleMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...

class SphereMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...
    def kind(self, kind: SphereKind) -> SphereMeshBuilder: ...
    def ico(self, subdivisions: int) -> Mesh: ...
    def uv(self, sectors: int, stacks: int) -> Mesh: ...

class PlaneMeshBuilder(MeshBuilder):
    """Raises ValueError for a negative, NaN, or infinite size; zero is allowed.

    Subdivision counts outside 0 to 4294967295 raise ValueError.
    """

    def __init__(self, normal: Dir3 = ..., size: Vec2 = ...) -> None: ...
    @staticmethod
    def from_size(size: Vec2) -> PlaneMeshBuilder: ...
    @staticmethod
    def from_length(length: float) -> PlaneMeshBuilder: ...
    def normal(self, normal: Dir3) -> PlaneMeshBuilder: ...
    def size(self, width: float, height: float) -> PlaneMeshBuilder: ...
    def subdivisions(self, subdivisions: int) -> PlaneMeshBuilder: ...
    def subdivisions_x(self, subdivisions: int) -> PlaneMeshBuilder: ...
    def subdivisions_z(self, subdivisions: int) -> PlaneMeshBuilder: ...
    def build(self) -> Mesh: ...

class CircleMeshBuilder(MeshBuilder):
    def resolution(self, resolution: int) -> CircleMeshBuilder: ...
    def build(self) -> Mesh: ...

class TorusMeshBuilder(MeshBuilder):
    """Mesh builder for Torus primitive."""
    def minor_resolution(self, resolution: int) -> TorusMeshBuilder: ...
    def major_resolution(self, resolution: int) -> TorusMeshBuilder: ...
    def angle_range(self, range: Range) -> TorusMeshBuilder: ...
    def build(self) -> Mesh: ...

class Capsule3dMeshBuilder(MeshBuilder):
    """Mesh builder for 3D Capsule primitive.

    Ring counts outside 0 to 4294967295 raise ValueError.
    """
    def rings(self, rings: int) -> Capsule3dMeshBuilder: ...
    def longitudes(self, longitudes: int) -> Capsule3dMeshBuilder: ...
    def latitudes(self, latitudes: int) -> Capsule3dMeshBuilder: ...
    def uv_profile(self, profile: CapsuleUvProfile) -> Capsule3dMeshBuilder: ...
    def build(self) -> Mesh: ...

class TetrahedronMeshBuilder(MeshBuilder):
    """Mesh builder for Tetrahedron primitive."""
    def build(self) -> Mesh: ...

class Triangle3dMeshBuilder(MeshBuilder):
    """Mesh builder for 3D Triangle primitive."""
    def build(self) -> Mesh: ...

class AnnulusMeshBuilder(MeshBuilder):
    """Mesh builder for Annulus (ring) primitive."""
    def resolution(self, resolution: int) -> AnnulusMeshBuilder: ...
    def build(self) -> Mesh: ...

class Capsule2dMeshBuilder(MeshBuilder):
    """Mesh builder for 2D Capsule primitive."""
    def resolution(self, resolution: int) -> Capsule2dMeshBuilder: ...
    def build(self) -> Mesh: ...

class Triangle2dMeshBuilder(MeshBuilder):
    """Mesh builder for 2D Triangle primitive."""
    def build(self) -> Mesh: ...

class EllipseMeshBuilder(MeshBuilder):
    """Mesh builder for Ellipse primitive."""
    def resolution(self, resolution: int) -> EllipseMeshBuilder: ...
    def build(self) -> Mesh: ...

class RegularPolygonMeshBuilder(MeshBuilder):
    """Mesh builder for RegularPolygon primitive."""
    def build(self) -> Mesh: ...

class RhombusMeshBuilder(MeshBuilder):
    """Mesh builder for Rhombus primitive."""
    def build(self) -> Mesh: ...

class CircularSectorMeshBuilder(MeshBuilder):
    """Mesh builder for CircularSector primitive."""
    def resolution(self, resolution: int) -> CircularSectorMeshBuilder: ...
    def uv_mode(self, mode: CircularMeshUvMode) -> CircularSectorMeshBuilder: ...
    def build(self) -> Mesh: ...

class CircularSegmentMeshBuilder(MeshBuilder):
    """Mesh builder for CircularSegment primitive."""
    def resolution(self, resolution: int) -> CircularSegmentMeshBuilder: ...
    def uv_mode(self, mode: CircularMeshUvMode) -> CircularSegmentMeshBuilder: ...
    def build(self) -> Mesh: ...

class Segment2dMeshBuilder(MeshBuilder):
    """Mesh builder for Segment2d primitive."""
    def build(self) -> Mesh: ...

class SphereKind:

    def __copy__(self) -> SphereKind: ...
    def __deepcopy__(self, memo: dict[int, object]) -> SphereKind: ...

    class Ico(SphereKind):
        __match_args__: ClassVar[tuple[Literal["subdivisions"]]]
        @property
        def subdivisions(self) -> int: ...

        def __init__(self, *, subdivisions: int) -> None:
            """Icosphere mesh kind with the given number of subdivisions."""

    class Uv(SphereKind):
        __match_args__: ClassVar[tuple[Literal["sectors"], Literal["stacks"]]]
        @property
        def sectors(self) -> int: ...
        @property
        def stacks(self) -> int: ...

        def __init__(self, *, sectors: int, stacks: int) -> None:
            """UV sphere mesh kind with the given number of sectors and stacks."""

class Mesh2d(Component):
    """2D mesh component for sprite rendering."""
    def __init__(self, mesh: Handle[Mesh] = ...) -> None: ...
    @property
    def handle(self) -> Handle[Mesh]: ...

class Mesh3d(Component):
    """3D mesh component."""
    def __init__(self, mesh: Handle[Mesh] = ...) -> None: ...
    @property
    def handle(self) -> Handle[Mesh]: ...

class MeshTag(Component):
    """A component that stores an arbitrary index used to identify the mesh instance when rendering."""
    def __init__(self, value: int = 0) -> None: ...
    @property
    def value(self) -> int: ...
    @value.setter
    def value(self, value: int) -> None: ...
    @staticmethod
    def batch(*, value: np.typing.ArrayLike | None = None) -> Batchable: ...  # type: ignore[override]
    def __eq__(self, other: MeshTag) -> bool: ...  # type: ignore[override]

class MorphWeights(Component):
    """Controls the morph targets for all child Mesh3d entities.

    This component serves as the "source of truth" for morph target weights.
    It synchronizes with child MeshMorphWeights components, following the GLTF spec
    architecture where multi-primitive meshes are decomposed into individual Bevy surfaces.

    Args:
        weights: List of morph target weights (float values typically 0.0-1.0).
        first_mesh: Optional handle to the first child mesh, useful for accessing
            metadata like morph target names.
    """
    def __init__(self, weights: list[float] = ..., first_mesh: Handle[Mesh] | None = None) -> None: ...
    @property
    def first_mesh(self) -> Handle[Mesh] | None: ...
    @property
    def weights(self) -> LiveArray[float]:
        """Fixed-length live view of the weights.

        Through `Query[Mut[MorphWeights]]`, `mw.weights[i] = w` writes the
        component and marks it changed; through `Query[MorphWeights]` the view
        is read-only; on an owned value it is a read-only snapshot, so assign
        the whole list instead. The view expires with its system and cannot
        change the number of weights.
        """
    @weights.setter
    def weights(self, weights: list[float]) -> None: ...

class MeshMorphWeights(Component):
    """Controls morph targets for one Mesh3d entity."""

    def __hash__(self) -> int: ...

    class Value(MeshMorphWeights):
        __match_args__: ClassVar[tuple[Literal["weights"]]]
        def __init__(self, *, weights: Iterable[float]) -> None: ...
        @property
        def weights(self) -> LiveList[float]:
            """Live list of the weights.

            Through `Query[Mut[MeshMorphWeights]]`, `w.weights[i] = x` and list
            edits such as `append` write the component and mark it changed;
            through `Query[MeshMorphWeights]` the list is read-only; on an owned
            value it is a read-only snapshot, so assign the whole list instead.
            The list expires with its system and raises once the component
            changes variant.
            """
        @weights.setter
        def weights(self, weights: Iterable[float]) -> None: ...

    class Reference(MeshMorphWeights):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: Entity
        def __init__(self, value: Entity) -> None: ...

    def __copy__(self) -> MeshMorphWeights: ...
    def __deepcopy__(self, memo: dict[int, object]) -> MeshMorphWeights: ...

class SkinnedMeshInverseBindposes(Asset):
    """Inverse bind pose matrices for skeletal mesh animation.

    This asset contains the inverse bind pose matrices for each joint in a skeleton.
    Used with SkinnedMesh component for skeletal animation.

    Args:
        matrices: List of Mat4 inverse bind pose matrices, one per joint.
    """
    def __init__(self, matrices: list[Mat4]) -> None: ...
    def get(self, index: int) -> Mat4 | None: ...
    def __getitem__(self, index: int) -> Mat4: ...
    def __iter__(self) -> Iterator[Mat4]: ...
    def __len__(self) -> int: ...

class JointIndex:
    """An index into `Mesh.ATTRIBUTE_JOINT_INDEX` and `SkinnedMesh.joints`."""

    def __init__(self, value: int) -> None: ...
    @property
    def value(self) -> int: ...
    @value.setter
    def value(self, value: int) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class JointAabb:
    """The model-space box around the vertices skinned to one joint."""

    def __init__(self, *, center: Vec3, half_size: Vec3) -> None: ...
    @property
    def center(self) -> Vec3: ...
    @center.setter
    def center(self, value: Vec3) -> None: ...
    @property
    def half_size(self) -> Vec3: ...
    @half_size.setter
    def half_size(self, value: Vec3) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class SkinnedMeshBounds:
    """Per-joint boxes from which Bevy computes a skinned mesh's AABB.

    `aabbs[i]` encloses the vertices skinned to joint
    `aabb_index_to_joint_index[i]`; joints with no skinned vertices have no box.
    """

    def __init__(
        self,
        *,
        aabbs: list[JointAabb] = ...,
        aabb_index_to_joint_index: list[JointIndex] = ...,
    ) -> None: ...
    @staticmethod
    def from_mesh(mesh: Mesh) -> SkinnedMeshBounds:
        """Compute the bounds from a mesh's position, joint index and joint
        weight attributes; raises ValueError like
        `Mesh.generate_skinned_mesh_bounds`."""

    @property
    def aabbs(self) -> LiveList[JointAabb]: ...
    @aabbs.setter
    def aabbs(self, value: list[JointAabb]) -> None: ...
    @property
    def aabb_index_to_joint_index(self) -> LiveList[JointIndex]: ...
    @aabb_index_to_joint_index.setter
    def aabb_index_to_joint_index(self, value: list[JointIndex]) -> None: ...
    def iter(self) -> Iterator[tuple[JointIndex, JointAabb]]:
        """Each joint index paired with its box, borrowed like the fields."""

    def __eq__(self, other: object) -> bool: ...

class SkinnedMesh(Component):
    """Component that defines a skinned mesh for skeletal animation.

    A skinned mesh deforms its vertices based on a hierarchy of bone entities (joints).
    Each vertex is influenced by one or more bones, weighted by the inverse bind pose matrices.

    This component is typically created automatically when loading GLTF files with skeletal
    animations, but can also be constructed manually for procedural skinned meshes.
    The mesh must contain ``Mesh.ATTRIBUTE_JOINT_INDEX`` and
    ``Mesh.ATTRIBUTE_JOINT_WEIGHT`` for the skinned render pipeline.
    """

    def __init__(
        self,
        *,
        inverse_bindposes: Handle[SkinnedMeshInverseBindposes] = ...,
        joints: list[Entity] = ...,
    ) -> None:
        """Create a new SkinnedMesh component.

        Args:
            inverse_bindposes: Handle to SkinnedMeshInverseBindposes asset containing
                the inverse bind pose matrices for each joint
            joints: List of Entity references representing the bone hierarchy
        """

    @property
    def inverse_bindposes(self) -> Handle[SkinnedMeshInverseBindposes]:
        """Get the handle to the inverse bind pose matrices asset."""

    @inverse_bindposes.setter
    def inverse_bindposes(self, value: Handle[SkinnedMeshInverseBindposes]) -> None: ...

    @property
    def joints(self) -> LiveList[Entity]:
        """The joint (bone) entities.

        Through `Query[Mut[SkinnedMesh]]` list edits write the component; through
        `Query[SkinnedMesh]` and on an owned value the list is read-only, so assign a
        whole list there. The list expires with its system.
        """

    @joints.setter
    def joints(self, value: list[Entity]) -> None: ...
