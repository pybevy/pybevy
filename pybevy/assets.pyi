from collections.abc import Iterator
from enum import Enum
from typing import ClassVar, Generic, TypeVar, overload

from pybevy.app import App, Plugin
from pybevy.audio import AudioSource
from pybevy.color import Color
from pybevy.ecs import Message, Resource
from pybevy.image import Image, ImageLoaderSettings
from pybevy.mesh import Mesh, Meshable, MeshBuilder
from pybevy.pbr import StandardMaterial
from pybevy.scene import Scene
from pybevy.sprite import ColorMaterial

A = TypeVar("A", bound=Asset)

class AssetPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class Asset: ...

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

class Handle(Generic[A]):
    @staticmethod
    def weak_from_u128(value: int, asset_type: type[A]) -> Handle[A]:
        """Create a weak handle from a UUID.

        This creates a handle that does NOT keep the asset alive. It's useful for:
        - Referencing assets that will be loaded later
        - Creating handles for comparison/lookup
        - Testing scenarios

        Args:
            value: A u128 value to use as the UUID
            asset_type: The Python type of the asset (e.g., Mesh, Image)
        """
    def asset_type_class(self) -> type[A]:
        """Get the Python type class of the asset this handle refers to.

        Returns the asset type as a Python class (e.g., Mesh, Image, StandardMaterial).
        """
    def id(self) -> int:
        """Get a unique identifier for this handle.

        For UUID-based handles, returns the UUID as u128.
        For Index-based strong handles, returns the index bits.

        Note: This is primarily useful for comparing handles or using them in sets/dicts.
        """
    def is_strong(self) -> bool:
        """Check if this is a strong handle (keeps the asset alive)."""
    def is_weak(self) -> bool:
        """Check if this is a weak handle (does not keep the asset alive)."""

class Assets(Resource, Generic[A]):
    @overload
    def add(self, asset: Meshable) -> Handle[Mesh]: ...
    @overload
    def add(self, asset: MeshBuilder) -> Handle[Mesh]: ...
    @overload
    def add(self: Assets[StandardMaterial], asset: Color) -> Handle[StandardMaterial]: ...
    @overload
    def add(self: Assets[ColorMaterial], asset: Color) -> Handle[ColorMaterial]: ...
    @overload
    def add(self, asset: A) -> Handle[A]: ...
    def contains(self, id: Handle[A]) -> bool: ...
    def get(self, id: Handle[A]) -> A | None:
        """Get immutable reference to asset (from Res[Assets[T]]).

        Returns borrowed reference to asset in storage, or None if not found.
        Reference is valid only during current system execution.

        Raises:
            RuntimeError: If called on ResMut[Assets[T]] (use get_mut instead)
        """
    def get_mut(self, id: Handle[A]) -> A | None:
        """Get mutable reference to asset (from ResMut[Assets[T]]).

        Returns borrowed mutable reference to asset in storage, or None if not found.
        Reference is valid only during current system execution.

        Example:
            def system(materials: ResMut[Assets[StandardMaterial]]):
                mat = materials.get_mut(handle)
                if mat:
                    mat.base_color = Color.srgb(1.0, 0.0, 0.0)

        Raises:
            RuntimeError: If called on Res[Assets[T]] (use get instead)
        """
    def is_empty(self) -> bool: ...
    def __iter__(self) -> Iterator[tuple[Handle[A], A]]: ...
    def len(self) -> int: ...
    def remove(self, id: Handle[A]) -> None | A: ...

class AssetIter(Iterator[tuple[Handle[A], A]]):
    """Iterator over (handle, asset) pairs in an Assets collection."""
    def __next__(self) -> tuple[Handle[A], A]: ...
    def __iter__(self) -> AssetIter[A]: ...

