from typing import ClassVar, Literal

import numpy as np

from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.color import Color
from pybevy.ecs import Batchable, Component
from pybevy.image import Image, TextureAtlas
from pybevy.math import Rect, Vec2

class ColorMaterialPlugin(Plugin):
    """Plugin for 2D ColorMaterial rendering."""
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class ColorMaterial(Asset):
    """Simple 2D material with color and optional texture."""
    def __init__(
        self,
        color: Color = Color.WHITE,
        texture: Handle[Image] | None = None,
        alpha_mode: AlphaMode2d | None = None,
    ) -> None: ...
    color: Color
    texture: Handle[Image] | None
    alpha_mode: AlphaMode2d

class SpritePlugin(Plugin):
    """Plugin that enables 2D sprite rendering.

    Adds systems for rendering Sprite components. This plugin is already
    included in DefaultPlugins, so you typically don't need to add it manually.

    Example:
        ```python
        # Usually not needed - included in DefaultPlugins
        app.add_plugins(SpritePlugin())
        ```
    """

    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class AlphaMode2d:
    """Transparency mode for 2D sprites and materials."""

    class Opaque(AlphaMode2d):
        """Ignore base-color alpha and render fully opaque."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Mask(AlphaMode2d):
        """Use binary transparency at ``threshold``."""
        __match_args__: ClassVar[tuple[Literal["threshold"]]]
        threshold: float
        def __init__(self, threshold: float) -> None: ...

    class Blend(AlphaMode2d):
        """Use standard alpha blending."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

