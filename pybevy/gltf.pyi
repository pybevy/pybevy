from typing import ClassVar, Literal

from pybevy.animation import AnimationClip
from pybevy.app import App, Plugin
from pybevy.assets import Asset, AssetPath, Handle, RenderAssetUsages
from pybevy.collections import LiveSequence
from pybevy.ecs import Component
from pybevy.image import ImageSamplerDescriptor
from pybevy.transform import Transform

class GltfPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class GltfMaterial(Asset):
    """Source material data loaded from a glTF file."""

class Gltf(Asset):
    """Representation of a loaded glTF file."""

    @property
    def scenes(self) -> list[Handle]: ...
    @scenes.setter
    def scenes(self, value: list[Handle]) -> None: ...
    @property
    def named_scenes(self) -> list[tuple[str, Handle]]: ...
    @named_scenes.setter
    def named_scenes(self, value: list[tuple[str, Handle]]) -> None: ...
    @property
    def meshes(self) -> list[Handle]: ...
    @meshes.setter
    def meshes(self, value: list[Handle]) -> None: ...
    @property
    def named_meshes(self) -> list[tuple[str, Handle]]: ...
    @named_meshes.setter
    def named_meshes(self, value: list[tuple[str, Handle]]) -> None: ...
    @property
    def materials(self) -> list[Handle[GltfMaterial]]: ...
    @materials.setter
    def materials(self, value: list[Handle[GltfMaterial]]) -> None: ...
    @property
    def named_materials(self) -> list[tuple[str, Handle[GltfMaterial]]]: ...
    @named_materials.setter
    def named_materials(self, value: list[tuple[str, Handle[GltfMaterial]]]) -> None: ...
    @property
    def nodes(self) -> list[Handle]: ...
    @nodes.setter
    def nodes(self, value: list[Handle]) -> None: ...
    @property
    def named_nodes(self) -> list[tuple[str, Handle]]: ...
    @named_nodes.setter
    def named_nodes(self, value: list[tuple[str, Handle]]) -> None: ...
    @property
    def skins(self) -> list[Handle]: ...
    @skins.setter
    def skins(self, value: list[Handle]) -> None: ...
    @property
    def named_skins(self) -> list[tuple[str, Handle]]: ...
    @named_skins.setter
    def named_skins(self, value: list[tuple[str, Handle]]) -> None: ...
    @property
    def default_scene(self) -> Handle | None: ...
    @default_scene.setter
    def default_scene(self, value: Handle | None) -> None: ...
    @property
    def animations(self) -> list[Handle[AnimationClip]]: ...
    @animations.setter
    def animations(self, value: list[Handle[AnimationClip]]) -> None: ...
    @property
    def named_animations(self) -> dict[str, Handle[AnimationClip]]: ...
    @named_animations.setter
    def named_animations(self, value: dict[str, Handle[AnimationClip]]) -> None: ...

class GltfMesh(Asset):
    """A glTF mesh, which may consist of multiple primitives."""

    @property
    def index(self) -> int: ...
    @index.setter
    def index(self, value: int) -> None: ...
    @property
    def name(self) -> str: ...
    @name.setter
    def name(self, value: str) -> None: ...
    @property
    def primitives(self) -> LiveSequence[GltfPrimitive]: ...
    @primitives.setter
    def primitives(self, value: list[GltfPrimitive]) -> None: ...
    @property
    def extras(self) -> str | None:
        """Additional untyped data from the glTF file (JSON string)."""
    @extras.setter
    def extras(self, value: str | None) -> None: ...
    def asset_label(self) -> GltfAssetLabel: ...

class GltfNode(Asset):
    """A glTF node with its children, mesh, transform, and skin."""

    @property
    def index(self) -> int: ...
    @index.setter
    def index(self, value: int) -> None: ...
    @property
    def name(self) -> str: ...
    @name.setter
    def name(self, value: str) -> None: ...
    @property
    def children(self) -> list[Handle]: ...
    @children.setter
    def children(self, value: list[Handle]) -> None: ...
    @property
    def mesh(self) -> Handle | None: ...
    @mesh.setter
    def mesh(self, value: Handle | None) -> None: ...
    @property
    def skin(self) -> Handle | None: ...
    @skin.setter
    def skin(self, value: Handle | None) -> None: ...
    @property
    def transform(self) -> Transform: ...
    @transform.setter
    def transform(self, value: Transform) -> None: ...
    @property
    def extras(self) -> str | None:
        """Additional untyped data from the glTF file (JSON string)."""
    @extras.setter
    def extras(self, value: str | None) -> None: ...
    def asset_label(self) -> GltfAssetLabel: ...

