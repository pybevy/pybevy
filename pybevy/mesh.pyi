from collections.abc import Iterator
from enum import Enum
from types import TracebackType
from typing import ClassVar

import numpy as np

from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.ecs import Component, Entity
from pybevy.image import RenderAssetUsages
from pybevy.math import Dir3, Vec2
from pybevy.pbr import StandardMaterial
from pybevy.sprite import ColorMaterial

class MeshPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class MeshBuilder:
    def build(self) -> Mesh: ...

class Meshable:
    def mesh(self) -> MeshBuilder: ...

class MeshAttributeContext:
    """Context manager for read-only zero-copy access to mesh vertex attributes.

    Provides a NumPy array view of vertex attribute data (positions, normals, etc.)
    without copying. Use within a `with` statement.

    Example:
        ```python
        with mesh.positions_context() as positions:
            mean_pos = positions.mean(axis=0)
        ```
    """
    def __enter__(self) -> np.ndarray:
        """Enter the context and return the numpy array view."""

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> bool:
        """Exit the context (no cleanup needed)."""

class MeshAttributeContextMut:
    """Context manager for mutable zero-copy access to mesh vertex attributes.

    Provides a mutable NumPy array view of vertex attribute data.
    Modifications are immediately reflected in the mesh.
    Use within a `with` statement.

    Example:
        ```python
        with mesh.positions_context_mut() as positions:
            positions[:, 2] += 1.0  # Translate in Z
        ```
    """
    def __enter__(self) -> np.ndarray:
        """Enter the context and return the mutable numpy array view."""

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> bool:
        """Exit the context (no cleanup needed)."""

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
    def with_inserted_attribute(
        self, attribute: MeshVertexAttribute, values: VertexAttributeValues | np.ndarray
    ) -> Mesh: ...
    def insert_attribute(
        self, attribute: MeshVertexAttribute, values: VertexAttributeValues | np.ndarray
    ) -> None: ...
    def with_inserted_indices(
        self, indices: Indices | np.ndarray | list[int]
    ) -> Mesh: ...
    def insert_indices(self, indices: Indices | np.ndarray | list[int]) -> None: ...

    def positions(self) -> MeshAttributeContext:
        """Get zero-copy read-only access to vertex positions via context manager.

        Returns:
            Context manager yielding readonly numpy array with shape (N, 3)

        Example:
            ```python
            with mesh.positions_context() as positions:
                mean_pos = positions.mean(axis=0)
            ```
        """

    def positions_mut(self) -> MeshAttributeContextMut:
        """Get zero-copy mutable access to vertex positions via context manager.

        Returns:
            Context manager yielding mutable numpy array with shape (N, 3)

        Example:
            ```python
            with mesh.positions_context_mut() as positions:
                positions[:, 2] += 1.0  # Move all vertices up by 1
            ```
        """

    def normals(self) -> MeshAttributeContext:
        """Get zero-copy read-only access to vertex normals via context manager."""

    def normals_mut(self) -> MeshAttributeContextMut:
        """Get zero-copy mutable access to vertex normals via context manager."""

    def uvs(self) -> MeshAttributeContext:
        """Get zero-copy read-only access to UV coordinates via context manager."""

    def uvs_mut(self) -> MeshAttributeContextMut:
        """Get zero-copy mutable access to UV coordinates via context manager."""

    def attribute(self, id: MeshVertexAttribute) -> MeshAttributeContext:
        """Get zero-copy read-only access to any vertex attribute via context manager."""

    def attribute_mut(self, id: MeshVertexAttribute) -> MeshAttributeContextMut:
        """Get zero-copy mutable access to any vertex attribute via context manager."""

    def positions_copy(self) -> np.ndarray:
        """Get an owned copy of vertex positions as numpy array with shape (N, 3)."""

    def set_positions(self, positions: np.ndarray) -> None:
        """Copy vertex positions from numpy array with shape (N, 3)."""

    def normals_copy(self) -> np.ndarray:
        """Get an owned copy of vertex normals as numpy array with shape (N, 3)."""

    def set_normals(self, normals: np.ndarray) -> None:
        """Copy vertex normals from numpy array with shape (N, 3)."""

    def with_generated_tangents(self) -> Mesh: ...
    def generate_tangents(self) -> None: ...

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
    UV0: ClassVar[UvChannel]
    UV1: ClassVar[UvChannel]

    def __init__(self) -> None: ...

class MeshVertexAttribute:
    @property
    def name(self) -> str: ...
    @property
    def id(self) -> int: ...
    @property
    def format(self) -> str: ...
    def __eq__(self, other: object) -> bool: ...

class Indices:
    def __init__(self, obj: np.ndarray | list[int]) -> None: ...
    @property
    def len(self) -> int: ...
    def is_empty(self) -> bool: ...
    def __iter__(self) -> Iterator[IndicesIterator]: ...
    def __eq__(self, other: object) -> bool: ...

class IndicesIterator:
    def __next__(self) -> int: ...

class VertexAttributeValues:
    def __init__(self, obj: np.ndarray) -> None: ...
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
    def kind(self) -> SphereKind: ...
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
    def __init__(self) -> None: ...

    @staticmethod
    def ico(subdivisions: int) -> SphereKind:
        """Icosphere mesh kind with the given number of subdivisions."""

    @staticmethod
    def uv(sectors: int, stacks: int) -> SphereKind:
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