class AssetServer(Resource):
    def load(self, path: str | AssetPath, asset_type: type[A]) -> Handle[A]: ...

    def load_scene(self, path: str | AssetPath) -> Handle[Scene]: ...
    def load_image(self, path: str | AssetPath) -> Handle[Image]: ...
    def load_with_settings(self, path: str | AssetPath, asset_type: type[A], settings: ImageLoaderSettings) -> Handle[A]:
        """Load an asset with custom loader settings.

        Equivalent to Bevy's `asset_server.load_with_settings::<A, S>()`.

        Currently supported asset types and their settings:
        - Image + ImageLoaderSettings (sampler mode, sRGB, format)

        Args:
            path: Path to the asset file (relative to assets directory)
            asset_type: The asset type class (e.g. Image)
            settings: Loader settings matching the asset type

        Example:
            ```python
            from pybevy.image import ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor
            from pybevy.image import ImageAddressMode

            settings = ImageLoaderSettings(sampler=ImageSampler.descriptor(
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

    def load_state(self, id: Handle[A]) -> LoadState:
        """Get the load state for the given asset handle.

        Returns `LoadState.NotLoaded` if the asset is not tracked.

        Args:
            id: The asset handle to check

        Returns:
            The current load state of the asset
        """

    def is_loaded(self, id: Handle[A]) -> bool:
        """Check if the asset is loaded (but not necessarily its dependencies).

        Returns `True` if the asset's load state is `LoadState.Loaded`.

        Args:
            id: The asset handle to check

        Returns:
            True if the asset is loaded, False otherwise
        """

    def is_loaded_with_dependencies(self, id: Handle[A]) -> bool:
        """Check if the asset and all of its dependencies are loaded.

        Returns `True` if the asset and all recursive dependencies have finished loading.
        This is the method you typically want to use before transitioning to a new scene.

        Args:
            id: The asset handle to check

        Returns:
            True if the asset and all dependencies are loaded, False otherwise

        Example:
            ```python
            def check_loading(
                asset_server: Res[AssetServer],
                scene_handle: Res[SceneHandle]
            ):
                if asset_server.is_loaded_with_dependencies(scene_handle.0):
                    print("Scene fully loaded!")
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
            handle1 = asset_server.load("model.gltf", Scene)

            # Later, retrieve the same handle without re-loading
            handle2 = asset_server.get_handle("model.gltf", Scene)
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
    def parse(path: str) -> AssetPath: ...
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

class AssetEventType(Enum):
    """The type of asset event that occurred."""

    Added = ...
    """Asset was added to the asset storage."""

    Modified = ...
    """Asset was modified in the asset storage."""

    Removed = ...
    """Asset was removed from the asset storage."""

    Unused = ...
    """Asset is no longer used (all handles dropped)."""

    LoadedWithDependencies = ...
    """Asset and all its dependencies finished loading."""

class AssetEvent(Message):
    """Event fired when an asset's state changes.

    AssetEvents are fired during the asset lifecycle for operations like
    loading, reloading, and unloading assets. Use MessageReader[AssetEvent]
    in systems to respond to asset state changes.

    Example:
        ```python
        def track_image_loads(reader: MessageReader[AssetEvent]):
            for event in reader.read():
                if event.is_loaded_with_dependencies():
                    print(f"Image loaded: {event.handle}")
        ```

    Attributes:
        handle: The asset handle for this event
        event_type: The type of asset event
    """

    handle: Handle[Asset]
    event_type: AssetEventType

    def is_added(self) -> bool:
        """Check if this is an Added event (asset was loaded)."""

    def is_modified(self) -> bool:
        """Check if this is a Modified event (asset was reloaded/changed)."""

    def is_removed(self) -> bool:
        """Check if this is a Removed event (asset was unloaded)."""

    def is_unused(self) -> bool:
        """Check if this is an Unused event (last strong handle dropped)."""

    def is_loaded_with_dependencies(self) -> bool:
        """Check if this is a LoadedWithDependencies event (asset and all dependencies loaded)."""

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
    def __hash__(self) -> int: ...