class GltfPrimitive(Asset):
    """Part of a GltfMesh consisting of a mesh and optional material."""

    @property
    def index(self) -> int: ...
    @index.setter
    def index(self, value: int) -> None: ...
    @property
    def parent_mesh_index(self) -> int: ...
    @parent_mesh_index.setter
    def parent_mesh_index(self, value: int) -> None: ...
    @property
    def name(self) -> str: ...
    @name.setter
    def name(self, value: str) -> None: ...
    @property
    def mesh(self) -> Handle: ...
    @mesh.setter
    def mesh(self, value: Handle) -> None: ...
    @property
    def material(self) -> Handle[GltfMaterial] | None: ...
    @material.setter
    def material(self, value: Handle[GltfMaterial] | None) -> None: ...
    @property
    def extras(self) -> str | None:
        """Additional untyped data from the glTF file (JSON string)."""
    @extras.setter
    def extras(self, value: str | None) -> None: ...
    @property
    def material_extras(self) -> str | None:
        """Additional untyped material data from the glTF file (JSON string)."""
    @material_extras.setter
    def material_extras(self, value: str | None) -> None: ...
    def asset_label(self) -> GltfAssetLabel: ...

class GltfSkin(Asset):
    """A glTF skin with joint nodes and inverse-bind matrices."""

    @property
    def index(self) -> int: ...
    @index.setter
    def index(self, value: int) -> None: ...
    @property
    def name(self) -> str: ...
    @name.setter
    def name(self, value: str) -> None: ...
    @property
    def joints(self) -> list[Handle]: ...
    @joints.setter
    def joints(self, value: list[Handle]) -> None: ...
    @property
    def inverse_bind_matrices(self) -> Handle: ...
    @inverse_bind_matrices.setter
    def inverse_bind_matrices(self, value: Handle) -> None: ...
    @property
    def extras(self) -> str | None:
        """Additional untyped data from the glTF file (JSON string)."""
    @extras.setter
    def extras(self, value: str | None) -> None: ...
    def asset_label(self) -> GltfAssetLabel: ...

class GltfExtras(Component):
    """Additional untyped data that can be present on GLTF types at the primitive level."""

    def __init__(self, *, value: str = "") -> None: ...
    @property
    def value(self) -> str: ...
    @value.setter
    def value(self, value: str) -> None: ...

class GltfMeshName(Component):
    """The mesh name of a glTF primitive."""

    def __init__(self, name: str = "") -> None: ...
    @property
    def name(self) -> str: ...
    @name.setter
    def name(self, value: str) -> None: ...
    def __str__(self) -> str: ...

class GltfMaterialName(Component):
    """The material name of a glTF primitive."""

    def __init__(self, name: str = "") -> None: ...
    @property
    def name(self) -> str: ...
    @name.setter
    def name(self, value: str) -> None: ...
    def __str__(self) -> str: ...

class GltfSceneExtras(Component):
    """Additional untyped data that can be present on GLTF types at the scene level."""

    def __init__(self, *, value: str = "") -> None: ...
    @property
    def value(self) -> str: ...
    @value.setter
    def value(self, value: str) -> None: ...

class GltfMeshExtras(Component):
    """Additional untyped data that can be present on GLTF types at the mesh level."""

    def __init__(self, *, value: str = "") -> None: ...
    @property
    def value(self) -> str: ...
    @value.setter
    def value(self, value: str) -> None: ...

class GltfMaterialExtras(Component):
    """Additional untyped data that can be present on GLTF types at the material level."""

    def __init__(self, *, value: str = "") -> None: ...
    @property
    def value(self) -> str: ...
    @value.setter
    def value(self, value: str) -> None: ...

