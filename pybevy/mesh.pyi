from collections.abc import Iterator
from enum import Enum
from types import TracebackType
from typing import ClassVar, Literal

import numpy as np

from pybevy import array as xp
from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.ecs import Batchable, Component, Entity
from pybevy.image import RenderAssetUsages
from pybevy.math import Dir3, Quat, Vec2, Vec3
from pybevy.pbr import StandardMaterial
from pybevy.sprite import ColorMaterial
from pybevy.transform import Transform
from pybevy.wgpu import VertexFormat

class MeshPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class MeshBuilder:
    def build(self) -> Mesh: ...

class Meshable:
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
    ATTRIBUTE_POSITION: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_NORMAL: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_UV_0: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_UV_1: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_TANGENT: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_COLOR: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_JOINT_WEIGHT: ClassVar[MeshVertexAttribute]
    ATTRIBUTE_JOINT_INDEX: ClassVar[MeshVertexAttribute]

    def __init__(self, primitive_topology: PrimitiveTopology) -> None: ...
    def primitive_topology(self) -> PrimitiveTopology: ...
    @property
    def asset_usage(self) -> RenderAssetUsages: ...
    @property
    def enable_raytracing(self) -> bool: ...
    def get_vertex_buffer_size(self) -> int: ...
    def count_vertices(self) -> int: ...
    def contains_attribute(self, attribute: MeshVertexAttribute) -> bool: ...
    def remove_attribute(
        self, attribute: MeshVertexAttribute
    ) -> VertexAttributeValues | None: ...
    def with_removed_attribute(self, attribute: MeshVertexAttribute) -> Mesh: ...
    def with_inserted_attribute(
        self,
        attribute: MeshVertexAttribute,
        values: VertexAttributeValues | np.ndarray | xp.Array,
    ) -> Mesh: ...
    def insert_attribute(
        self,
        attribute: MeshVertexAttribute,
        values: VertexAttributeValues | np.ndarray | xp.Array,
    ) -> None: ...
    def with_inserted_indices(
        self, indices: Indices | np.ndarray | xp.Array | list[int]
    ) -> Mesh: ...
    def insert_indices(
        self, indices: Indices | np.ndarray | xp.Array | list[int]
    ) -> None: ...

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
                pos[:, 2] += 1.0  # Move all vertices up by 1
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

    def attribute(self, id: MeshVertexAttribute) -> xp.Array:
        """Read-only zero-copy bounded array of any float32 vertex attribute."""

    def attribute_mut(self, id: MeshVertexAttribute) -> MeshBoundedContextMut:
        """In-place mutable bounded array of any float32 attribute."""

    def set_positions(self, positions: np.typing.ArrayLike | xp.Array) -> None:
        """Copy vertex positions from an (N, 3) array-like (numpy array, bounded
        pybevy.array array, or nested list)."""

    def set_normals(self, normals: np.typing.ArrayLike | xp.Array) -> None:
        """Copy vertex normals from an (N, 3) array-like (numpy array, bounded
        pybevy.array array, or nested list)."""

    def with_generated_tangents(self) -> Mesh: ...
    def generate_tangents(self) -> None: ...
    def compute_normals(self) -> None: ...
    def compute_flat_normals(self) -> None: ...
    def compute_smooth_normals(self) -> None: ...
    def with_computed_normals(self) -> Mesh: ...
    def with_computed_flat_normals(self) -> Mesh: ...
    def with_computed_smooth_normals(self) -> Mesh: ...
    def duplicate_vertices(self) -> None: ...
    def with_duplicated_vertices(self) -> Mesh: ...
    def invert_winding(self) -> None: ...
    def with_inverted_winding(self) -> Mesh: ...
    def merge(self, other: Mesh) -> None: ...
    def transform_by(self, transform: Transform) -> None: ...
    def transformed_by(self, transform: Transform) -> Mesh: ...
    def translate_by(self, translation: Vec3) -> None: ...
    def translated_by(self, translation: Vec3) -> Mesh: ...
    def rotate_by(self, rotation: Quat) -> None: ...
    def rotated_by(self, rotation: Quat) -> Mesh: ...
    def scale_by(self, scale: Vec3) -> None: ...
    def scaled_by(self, scale: Vec3) -> Mesh: ...
    def has_morph_targets(self) -> bool: ...

