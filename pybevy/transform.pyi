from typing import ClassVar, overload

from pybevy.app import App, Plugin
from pybevy.ecs import Batchable, Component
from pybevy.math import Affine3A, Dir3, Isometry3d, Mat4, Quat, Vec3

class TransformPlugin(Plugin):
    """Transform plugin providing transform propagation and hierarchy systems."""
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

try:
    import numpy as np
except ImportError:
    pass

class GlobalTransform(Component):
    """Read-only component representing an entity's world-space transform.

    GlobalTransform is computed automatically by Bevy by combining the Transform
    of an entity with all its ancestors. It cannot be directly mutated - instead,
    modify the entity's Transform component.

    Use GlobalTransform to:
    - Read world-space positions of entities in hierarchies
    - Transform points from local to world space
    - Get world-space directions (forward, up, right, etc.)
    - Compute relative transforms when reparenting entities
    """

    def __init__(self) -> None:
        """Create an identity GlobalTransform."""

    IDENTITY: ClassVar[GlobalTransform]
    """Returns an identity GlobalTransform (no translation, identity rotation, unit scale)."""

    @property
    def translation(self) -> Vec3:
        """Get the world-space translation as a Vec3."""

    @property
    def rotation(self) -> Quat:
        """Get the world-space rotation as a Quat.

        Note: This is computed using to_scale_rotation_translation().
        If you also need translation or scale, use to_scale_rotation_translation() instead.
        """

    @property
    def scale(self) -> Vec3:
        """Get the world-space scale as a Vec3.

        Note: Some computations overlap with to_scale_rotation_translation().
        If you also need rotation, use to_scale_rotation_translation() instead.
        """

    def to_scale_rotation_translation(self) -> tuple[Vec3, Quat, Vec3]:
        """Extract scale, rotation, and translation from this transform.

        Returns a tuple of (scale, rotation, translation).
        More efficient than calling scale(), rotation(), and translation() separately.
        """

    def to_matrix(self) -> Mat4:
        """Returns the 3D affine transformation matrix as a Mat4."""

    def affine(self) -> Affine3A:
        """Returns the 3D affine transformation matrix as an Affine3A."""

    def to_isometry(self) -> Isometry3d:
        """Get the isometry defined by this transform's rotation and translation, ignoring scale.

        Note: The transform is expected to be non-degenerate and without shearing,
        or the output will be invalid.
        """

    def compute_transform(self) -> Transform:
        """Convert this GlobalTransform to a local Transform.

        The transform is expected to be non-degenerate and without shearing,
        or the output will be invalid.
        """

    def transform_point(self, point: Vec3) -> Vec3:
        """Transform a point from local space to world space.

        Applies shear, scale, rotation, and translation.

        Note: GlobalTransform is typically obtained by querying entities,
        not constructed directly. Use Transform for entity setup.
        """

    def reparented_to(self, parent: GlobalTransform) -> Transform:
        """Compute the local Transform this entity would need if reparented.

        Returns the Transform self would have if it was a child of an entity
        with the given parent GlobalTransform, while maintaining the same
        world-space position.

        Useful for maintaining an entity's world position when changing its parent.

        The transform is expected to be non-degenerate and without shearing,
        or the output will be invalid.
        """

    def right(self) -> Vec3:
        """Return the local right vector (X axis)."""

    def left(self) -> Vec3:
        """Return the local left vector (-X axis)."""

    def up(self) -> Vec3:
        """Return the local up vector (Y axis)."""

    def down(self) -> Vec3:
        """Return the local down vector (-Y axis)."""

    def forward(self) -> Vec3:
        """Return the local forward vector (Z axis)."""

    def back(self) -> Vec3:
        """Return the local back vector (-Z axis)."""

    def mul_transform(self, transform: Transform) -> GlobalTransform:
        """Multiply this GlobalTransform by a Transform.

        Returns a new GlobalTransform that represents applying this transform
        followed by the given local transform.
        """

    def __copy__(self) -> GlobalTransform: ...
    def __deepcopy__(self, memo: dict[int, object]) -> GlobalTransform: ...