class GltfConvertCoordinates:
    """How to convert glTF's +Z-forward coordinates to Bevy's -Z-forward.

    Both are Y-up; they disagree about forward and right, so the conversion is
    a half turn about Y (glTF forward +Z, right -X; Bevy forward -Z, right +X).
    The two switches choose where that rotation is applied. Cameras and lights
    already use Bevy local coordinates; rotating the scene root also rotates
    their world transforms.
    """

    def __init__(
        self,
        *,
        rotate_scene_entity: bool = False,
        rotate_meshes: bool = False,
    ) -> None:
        """Create a coordinate conversion setting.

        Args:
            rotate_scene_entity: Rotate the scene's root entity, leaving mesh
                vertex data untouched (default: False).
            rotate_meshes: Convert mesh vertices, skinned-mesh bind poses, and bounds,
                compensating instancing entity transforms (default: False).
        """

    @property
    def rotate_scene_entity(self) -> bool:
        """Whether the scene's root entity carries the conversion rotation."""

    @rotate_scene_entity.setter
    def rotate_scene_entity(self, value: bool) -> None: ...

    @property
    def rotate_meshes(self) -> bool:
        """Whether mesh assets and bind poses are converted with compensating mesh entity transforms."""

    @rotate_meshes.setter
    def rotate_meshes(self, value: bool) -> None: ...

class GltfSkinnedMeshBoundsPolicy:

    def __copy__(self) -> GltfSkinnedMeshBoundsPolicy: ...
    def __deepcopy__(self, memo: dict[int, object]) -> GltfSkinnedMeshBoundsPolicy: ...

    BindPose: GltfSkinnedMeshBoundsPolicy
    Dynamic: GltfSkinnedMeshBoundsPolicy
    NoFrustumCulling: GltfSkinnedMeshBoundsPolicy

class GltfLoaderSettings:
    """Settings for loading glTF files.

    Pass these to `AssetServer.load_with_settings` to skip parts of a document,
    override the image sampler, or change how glTF's coordinate system is
    converted. Plain `load` takes no settings and uses the defaults.
    """

    def __init__(
        self,
        *,
        load_meshes: RenderAssetUsages = RenderAssetUsages(),
        load_materials: RenderAssetUsages = RenderAssetUsages(),
        load_cameras: bool = True,
        load_lights: bool = True,
        load_animations: bool = True,
        include_source: bool = False,
        default_sampler: ImageSamplerDescriptor | None = None,
        override_sampler: bool = False,
        validate: bool = True,
        convert_coordinates: GltfConvertCoordinates | None = None,
        skinned_mesh_bounds_policy: GltfSkinnedMeshBoundsPolicy | None = None,
    ) -> None:
        """Create glTF loader settings.

        Args:
            load_meshes: Where loaded mesh data is retained. Defaults to
                `MAIN_WORLD | RENDER_WORLD`. Remove both flags from a usage
                value to skip mesh nodes.
            load_materials: Where loaded material data is retained. Defaults to
                `MAIN_WORLD | RENDER_WORLD`. Remove both flags from a usage
                value to skip materials.
            load_cameras: Spawn a camera for each glTF camera node.
            load_lights: Spawn a light for each glTF light node.
            load_animations: Load `AnimationClip` assets and add
                `AnimationTarget` and `AnimationPlayer` to animated hierarchies.
            include_source: Keep the parsed glTF document on the loaded asset.
            default_sampler: Sampler to start from, before the document's own
                sampler data is applied on top. `None` uses the global default.
            override_sampler: Ignore the document's sampler data and use the
                default sampler as-is.
            validate: Run the glTF crate's validation pass while parsing.
            convert_coordinates: Set coordinate conversion for this load. `None` uses
                Bevy's plugin default, which PyBevy does not expose for configuration.
            skinned_mesh_bounds_policy: Set the skinned-mesh bounds policy for this load.
                `None` uses Bevy's plugin default, which PyBevy does not expose
                for configuration.
        """

    @property
    def load_meshes(self) -> RenderAssetUsages:
        """Where loaded mesh data is retained. An independent snapshot; assign a replacement to update the settings."""

    @load_meshes.setter
    def load_meshes(self, value: RenderAssetUsages) -> None: ...

    @property
    def load_materials(self) -> RenderAssetUsages:
        """Where loaded material data is retained. An independent snapshot; assign a replacement to update the settings."""

    @load_materials.setter
    def load_materials(self, value: RenderAssetUsages) -> None: ...

    @property
    def load_cameras(self) -> bool:
        """Whether a camera is spawned for each glTF camera node."""

    @load_cameras.setter
    def load_cameras(self, value: bool) -> None: ...

    @property
    def load_lights(self) -> bool:
        """Whether a light is spawned for each glTF light node."""

    @load_lights.setter
    def load_lights(self, value: bool) -> None: ...

    @property
    def load_animations(self) -> bool:
        """Whether animation clips, targets and players are loaded."""

    @load_animations.setter
    def load_animations(self, value: bool) -> None: ...

    @property
    def include_source(self) -> bool:
        """Whether the parsed glTF document is kept on the loaded asset."""

    @include_source.setter
    def include_source(self, value: bool) -> None: ...

    @property
    def default_sampler(self) -> ImageSamplerDescriptor | None:
        """Sampler applied before the document's own sampler data."""

    @default_sampler.setter
    def default_sampler(self, value: ImageSamplerDescriptor | None) -> None: ...

    @property
    def override_sampler(self) -> bool:
        """Whether the document's sampler data is ignored entirely."""

    @override_sampler.setter
    def override_sampler(self, value: bool) -> None: ...

    @property
    def validate(self) -> bool:
        """Run glTF JSON validation while parsing; later loader errors still
        fail the load.
        """

    @validate.setter
    def validate(self, value: bool) -> None: ...

    @property
    def convert_coordinates(self) -> GltfConvertCoordinates | None:
        """Coordinate conversion for this load; None uses the unexposed Bevy plugin default."""

    @convert_coordinates.setter
    def convert_coordinates(self, value: GltfConvertCoordinates | None) -> None: ...

    @property
    def skinned_mesh_bounds_policy(self) -> GltfSkinnedMeshBoundsPolicy | None:
        """Bounds policy for this load; None uses the unexposed Bevy plugin default."""

    @skinned_mesh_bounds_policy.setter
    def skinned_mesh_bounds_policy(self, value: GltfSkinnedMeshBoundsPolicy | None) -> None: ...