class SpriteImageMode:
    """Sprite image rendering mode configuration.

    The nested subclasses mirror Bevy's ``Auto``, ``Scale``, ``Sliced``, and
    ``Tiled`` variants. Their fields are available for inspection and Python
    structural pattern matching.

    Examples:
        ```python
        from pybevy.sprite import BorderRect, SpriteImageMode, SpriteScalingMode, TextureSlicer

        auto = SpriteImageMode.Auto()
        scaled = SpriteImageMode.Scale(SpriteScalingMode.FitCenter)
        sliced = SpriteImageMode.Sliced(TextureSlicer(BorderRect.all(10.0)))
        tiled = SpriteImageMode.Tiled(tile_y=False, stretch_value=2.0)

        match scaled:
            case SpriteImageMode.Scale(mode):
                print(mode)
        ```
    """

    class Auto(SpriteImageMode):
        """Render the image without explicit scaling or slicing."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Scale(SpriteImageMode):
        """Scale the image according to ``mode``."""
        __match_args__: ClassVar[tuple[Literal["mode"]]]
        mode: SpriteScalingMode
        def __init__(self, mode: SpriteScalingMode) -> None: ...

    class Sliced(SpriteImageMode):
        """Apply nine-patch slicing using ``slicer``."""
        __match_args__: ClassVar[tuple[Literal["slicer"]]]
        slicer: TextureSlicer
        def __init__(self, slicer: TextureSlicer) -> None: ...

    class Tiled(SpriteImageMode):
        """Repeat the image along the enabled axes."""
        __match_args__: ClassVar[tuple[Literal["tile_x"], Literal["tile_y"], Literal["stretch_value"]]]
        tile_x: bool
        tile_y: bool
        stretch_value: float
        def __init__(
            self,
            tile_x: bool = True,
            tile_y: bool = True,
            stretch_value: float = 1.0,
        ) -> None: ...

    def uses_slices(self) -> bool:
        """Check if this mode uses slices internally.

        Returns True if the mode is ``Sliced`` or ``Tiled``, which both use
        internal slicing for rendering.

        Returns:
            True if mode is Sliced or Tiled, False otherwise

        Examples:
            ```python
            assert not SpriteImageMode.Auto().uses_slices()

            sliced = SpriteImageMode.Sliced(TextureSlicer(BorderRect.all(10.0)))
            assert sliced.uses_slices()

            tiled = SpriteImageMode.Tiled()
            assert tiled.uses_slices()
            ```
        """

    def scale(self) -> SpriteScalingMode | None:
        """Get the scaling mode if this is a Scale variant.

        Returns the SpriteScalingMode if this SpriteImageMode is the Scale variant,
        otherwise returns None.

        Returns:
            SpriteScalingMode if this is Scale variant, None otherwise

        Examples:
            ```python
            mode = SpriteImageMode.Scale(SpriteScalingMode.FillCenter)
            assert mode.scale() == SpriteScalingMode.FillCenter

            assert SpriteImageMode.Auto().scale() is None
            ```
        """

class Anchor(Component):
    """Normalized offset of a 2D sprite from its Transform position.

    Controls which point of the sprite aligns with its Transform position.
    Also known as "pivot point" or "origin". Values are normalized relative
    to sprite size: (-0.5, -0.5) = bottom-left, (0, 0) = center, (0.5, 0.5) = top-right.

    **Coordinate System:**
    - X axis: -0.5 (left) → 0.0 (center) → 0.5 (right)
    - Y axis: -0.5 (bottom) → 0.0 (center) → 0.5 (top)
    - Default: CENTER (0, 0)

    Examples:
        ```python
        from pybevy.sprite import Sprite, Anchor
        from pybevy.transform import Transform

        # Sprite positioned at (100, 50) with center as anchor
        app.spawn((
            Sprite.from_image(image),
            Transform.from_xyz(100.0, 50.0, 0.0),
            Anchor.CENTER()  # (0, 0) - sprite center at (100, 50)
        ))

        # Bottom-left corner at (100, 50)
        app.spawn((
            Sprite.from_image(image),
            Transform.from_xyz(100.0, 50.0, 0.0),
            Anchor.BOTTOM_LEFT()  # (-0.5, -0.5)
        ))

        # Custom anchor point
        app.spawn((
            Sprite.from_image(image),
            Transform.from_xyz(100.0, 50.0, 0.0),
            Anchor.custom(Vec2(0.25, -0.25))  # Right of center, below center
        ))
        ```

    Notes:
        - Anchor is automatically added to all Sprite entities (required component)
        - Affects sprite positioning, rotation, and scaling pivot point
        - Use BOTTOM_LEFT for platformer characters (ground position)
        - Use CENTER for projectiles and effects (center position)

    See Also:
        - Transform: Defines the world position that the anchor aligns to
        - Sprite: The visual component that Anchor positions
    """

    CENTER: ClassVar[Anchor]
    BOTTOM_LEFT: ClassVar[Anchor]
    BOTTOM_CENTER: ClassVar[Anchor]
    BOTTOM_RIGHT: ClassVar[Anchor]
    CENTER_LEFT: ClassVar[Anchor]
    CENTER_RIGHT: ClassVar[Anchor]
    TOP_LEFT: ClassVar[Anchor]
    TOP_CENTER: ClassVar[Anchor]
    TOP_RIGHT: ClassVar[Anchor]

    def __init__(self, value: Vec2 = ...) -> None:
        """Create an anchor from a Vec2.

        Args:
            value: Normalized anchor position where:
                   x: -0.5 (left) to 0.5 (right)
                   y: -0.5 (bottom) to 0.5 (top)
        """

    @staticmethod
    def custom(value: Vec2) -> Anchor:
        """Create a custom anchor at the specified normalized position.

        Args:
            value: Normalized anchor position:
                   - (0, 0) = center
                   - (-0.5, -0.5) = bottom-left
                   - (0.5, 0.5) = top-right
                   - Any value in between for custom positioning

        Returns:
            Anchor at the specified position

        Examples:
            ```python
            # One-quarter from left, one-quarter from bottom
            anchor = Anchor.custom(Vec2(-0.25, -0.25))

            # Slightly above center
            anchor = Anchor.custom(Vec2(0.0, 0.1))
            ```
        """

    @property
    def value(self) -> Vec2:
        """Get the normalized anchor position as a Vec2.

        Returns:
            Vec2 containing the anchor's (x, y) normalized coordinates
        """

    def as_vec(self) -> Vec2:
        """Get the anchor position as a Vec2.

        Returns:
            Vec2 containing the anchor's (x, y) coordinates
        """

class BorderRect:
    """Defines border sizes for nine-patch texture slicing.

    Uses two Vec2 fields matching Bevy's BorderRect:
    - min_inset: Vec2(left, bottom) - inset from the minimum corner
    - max_inset: Vec2(right, top) - inset from the maximum corner

    Examples:
        ```python
        from pybevy.sprite import BorderRect, TextureSlicer

        # Uniform 10px border on all sides
        border = BorderRect.all(10.0)

        # Different borders using min/max insets
        border = BorderRect(
            min_inset=Vec2(8.0, 16.0),   # left=8, bottom=16
            max_inset=Vec2(8.0, 16.0),   # right=8, top=16
        )

        # Symmetric horizontal and vertical borders
        border = BorderRect.axes(horizontal=12.0, vertical=8.0)
        ```
    """

    min_inset: Vec2
    """Inset from the minimum corner (left, bottom) in pixels."""

    max_inset: Vec2
    """Inset from the maximum corner (right, top) in pixels."""

    def __init__(self, min_inset: Vec2 = ..., max_inset: Vec2 = ...) -> None:
        """Create a BorderRect with min/max inset vectors.

        Args:
            min_inset: Inset from the minimum corner (left, bottom) in pixels
            max_inset: Inset from the maximum corner (right, top) in pixels
        """

    @staticmethod
    def all(inset: float) -> BorderRect:
        """Create a BorderRect with the same size for all borders.

        Args:
            inset: Border size in pixels for all four sides
        """

    @staticmethod
    def axes(horizontal: float, vertical: float) -> BorderRect:
        """Create a BorderRect with symmetric horizontal and vertical borders.

        Args:
            horizontal: Border size for left and right edges in pixels
            vertical: Border size for top and bottom edges in pixels
        """

    def __eq__(self, other: object) -> bool:
        """Compare two BorderRect instances for equality.

        Args:
            other: Another BorderRect to compare against

        Returns:
            True if all border values are equal, False otherwise
        """

class SpriteScalingMode:
    """Image scaling modes for `SpriteImageMode.Scale()`.

    Controls how a sprite's image scales to fit its display size while
    maintaining aspect ratio. Choose between filling the area (may crop)
    or fitting within it (may letterbox), with alignment options.

    **Fill vs Fit:**
    - **Fill**: Scale to fill entire sprite area (may crop edges)
    - **Fit**: Scale to fit within sprite area (may have empty space)

    **Alignment:**
    - **Center**: Centered alignment (default for most use cases)
    - **Start**: Align to top-left
    - **End**: Align to bottom-right

    Examples:
        ```python
        from pybevy.sprite import Sprite, SpriteImageMode, SpriteScalingMode

        # Fit image within bounds, centered (letterbox if needed)
        sprite.image_mode = SpriteImageMode.Scale(SpriteScalingMode.FitCenter)

        # Fill entire area, may crop edges
        sprite.image_mode = SpriteImageMode.Scale(SpriteScalingMode.FillCenter)

        # Fit with top-left alignment
        sprite.image_mode = SpriteImageMode.Scale(SpriteScalingMode.FitStart)

        # Fill with bottom-right alignment
        sprite.image_mode = SpriteImageMode.Scale(SpriteScalingMode.FillEnd)
        ```

    Notes:
        - Useful for responsive UI elements that need to adapt to different sizes
        - FitCenter is best for preserving entire image (like photos in a frame)
        - FillCenter is best for backgrounds (crop edges if needed)
        - Start/End alignment useful for multi-image layouts

    See Also:
        - SpriteImageMode.Scale(): Applies scaling mode to sprite rendering
    """

    FillCenter: ClassVar[SpriteScalingMode]
    """Scale to fill sprite area, centered. May crop edges to maintain aspect ratio."""

    FillStart: ClassVar[SpriteScalingMode]
    """Scale to fill sprite area, aligned to top-left. May crop edges."""

    FillEnd: ClassVar[SpriteScalingMode]
    """Scale to fill sprite area, aligned to bottom-right. May crop edges."""

    FitCenter: ClassVar[SpriteScalingMode]
    """Scale to fit within sprite area, centered. May add letterboxing to maintain aspect ratio."""

    FitStart: ClassVar[SpriteScalingMode]
    """Scale to fit within sprite area, aligned to top-left. May add letterboxing."""

    FitEnd: ClassVar[SpriteScalingMode]
    """Scale to fit within sprite area, aligned to bottom-right. May add letterboxing."""

class SliceScaleMode:
    """Scaling mode for nine-patch texture slices."""

    class Stretch(SliceScaleMode):
        """Stretch the slice to fit the target area."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Tile(SliceScaleMode):
        """Repeat the slice after the drawing-to-source ratio reaches the threshold."""
        __match_args__: ClassVar[tuple[Literal["stretch_value"]]]
        stretch_value: float
        def __init__(self, stretch_value: float = 1.0) -> None: ...

    def __eq__(self, other: object) -> bool: ...