class PrimitiveTopology(Enum):
    PointList = ...
    LineList = ...
    LineStrip = ...
    TriangleList = ...
    TriangleStrip = ...

class UvChannel:
    """UV channel selection for texture mapping."""

    Uv0: ClassVar[UvChannel]
    Uv1: ClassVar[UvChannel]

    def __init__(self) -> None: ...

class MeshVertexAttribute:
    @property
    def name(self) -> str: ...
    @property
    def id(self) -> int: ...
    @property
    def format(self) -> VertexFormat: ...
    def __eq__(self, other: object) -> bool: ...

class Indices:
    def __init__(self, obj: np.ndarray | xp.Array | list[int]) -> None: ...
    @property
    def len(self) -> int: ...
    def is_empty(self) -> bool: ...
    def __iter__(self) -> Iterator[IndicesIterator]: ...
    def __eq__(self, other: object) -> bool: ...

class IndicesIterator:
    def __next__(self) -> int: ...

class VertexAttributeValues:
    def __init__(self, obj: np.ndarray | xp.Array) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class CylinderMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...

class ConeMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...

class CuboidMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...

class RectangleMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...

class SphereMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...
    def kind(self, kind: SphereKind | None = None) -> SphereKind | "SphereMeshBuilder":
        """No argument: return the current kind. With a kind: return a NEW
        builder with the kind replaced (Bevy's chaining-builder parity)."""
    def ico(self, subdivisions: int) -> Mesh: ...
    def uv(self, sectors: int, stacks: int) -> Mesh: ...

class PlaneMeshBuilder(MeshBuilder):
    @staticmethod
    def new(normal: Dir3, size: Vec2) -> PlaneMeshBuilder: ...
    @staticmethod
    def from_size(size: Vec2) -> PlaneMeshBuilder: ...
    @staticmethod
    def from_length(length: float) -> PlaneMeshBuilder: ...
    def normal(self, normal: Dir3) -> PlaneMeshBuilder: ...
    def size(self, width: float, height: float) -> PlaneMeshBuilder: ...
    def subdivisions(self, subdivisions: int) -> PlaneMeshBuilder: ...
    def build(self) -> Mesh: ...

class CircleMeshBuilder(MeshBuilder):
    def build(self) -> Mesh: ...

class TorusMeshBuilder(MeshBuilder):
    """Mesh builder for Torus primitive."""
    def minor_resolution(self, resolution: int) -> TorusMeshBuilder: ...
    def major_resolution(self, resolution: int) -> TorusMeshBuilder: ...
    def build(self) -> Mesh: ...

class Capsule3dMeshBuilder(MeshBuilder):
    """Mesh builder for 3D Capsule primitive."""
    def rings(self, rings: int) -> Capsule3dMeshBuilder: ...
    def longitudes(self, longitudes: int) -> Capsule3dMeshBuilder: ...
    def latitudes(self, latitudes: int) -> Capsule3dMeshBuilder: ...
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
    def build(self) -> Mesh: ...

class CircularSegmentMeshBuilder(MeshBuilder):
    """Mesh builder for CircularSegment primitive."""
    def resolution(self, resolution: int) -> CircularSegmentMeshBuilder: ...
    def build(self) -> Mesh: ...

class Segment2dMeshBuilder(MeshBuilder):
    """Mesh builder for Segment2d primitive."""
    def build(self) -> Mesh: ...

class SphereKind:
    class Ico(SphereKind):
        __match_args__: ClassVar[tuple[Literal["subdivisions"]]]
        subdivisions: int

        def __init__(self, subdivisions: int) -> None: ...
        """Icosphere mesh kind with the given number of subdivisions."""

    class Uv(SphereKind):
        __match_args__: ClassVar[tuple[Literal["sectors"], Literal["stacks"]]]
        sectors: int
        stacks: int

        def __init__(self, sectors: int, stacks: int) -> None: ...
        """UV sphere mesh kind with the given number of sectors and stacks."""