class GltfAssetLabel:
    class Scene(GltfAssetLabel):
        __match_args__: ClassVar[tuple[Literal["index"]]]
        index: int
        def __init__(self, index: int) -> None: ...

    class Node(GltfAssetLabel):
        __match_args__: ClassVar[tuple[Literal["index"]]]
        index: int
        def __init__(self, index: int) -> None: ...

    class Mesh(GltfAssetLabel):
        __match_args__: ClassVar[tuple[Literal["index"]]]
        index: int
        def __init__(self, index: int) -> None: ...

    class Primitive(GltfAssetLabel):
        __match_args__: ClassVar[tuple[Literal["mesh"], Literal["primitive"]]]
        mesh: int
        primitive: int
        def __init__(self, *, mesh: int, primitive: int) -> None: ...

    class Texture(GltfAssetLabel):
        __match_args__: ClassVar[tuple[Literal["index"]]]
        index: int
        def __init__(self, index: int) -> None: ...

    class Material(GltfAssetLabel):
        __match_args__: ClassVar[tuple[Literal["index"], Literal["is_scale_inverted"]]]
        index: int
        is_scale_inverted: bool
        def __init__(self, *, index: int, is_scale_inverted: bool) -> None:
            """Label for the GltfMaterial sub-asset of a glTF file.

            The processed StandardMaterial lives under the "/std" suffix; load it
            with an explicit label, e.g.
            ``AssetPath(path, label=f"{GltfAssetLabel.Material(0, False)}/std")``.
            """

    class DefaultMaterial(GltfAssetLabel):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Animation(GltfAssetLabel):
        __match_args__: ClassVar[tuple[Literal["index"]]]
        index: int
        def __init__(self, index: int) -> None: ...

    class Skin(GltfAssetLabel):
        __match_args__: ClassVar[tuple[Literal["index"]]]
        index: int
        def __init__(self, index: int) -> None: ...

    class InverseBindMatrices(GltfAssetLabel):
        __match_args__: ClassVar[tuple[Literal["index"]]]
        index: int
        def __init__(self, index: int) -> None: ...

    def from_asset(self, path: str) -> AssetPath: ...
    def __str__(self) -> str: ...