class TextureSlicer:
    """Configuration for nine-patch texture slicing.

    Defines how a texture is divided into 9 regions and how each region
    scales when the sprite is resized. Essential for creating scalable
    UI elements like buttons, panels, and frames.

    **Nine Regions:**
    - 4 Corners: Never scale (maintain size)
    - 4 Edges: Scale in one direction (horizontal or vertical)
    - 1 Center: Scales in both directions

    **Scaling Options:**
    - Stretch: Scale uniformly (may distort)
    - Tile: Repeat pattern (maintains appearance)

    Args:
        border: Defines the pixel widths/heights of non-stretchable borders
        center_tile: If True, tile the center region instead of stretching
        center_stretch_value: Tiling multiplier for center (only if center_tile=True)
        sides_tile: If True, tile the edge regions instead of stretching
        sides_stretch_value: Tiling multiplier for edges (only if sides_tile=True)
        max_corner_scale: Maximum scale factor for corners (default: 1.0 = no scaling)

    Examples:
        ```python
        from pybevy.sprite import TextureSlicer, BorderRect, SpriteImageMode

        # Simple button with 10px borders, stretch all regions
        border = BorderRect.all(10.0)
        slicer = TextureSlicer(border)
        sprite.image_mode = SpriteImageMode.Sliced(slicer)

        # Panel with tiled center for patterned background
        slicer = TextureSlicer(
            border=BorderRect.all(16.0),
            center_tile=True,
            center_stretch_value=1.0
        )

        # Ornate frame with tiled borders
        slicer = TextureSlicer(
            border=BorderRect(12.0, 12.0, 16.0, 16.0),
            sides_tile=True,
            sides_stretch_value=1.0
        )

        # Allow corners to scale up to 2x for very large sprites
        slicer = TextureSlicer(
            border=BorderRect.all(8.0),
            max_corner_scale=2.0
        )
        ```

    Notes:
        - Texture must be designed for nine-patch slicing (clean borders)
        - Tiled regions should use tileable patterns
        - Center tiling is common for patterned backgrounds
        - Edge tiling is less common (usually stretched)
        - max_corner_scale > 1.0 allows corners to grow for very large sprites

    See Also:
        - BorderRect: Defines the border dimensions
        - SliceScaleMode: Controls stretch vs tile behavior
        - SpriteImageMode.Sliced(): Applies nine-patch rendering to sprites
    """

    border: BorderRect
    max_corner_scale: float
    center_scale_mode: SliceScaleMode
    sides_scale_mode: SliceScaleMode

    def __init__(
        self,
        border: BorderRect = ...,
        center_tile: bool = False,
        center_stretch_value: float = 1.0,
        sides_tile: bool = False,
        sides_stretch_value: float = 1.0,
        max_corner_scale: float = 1.0,
        center_scale_mode: SliceScaleMode | None = None,
        sides_scale_mode: SliceScaleMode | None = None,
    ) -> None:
        """Create a TextureSlicer for nine-patch rendering.

        Args:
            border: Border sizes defining the nine regions
            center_tile: If True, tile center region instead of stretching
            center_stretch_value: Tiling multiplier for center (when center_tile=True)
            sides_tile: If True, tile edge regions instead of stretching
            sides_stretch_value: Tiling multiplier for edges (when sides_tile=True)
            max_corner_scale: Max corner scale factor (1.0 = maintain original size)
            center_scale_mode: Explicit scale mode for center (overrides center_tile if set)
            sides_scale_mode: Explicit scale mode for sides (overrides sides_tile if set)
        """

    def __eq__(self, other: object) -> bool:
        """Compare two TextureSlicer instances for equality.

        Args:
            other: Another TextureSlicer to compare against

        Returns:
            True if all properties are equal, False otherwise
        """