class Mesh2d(Component):
    """2D mesh component for sprite rendering."""
    def __init__(self, mesh: Handle[Mesh]) -> None: ...
    @property
    def handle(self) -> Handle[Mesh]: ...

class Mesh3d(Component):
    """3D mesh component."""
    def __init__(self, mesh: Handle[Mesh]) -> None: ...
    @property
    def handle(self) -> Handle[Mesh]: ...

class MeshMaterial2d(Component):
    """Material component for 2D meshes."""
    def __init__(self, material: Handle[ColorMaterial]) -> None: ...
    @property
    def handle(self) -> Handle[ColorMaterial]: ...

class MeshMaterial3d(Component):
    """Material component for 3D meshes.

    For ``@material`` types, use subscript notation to get the matching component::

        MeshMaterial3d[HologramMaterial](handle)
    """
    def __class_getitem__(cls, key: type) -> type: ...
    def __init__(self, material: Handle[StandardMaterial]) -> None: ...
    @property
    def handle(self) -> Handle[StandardMaterial]: ...

class MeshTag(Component):
    """A component that stores an arbitrary index used to identify the mesh instance when rendering."""
    def __init__(self, value: int = 0) -> None: ...
    @property
    def value(self) -> int: ...
    @value.setter
    def value(self, value: int) -> None: ...
    @staticmethod
    def from_numpy(*, value: np.typing.ArrayLike | None = None) -> Batchable: ...  # type: ignore[override]
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
    def __init__(self, weights: list[float], first_mesh: Handle[Mesh] | None = None) -> None: ...
    @property
    def first_mesh(self) -> Handle[Mesh] | None: ...
    @property
    def weights(self) -> list[float]: ...
    @weights.setter
    def weights(self, weights: list[float]) -> None: ...
    def get_weight(self, index: int) -> float: ...
    def set_weight(self, index: int, value: float) -> None: ...
    def __len__(self) -> int: ...

class SkinnedMeshInverseBindposes(Asset):
    """Inverse bind pose matrices for skeletal mesh animation.

    This asset contains the inverse bind pose matrices for each joint in a skeleton.
    Used with SkinnedMesh component for skeletal animation.

    Args:
        matrices: List of Mat4 inverse bind pose matrices, one per joint.
    """
    def __init__(self, matrices: list) -> None: ...
    def get(self, index: int) -> object | None: ...
    def to_list(self) -> list: ...
    def __len__(self) -> int: ...

class SkinnedMesh(Component):
    """Component that defines a skinned mesh for skeletal animation.

    A skinned mesh deforms its vertices based on a hierarchy of bone entities (joints).
    Each vertex is influenced by one or more bones, weighted by the inverse bind pose matrices.

    This component is typically created automatically when loading GLTF files with skeletal
    animations, but can also be constructed manually for procedural skinned meshes.
    """

    def __init__(self, inverse_bindposes: Handle, joints: list[Entity]) -> None:
        """Create a new SkinnedMesh component.

        Args:
            inverse_bindposes: Handle to SkinnedMeshInverseBindposes asset containing
                the inverse bind pose matrices for each joint
            joints: List of Entity references representing the bone hierarchy
        """

    @property
    def inverse_bindposes(self) -> Handle:
        """Get the handle to the inverse bind pose matrices asset."""

    @property
    def joints(self) -> list[Entity]:
        """Get the list of joint (bone) entities."""

    def joint_count(self) -> int:
        """Get the number of joints in the skeleton."""

    def get_joint(self, index: int) -> Entity:
        """Get a specific joint entity by index.

        Args:
            index: Zero-based index of the joint to retrieve

        Returns:
            The Entity at the specified index

        Raises:
            ValueError: If index is out of bounds
        """

    def __len__(self) -> int:
        """Get the number of joints in the skeleton."""