class Transform(Component):
    translation: Vec3
    rotation: Quat
    scale: Vec3

    def __init__(
        self,
        translation: Vec3 = Vec3.ZERO,
        rotation: Quat = Quat.IDENTITY,
        scale: Vec3 = Vec3.ONE,
    ) -> None: ...
    IDENTITY: ClassVar[Transform]
    """Returns a new identity Transform (no translation, identity rotation, unit scale)."""
    @staticmethod
    def from_xyz(x: float, y: float, z: float) -> Transform: ...
    @staticmethod
    def from_rotation(rotation: Quat) -> Transform: ...
    @staticmethod
    def from_scale(scale: Vec3) -> Transform: ...
    @staticmethod
    def from_translation(translation: Vec3) -> Transform: ...
    @staticmethod
    def from_matrix(world_from_local: Mat4) -> Transform: ...
    @staticmethod
    def from_isometry(iso: Isometry3d) -> Transform:
        """Create a Transform equivalent to the given isometry.

        The resulting Transform will have the isometry's translation and rotation,
        with a scale of 1.
        """
    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        translation: np.ndarray | None = None,
        rotation: np.ndarray | None = None,
        scale: np.ndarray | None = None,
    ) -> Batchable: ...
    def rotate_x(self, angle: float) -> None: ...
    def rotate_y(self, angle: float) -> None: ...
    def rotate_z(self, angle: float) -> None: ...
    def rotate(self, rotation: Quat) -> None: ...
    def look_at(self, target: Vec3, up: Vec3 | Dir3) -> None: ...
    def looking_at(self, target: Vec3, up: Vec3 | Dir3) -> Transform: ...
    def looking_to(self, direction: Vec3, up: Vec3 | Dir3) -> Transform: ...
    def aligned_by(
        self,
        main_axis: Vec3 | Dir3,
        main_direction: Vec3 | Dir3,
        secondary_axis: Vec3 | Dir3,
        secondary_direction: Vec3 | Dir3,
    ) -> Transform: ...
    def align(
        self,
        main_axis: Vec3 | Dir3,
        main_direction: Vec3 | Dir3,
        secondary_axis: Vec3 | Dir3,
        secondary_direction: Vec3 | Dir3,
    ) -> None: ...
    def to_matrix(self) -> Mat4: ...
    def compute_affine(self) -> Affine3A: ...
    def local_x(self) -> Dir3: ...
    def local_y(self) -> Dir3: ...
    def local_z(self) -> Dir3: ...
    def left(self) -> Dir3: ...
    def right(self) -> Dir3: ...
    def forward(self) -> Dir3: ...
    def back(self) -> Dir3: ...
    def up(self) -> Dir3: ...
    def down(self) -> Dir3: ...
    def with_translation(self, translation: Vec3) -> Transform: ...
    def with_rotation(self, rotation: Quat) -> Transform: ...
    @overload
    def with_scale(self, scale: Vec3) -> Transform: ...
    @overload
    def with_scale(self, scale: tuple[float, float, float]) -> Transform: ...
    def transform_point(self, point: Vec3) -> Vec3: ...
    def is_finite(self) -> bool: ...
    def to_isometry(self) -> Isometry3d:
        """Get the isometry defined by this transform's rotation and translation, ignoring scale."""
    def mul_transform(self, transform: Transform) -> Transform: ...
    def __copy__(self) -> Transform: ...
    def __deepcopy__(self, memo: dict[int, object]) -> Transform: ...
    def rotate_axis(self, axis: Dir3, angle: float) -> None: ...
    def rotate_local(self, rotation: Quat) -> None: ...
    def rotate_local_axis(self, axis: Dir3, angle: float) -> None: ...
    def rotate_local_x(self, angle: float) -> None: ...
    def rotate_local_y(self, angle: float) -> None: ...
    def rotate_local_z(self, angle: float) -> None: ...
    def translate_around(self, point: Vec3, rotation: Quat) -> None: ...
    def rotate_around(self, point: Vec3, rotation: Quat) -> None: ...
    def look_to(self, direction: Vec3, up: Vec3 | Dir3) -> None: ...