class Sprite(Component):
    """2D sprite component for rendering images and textures.

    The core component for displaying 2D graphics. Sprites can render images,
    texture atlases (sprite sheets), solid colors, or nine-patch UI elements.
    Automatically includes Transform, Visibility, and Anchor components.

    **Required Components (auto-added):**
    - Transform: World position, rotation, scale
    - Visibility: Culling and rendering control
    - Anchor: Sprite pivot point

    **Common Use Cases:**
    - Characters and enemies (from texture atlases)
    - Backgrounds and tiles (tiled or stretched)
    - UI elements (nine-patch slicing)
    - Solid color shapes and effects

    Args:
        image: Handle to the image asset to display
        color: Color tint applied to the sprite (default: white = no tint)
        flip_x: Flip sprite horizontally (mirror on X axis)
        flip_y: Flip sprite vertically (mirror on Y axis)
        custom_size: Override size in pixels (None = use image size)
        rect: Render only a sub-rectangle of the image
        texture_atlas: Texture atlas for sprite sheet rendering

    Examples:
        ```python
        from pybevy.sprite import Sprite, Anchor
        from pybevy.transform import Transform
        from pybevy.color import Color

        # Simple sprite from an image
        app.spawn((
            Sprite.from_image(image_handle),
            Transform.from_xyz(100.0, 50.0, 0.0),
        ))

        # Sprite with custom size and color tint
        app.spawn((
            Sprite(
                image=image_handle,
                color=Color.srgb(1.0, 0.5, 0.5),  # Red tint
                custom_size=(64.0, 64.0),
                flip_x=True
            ),
            Transform.from_xyz(200.0, 100.0, 0.0),
        ))

        # Solid color rectangle
        app.spawn((
            Sprite.from_color(Color.srgb(0.2, 0.4, 0.8), (100.0, 50.0)),
            Transform.from_xyz(0.0, 0.0, 0.0),
        ))

        # Sprite from texture atlas (sprite sheet)
        atlas = TextureAtlas(...)
        app.spawn((
            Sprite.from_atlas_image(image_handle, atlas),
            Transform.from_xyz(0.0, 0.0, 0.0),
        ))
        ```

    Notes:
        - Sprites are rendered by the `SpritePlugin` (included in DefaultPlugins)
        - Use `custom_size` to scale sprites independently of Transform.scale
        - `rect` is useful for sprite sheets without creating a TextureAtlas
        - Color tint multiplies with image colors (white = no change)
        - Anchor defaults to CENTER (0, 0) if not specified

    See Also:
        - Transform: Controls sprite position, rotation, and scale
        - Anchor: Controls sprite pivot point
        - TextureAtlas: For sprite sheet animation
        - SpriteImageMode: For advanced scaling and slicing
    """

    image: Handle[Image]
    """Handle to the image asset to display."""

    texture_atlas: TextureAtlas | None
    """Optional texture atlas for sprite sheet rendering."""

    color: Color
    """Color tint multiplied with image colors (white = no tint)."""

    flip_x: bool
    """Flip sprite horizontally (mirror on X axis)."""

    flip_y: bool
    """Flip sprite vertically (mirror on Y axis)."""

    custom_size: tuple[float, float] | None
    """Override size in pixels (None = use image size)."""

    rect: Rect | None
    """Render only a sub-rectangle of the image (None = full image)."""

    image_mode: SpriteImageMode
    """How the sprite image is scaled/tiled (default: Auto)."""

    def __init__(
        self,
        image: Handle[Image],
        color: Color = Color.WHITE,
        flip_x: bool = False,
        flip_y: bool = False,
        custom_size: tuple[float, float] | None = None,
        rect: Rect | None = None,
        texture_atlas: TextureAtlas | None = None,
        image_mode: SpriteImageMode = ...,
    ) -> None:
        """Create a sprite with the specified configuration.

        Args:
            image: Handle to the image asset
            color: Color tint (default: white = no tint)
            flip_x: Mirror horizontally
            flip_y: Mirror vertically
            custom_size: Size in pixels (None = use image dimensions)
            rect: Sub-rectangle to render (None = full image)
            texture_atlas: Texture atlas for sprite sheets
            image_mode: How the sprite image is scaled/tiled (default: Auto)
        """

    @staticmethod
    def from_image(image: Handle[Image]) -> Sprite:
        """Create a sprite from an image handle.

        Simplest way to create a sprite. Uses image's natural size,
        white color (no tint), and auto image mode.

        Args:
            image: Handle to the image asset to display

        Returns:
            New Sprite with default settings

        Examples:
            ```python
            # Load and display an image
            def setup(commands: Commands, assets: Res[AssetServer]):
                image = assets.load("character.png")
                commands.spawn((
                    Sprite.from_image(image),
                    Transform.from_xyz(0.0, 0.0, 0.0),
                ))
            ```
        """

    @staticmethod
    def from_atlas_image(image: Handle[Image], atlas: TextureAtlas) -> Sprite:
        """Create a sprite from an image with a texture atlas.

        For rendering sprite sheets where multiple sprites are packed
        into a single image. The atlas defines which region to display.

        Args:
            image: Handle to the image asset containing the sprite sheet
            atlas: TextureAtlas defining which region to display

        Returns:
            New Sprite configured for atlas rendering

        Examples:
            ```python
            # Display one frame from a sprite sheet
            def setup(commands: Commands, assets: Res[AssetServer]):
                image = assets.load("sprites.png")
                atlas = TextureAtlas(index=0, layout=layout_handle)
                commands.spawn((
                    Sprite.from_atlas_image(image, atlas),
                    Transform.from_xyz(0.0, 0.0, 0.0),
                ))
            ```
        """

    @staticmethod
    def from_color(color: Color, size: Vec2) -> Sprite:
        """Create a solid-color sprite.

        Renders a filled rectangle with the specified color. Useful for
        colored shapes, debug visualization, or simple UI elements.

        Args:
            color: Color to fill the sprite with
            size: Size of the sprite in world units (width, height)

        Returns:
            New Sprite filled with the specified color

        Examples:
            ```python
            # Red rectangle 100x50
            app.spawn((
                Sprite.from_color(Color.srgb(1.0, 0.0, 0.0), Vec2(100.0, 50.0)),
                Transform.from_xyz(0.0, 0.0, 0.0),
            ))

            # Semi-transparent blue square
            app.spawn((
                Sprite.from_color(Color.srgba(0.0, 0.0, 1.0, 0.5), Vec2(50.0, 50.0)),
                Transform.from_xyz(100.0, 0.0, 0.0),
            ))
            ```
        """

    @staticmethod
    def sized(custom_size: Vec2) -> Sprite:
        """Create a sprite with a custom size.

        Creates a sprite that will be rendered at the specified size
        regardless of the source image dimensions. Useful when you
        know the desired size before loading the image.

        Args:
            custom_size: Size of the sprite in pixels (width, height)

        Returns:
            New Sprite with the specified size

        Examples:
            ```python
            # Create a 64x64 sprite (image will be scaled to fit)
            app.spawn((
                Sprite.sized((64.0, 64.0)),
                Transform.from_xyz(0.0, 0.0, 0.0),
            ))
            ```
        """

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        flip_x: np.typing.ArrayLike | None = None,
        flip_y: np.typing.ArrayLike | None = None,
        color: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

    def as_asset_id(self) -> Handle[Image]:
        """Get the AssetId of this sprite's image.

        Returns a handle representing the AssetId of the sprite's image.
        This is useful for comparing sprites or looking up assets.

        Returns:
            Handle to the sprite's image asset

        Examples:
            ```python
            sprite = Sprite.from_image(image_handle)
            asset_id = sprite.as_asset_id()
            ```
        """
