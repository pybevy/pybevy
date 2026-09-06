from collections.abc import Iterator
from typing import ClassVar, Final, Generic, Literal, TypeVar, overload

from pybevy.app import App, Plugin
from pybevy.audio import AudioSource
from pybevy.color import Color
from pybevy.ecs import Message, Resource, SystemSet
from pybevy.gltf import GltfLoaderSettings
from pybevy.image import Image, ImageLoaderSettings, ImageSaverSettings
from pybevy.mesh import Mesh, Meshable, MeshBuilder
from pybevy.pbr import StandardMaterial
from pybevy.sprite import ColorMaterial
from pybevy.world_serialization import WorldAsset

A = TypeVar("A", bound=Asset)
# Handle only ever produces its asset type, never consumes it, so a
# Handle[Image] is usable wherever a Handle[Asset] is expected. Assets[A] must
# stay invariant: `add(self, asset: A)` consumes one.
A_co = TypeVar("A_co", bound=Asset, covariant=True)
_VariantA_co = TypeVar("_VariantA_co", bound=Asset, covariant=True)

AssetTrackingSystems: Final[SystemSet]
AssetEventSystems: Final[SystemSet]

class AssetPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...
    @property
    def watch_for_changes_override(self) -> bool | None:
        """Whether edits to files under ``assets/`` are picked up while the app runs.

        True when the app was launched with hot reload (``pybevy dev``/``watch``),
        False otherwise so shipped apps do not carry a file watcher.
        """

class Asset: ...

class AssetIndex:
    """Bevy's opaque, generational runtime asset index."""
    @staticmethod
    def from_bits(bits: int) -> AssetIndex: ...
    def to_bits(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class AssetId(Generic[A_co]):
    """A copyable, non-owning identifier with exact Bevy enum variants."""

    class Index(AssetId[_VariantA_co], Generic[_VariantA_co]):
        __match_args__: ClassVar[tuple[Literal["index"]]]
        index: AssetIndex
        def __init__(self, index: AssetIndex, asset_type: type[_VariantA_co]) -> None: ...

    class Uuid(AssetId[_VariantA_co], Generic[_VariantA_co]):
        __match_args__: ClassVar[tuple[Literal["uuid"]]]
        uuid: int
        def __init__(self, uuid: int, asset_type: type[_VariantA_co]) -> None: ...

    @staticmethod
    def uuid_from_u128(value: int, asset_type: type[A_co]) -> AssetId.Uuid[A_co]: ...
    def asset_type_class(self) -> type[A_co]: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class LoadedFolder(Asset):
    """A loaded folder containing handles for all assets in a directory.

    This asset is produced by AssetServer.load_folder() and is primarily used
    for waiting on asset loading completion via asset events.

    Note: Loading folders is not supported on all platforms (e.g., WASM/Web).
    """
    def __init__(self) -> None: ...
    @property
    def handles(self) -> list[Handle[Asset]]:
        """Get the list of asset handles in this folder.

        Returns a list of Handle objects for all assets loaded from the folder.
        Note: Some handles may fail to convert if they reference asset types not
        supported by PyBevy. In such cases, a warning is logged and those handles
        are skipped.
        """
    def __len__(self) -> int:
        """Get the number of assets in this folder."""

class LoadState:
    """The load state of an asset.

    An asset can be in one of four states:
    - NotLoaded: The asset has not started loading yet
    - Loading: The asset is in the process of loading
    - Loaded: The asset has been loaded and added to the world
    - Failed: The asset failed to load

    Use enum comparison (``state == LoadState.Loading()``) or the ``is_*`` methods.
    """

    @staticmethod
    def NotLoaded() -> LoadState: ...
    @staticmethod
    def Loading() -> LoadState: ...
    @staticmethod
    def Loaded() -> LoadState: ...
    @staticmethod
    def Failed() -> LoadState: ...

    def is_not_loaded(self) -> bool:
        """Returns `True` if this instance is `LoadState.NotLoaded`"""

    def is_loading(self) -> bool:
        """Returns `True` if this instance is `LoadState.Loading`"""

    def is_loaded(self) -> bool:
        """Returns `True` if this instance is `LoadState.Loaded`"""

    def is_failed(self) -> bool:
        """Returns `True` if this instance is `LoadState.Failed`"""

class Handle(Generic[A_co]):
    """Reference to an asset stored in ``Assets[A]``.

    Handles returned by ``Assets.add`` and ``AssetServer.load*`` are strong and
    keep the asset alive. After the last strong handle is dropped, Bevy
    reclaims the asset on a later asset-tracking pass.
    """

    @staticmethod
    def uuid_from_u128(value: int, asset_type: type[A_co]) -> Handle[A_co]:
        """Create a non-owning Bevy UUID handle.

        This handle does not keep an asset alive. It is useful for:
        - Referencing assets that will be loaded later
        - Creating handles for comparison/lookup
        - Testing scenarios

        Args:
            value: A u128 value to use as the UUID
            asset_type: The Python type of the asset (e.g., Mesh, Image)
        """
    def asset_type_class(self) -> type[A_co]:
        """Get the Python type class of the asset this handle refers to.

        Returns the asset type as a Python class (e.g., Mesh, Image, StandardMaterial).
        """
    def id(self) -> AssetId[A_co]:
        """Get this handle's Bevy asset identifier."""
    def is_strong(self) -> bool:
        """Check if this is a strong handle (keeps the asset alive)."""
    def is_uuid(self) -> bool:
        """Check whether this is a Bevy UUID handle."""
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class Assets(Resource, Generic[A]):
    @overload
    def add(self, asset: A) -> Handle[A]: ...
    @overload
    def add(self, asset: Meshable) -> Handle[Mesh]: ...
    @overload
    def add(self, asset: MeshBuilder) -> Handle[Mesh]: ...
    @overload
    def add(self: Assets[StandardMaterial], asset: Color) -> Handle[StandardMaterial]: ...
    @overload
    def add(self: Assets[ColorMaterial], asset: Color) -> Handle[ColorMaterial]: ...
    def contains(self, id: Handle[A] | AssetId[A]) -> bool: ...
    def get(self, id: Handle[A] | AssetId[A]) -> A | None: ...
    def get_mut(self, id: Handle[A] | AssetId[A]) -> A | None: ...
    def is_empty(self) -> bool: ...
    def __iter__(self) -> Iterator[tuple[AssetId[A], A]]: ...
    def len(self) -> int: ...
    def remove(self, id: Handle[A] | AssetId[A]) -> None | A: ...

class AssetIter(Iterator[tuple[AssetId[A], A]]):
    """Iterator over (asset ID, asset) pairs in an Assets collection."""
    def __next__(self) -> tuple[AssetId[A], A]: ...
    def __iter__(self) -> AssetIter[A]: ...

class AssetServer(Resource):
    def load(self, path: str | AssetPath, asset_type: type[A] | None = None) -> Handle[A]:
        """Load an asset. With `asset_type` omitted, the type is inferred from
        the file extension (images, audio, gltf/glb); unknown extensions raise
        with the recognized list."""

    def load_world_asset(self, path: str | AssetPath) -> Handle[WorldAsset]: ...
    def load_image(self, path: str | AssetPath) -> Handle[Image]: ...
    def load_with_settings(self, path: str | AssetPath, asset_type: type[A], settings: ImageLoaderSettings | GltfLoaderSettings) -> Handle[A]:
        """Load an asset with custom loader settings.

        Equivalent to Bevy's `asset_server.load_with_settings::<A, S>()`.

        Currently supported asset types and their settings:
        - Image + ImageLoaderSettings (sampler mode, sRGB, format)
        - Gltf + GltfLoaderSettings (content, validation, coordinates, bounds, and samplers)

        Args:
            path: Path to the asset file (relative to assets directory)
            asset_type: The asset type class (e.g. Image)
            settings: Loader settings matching the asset type

        Example:
            ```python
            from pybevy.image import Image, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor
            from pybevy.image import ImageAddressMode

            settings = ImageLoaderSettings(sampler=ImageSampler.Descriptor(
                ImageSamplerDescriptor(
                    address_mode_u=ImageAddressMode.Repeat,
                    address_mode_v=ImageAddressMode.Repeat,
                )
            ))
            handle = asset_server.load_with_settings("textures/grass.png", Image, settings)
            ```
        """
    def load_image_with_settings(self, path: str | AssetPath, settings: ImageLoaderSettings) -> Handle[Image]:
        """Convenience method: load an image with custom loader settings.

        Equivalent to `load_with_settings(path, Image, settings)`.
        """
    def save_image(
        self,
        image: Image,
        path: str | AssetPath,
        settings: ImageSaverSettings | None = None,
    ) -> None:
        """Save an image to a file, blocking until the write completes.

        Absolute paths are written as-is; relative paths resolve against the
        default asset source root. PNG is the supported format.

        Raises:
            ValueError: Unsupported format/extension or the image has no pixel data.
            OSError: File write failure.
            RuntimeError: No asset source or writer is registered.
        """
    def load_mesh(self, path: str | AssetPath) -> Handle[Mesh]: ...
    def load_audio(self, path: str | AssetPath) -> Handle[AudioSource]: ...
    def load_folder(self, path: str | AssetPath) -> Handle[LoadedFolder]:
        """Load all assets from a folder.

        Returns a Handle[LoadedFolder] that tracks the loading of all assets
        in the specified directory. All assets in the folder become dependencies
        of the LoadedFolder asset.

        Args:
            path: Path to the folder to load (relative to assets directory)

        Returns:
            Handle to the LoadedFolder asset

        Note:
            - Not supported on all platforms (e.g., WASM/Web)
            - Individual assets can still be loaded with load() after calling this
            - Use asset events to wait for loading completion
        """

    def load_state(self, id: Handle[A] | AssetId[A]) -> LoadState:
        """Get the load state for the given asset handle.

        Returns `LoadState.NotLoaded` if the asset is not tracked.

        Args:
            id: The asset handle to check

        Returns:
            The current load state of the asset
        """

    def is_loaded(self, id: Handle[A] | AssetId[A]) -> bool:
        """Check if the asset is loaded (but not necessarily its dependencies).

        Returns `True` if the asset's load state is `LoadState.Loaded`.

        Args:
            id: The asset handle to check

        Returns:
            True if the asset is loaded, False otherwise
        """

    def is_loaded_with_dependencies(self, id: Handle[A] | AssetId[A]) -> bool:
        """Check if the asset and all of its dependencies are loaded.

        Returns `True` if the asset and all recursive dependencies have finished loading.
        This is the method you typically want to use before transitioning to a new scene.

        Args:
            id: The asset handle to check

        Returns:
            True if the asset and all dependencies are loaded, False otherwise

        Example:
            ```python
            from dataclasses import dataclass

            from pybevy.assets import AssetServer, Handle
            from pybevy.decorators import resource
            from pybevy.ecs import Res, Resource
            from pybevy.world_serialization import WorldAsset

            @resource
            @dataclass
            class WorldHandle(Resource):
                value: Handle[WorldAsset]

            def is_world_ready(
                asset_server: Res[AssetServer],
                world_handle: Res[WorldHandle],
            ) -> bool:
                return asset_server.is_loaded_with_dependencies(world_handle.value)
            ```
        """

    def get_handle(self, path: str | AssetPath, asset_type: type[A]) -> Handle[A] | None:
        """Get an existing handle for the given path if the asset has already started loading.

        Returns `None` if the asset at the given path has not been loaded yet.

        Args:
            path: Path to the asset (relative to assets directory)
            asset_type: The type of asset to retrieve

        Returns:
            The handle if the asset is being tracked, None otherwise

        Example:
            ```python
            # First load
            handle1 = asset_server.load("model.gltf", WorldAsset)

            # Later, retrieve the same handle without re-loading
            handle2 = asset_server.get_handle("model.gltf", WorldAsset)
            assert handle1 == handle2  # Same handle
            ```
        """

class AssetTypeParam:
    def asset_type_class(self) -> type:
        """Get the Python type class for this asset type parameter."""

class AssetPath:
    def __init__(
        self,
        path: str,
        label: str | None = None,
        source: str | None = None,
    ) -> None: ...
    @staticmethod
    def parse(asset_path: str) -> AssetPath: ...
    @property
    def path(self) -> str: ...
    @property
    def label(self) -> str | None: ...
    @property
    def source(self) -> str | None: ...

class DependencyLoadState:
    """Load state of an asset's direct dependencies."""

    @staticmethod
    def NotLoaded() -> DependencyLoadState: ...
    @staticmethod
    def Loading() -> DependencyLoadState: ...
    @staticmethod
    def Loaded() -> DependencyLoadState: ...
    @staticmethod
    def Failed() -> DependencyLoadState: ...

    def is_not_loaded(self) -> bool:
        """Returns True if dependencies have not started loading."""

    def is_loading(self) -> bool:
        """Returns True if dependencies are loading."""

    def is_loaded(self) -> bool:
        """Returns True if all dependencies are loaded."""

    def is_failed(self) -> bool:
        """Returns True if any dependency failed to load."""

class RecursiveDependencyLoadState:
    """Load state of an asset's recursive (transitive) dependencies."""

    @staticmethod
    def NotLoaded() -> RecursiveDependencyLoadState: ...
    @staticmethod
    def Loading() -> RecursiveDependencyLoadState: ...
    @staticmethod
    def Loaded() -> RecursiveDependencyLoadState: ...
    @staticmethod
    def Failed() -> RecursiveDependencyLoadState: ...

    def is_not_loaded(self) -> bool:
        """Returns True if recursive dependencies have not started loading."""

    def is_loading(self) -> bool:
        """Returns True if recursive dependencies are loading."""

    def is_loaded(self) -> bool:
        """Returns True if all recursive dependencies are loaded."""

    def is_failed(self) -> bool:
        """Returns True if any recursive dependency failed to load."""

class AssetServerMode:
    """Asset server processing mode."""

    Unprocessed: ClassVar[AssetServerMode]
    Processed: ClassVar[AssetServerMode]

class AssetEvent(Message, Generic[A_co]):
    """Event fired when an asset's state changes.

    AssetEvents are fired during the asset lifecycle for operations like
    loading, reloading, and unloading assets. Select the asset channel with
    ``MessageReader[AssetEvent[Image]]``.

    Example:
        ```python
        def track_image_loads(reader: MessageReader[AssetEvent[Image]]):
            for event in reader.read():
                if isinstance(event, AssetEvent.LoadedWithDependencies):
                    print(f"Image loaded: {event.id}")
        ```

    Each Bevy variant is represented by an exact nested class carrying its
    ``AssetId[A]``.
    """

    def is_loaded_with_dependencies(self, asset_id: AssetId[A_co] | Handle[A_co]) -> bool:
        """True if this event is LoadedWithDependencies and matches the id."""
    def is_added(self, asset_id: AssetId[A_co] | Handle[A_co]) -> bool:
        """True if this event is Added and matches the id."""
    def is_modified(self, asset_id: AssetId[A_co] | Handle[A_co]) -> bool:
        """True if this event is Modified and matches the id."""
    def is_removed(self, asset_id: AssetId[A_co] | Handle[A_co]) -> bool:
        """True if this event is Removed and matches the id."""
    def is_unused(self, asset_id: AssetId[A_co] | Handle[A_co]) -> bool:
        """True if this event is Unused and matches the id."""

    class Added(AssetEvent[_VariantA_co], Generic[_VariantA_co]):
        __match_args__: ClassVar[tuple[Literal["id"]]]
        id: AssetId[_VariantA_co]
        def __init__(self, id: AssetId[_VariantA_co]) -> None: ...

    class Modified(AssetEvent[_VariantA_co], Generic[_VariantA_co]):
        __match_args__: ClassVar[tuple[Literal["id"]]]
        id: AssetId[_VariantA_co]
        def __init__(self, id: AssetId[_VariantA_co]) -> None: ...

    class Removed(AssetEvent[_VariantA_co], Generic[_VariantA_co]):
        __match_args__: ClassVar[tuple[Literal["id"]]]
        id: AssetId[_VariantA_co]
        def __init__(self, id: AssetId[_VariantA_co]) -> None: ...

    class Unused(AssetEvent[_VariantA_co], Generic[_VariantA_co]):
        __match_args__: ClassVar[tuple[Literal["id"]]]
        id: AssetId[_VariantA_co]
        def __init__(self, id: AssetId[_VariantA_co]) -> None: ...

    class LoadedWithDependencies(AssetEvent[_VariantA_co], Generic[_VariantA_co]):
        __match_args__: ClassVar[tuple[Literal["id"]]]
        id: AssetId[_VariantA_co]
        def __init__(self, id: AssetId[_VariantA_co]) -> None: ...

class AssetLoadFailedEvent(Message, Generic[A_co]):
    """Event fired when an asset fails to load.

    Select the asset channel with ``MessageReader[AssetLoadFailedEvent[Image]]``.

    Example:
        ```python
        def report_failures(reader: MessageReader[AssetLoadFailedEvent[Image]]):
            for event in reader:
                print(f"{event.path.path} failed: {event.error}")
        ```
    """

    @property
    def id(self) -> AssetId[A_co]:
        """The asset that failed to load."""

    @property
    def path(self) -> AssetPath:
        """The path the load was attempted from."""

    @property
    def error(self) -> str:
        """Bevy's rendered `AssetLoadError` message for this failure."""

    def __eq__(self, other: object) -> bool: ...

class UnapprovedPathMode:
    """Controls behavior when an asset path hasn't been explicitly approved.

    Used to configure how the asset server handles unapproved paths.
    """

    Allow: UnapprovedPathMode
    """Allow unapproved paths (permissive mode)."""

    Deny: UnapprovedPathMode
    """Deny unapproved paths with an error."""

    Forbid: UnapprovedPathMode
    """Forbid unapproved paths (strictest mode)."""

    def __eq__(self, other: object) -> bool: ...
