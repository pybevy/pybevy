from collections.abc import Buffer
from enum import Enum
from typing import ClassVar, Literal

import numpy as np

from pybevy.app import App, Plugin
from pybevy.array import Array
from pybevy.assets import Asset, Handle
from pybevy.collections import LiveList
from pybevy.color import Color
from pybevy.math import URect, UVec2, UVec3, Vec2
from pybevy.render import (
    Extent3d,
    TextureDimension,
    TextureFormat,
    TextureViewDimension,
)

class ImageFormat(Enum):
    """Image encoding format for saving/exporting images.

    Supported formats for encoding Bevy images to byte buffers or files.
    """
    Png = "Png"
    """PNG format - lossless compression, supports transparency"""

    Jpeg = "Jpeg"
    """JPEG format - lossy compression, good for photos (no transparency)"""

    Bmp = "Bmp"
    """BMP format - uncompressed, large file sizes"""

    Tiff = "Tiff"
    """TIFF format - flexible format supporting various compression methods"""

    Tga = "Tga"
    """TGA (Targa) format - simple format with optional compression"""

    WebP = "WebP"
    """WebP format - modern format with good compression and quality"""

    Gif = "Gif"
    """GIF format - supports animation and transparency, limited to 256 colors"""

    Ico = "Ico"
    """ICO format - Windows icon format, supports multiple sizes"""

    Pnm = "Pnm"
    """PNM format - Portable Anymap format family (PBM, PGM, PPM)"""

    Qoi = "Qoi"
    """QOI (Quite OK Image) format - fast lossless compression"""

    Hdr = "Hdr"
    """HDR (Radiance) format - high dynamic range images"""

    Dds = "Dds"
    """DDS (DirectDraw Surface) format - GPU-compressed textures"""

    OpenExr = "OpenExr"
    """OpenEXR format - high dynamic range images for VFX"""

    Ktx2 = "Ktx2"
    """KTX2 (Khronos Texture) format - GPU-optimized texture container"""

    Farbfeld = "Farbfeld"
    """Farbfeld format - simple lossless format with 16-bit RGBA"""

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class ImageCompareFunction(Enum):
    Never = "Never"
    Less = "Less"
    Equal = "Equal"
    LessEqual = "LessEqual"
    Greater = "Greater"
    NotEqual = "NotEqual"
    GreaterEqual = "GreaterEqual"
    Always = "Always"

    def __hash__(self) -> int: ...

class ImageSamplerBorderColor(Enum):
    TransparentBlack = "TransparentBlack"
    OpaqueBlack = "OpaqueBlack"
    OpaqueWhite = "OpaqueWhite"
    Zero = "Zero"

    def __hash__(self) -> int: ...

class ImageFormatSetting:
    class FromExtension(ImageFormatSetting):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Guess(ImageFormatSetting):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Format(ImageFormatSetting):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: ImageFormat
        def __init__(self, value: ImageFormat) -> None: ...

class SaveImageFormatSetting:
    """How ImageSaver picks the file format: explicit, or from the path extension."""

    class FromExtension(SaveImageFormatSetting):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Format(SaveImageFormatSetting):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: ImageFormat
        def __init__(self, value: ImageFormat) -> None: ...

class ImageSaverSettings:
    """Settings for AssetServer.save_image. Default format is SaveImageFormatSetting.FromExtension."""

    def __init__(self, format: SaveImageFormatSetting = SaveImageFormatSetting.FromExtension()) -> None: ...
    @property
    def format(self) -> SaveImageFormatSetting: ...
    @format.setter
    def format(self, value: SaveImageFormatSetting) -> None: ...

class RenderAssetUsages:
    """Asset usage flags indicating which worlds can access this asset.

    Combine flags with `|`: `RenderAssetUsages.MAIN_WORLD | RenderAssetUsages.RENDER_WORLD`.
    """

    MAIN_WORLD: ClassVar[RenderAssetUsages]
    RENDER_WORLD: ClassVar[RenderAssetUsages]

    def __init__(self) -> None:
        """Create the default usage flags, `MAIN_WORLD | RENDER_WORLD` (matches bevy's `RenderAssetUsages::default()`)."""

    def __or__(self, other: RenderAssetUsages) -> RenderAssetUsages: ...
    def __eq__(self, other: object) -> bool: ...
    def contains(self, other: RenderAssetUsages) -> bool:
        """Whether all flags in `other` are set on this value."""

class ImagePlugin(Plugin):
    """Plugin that adds Image asset support and configures texture loading.

    Registers the Image asset type and sets up asset loaders for various
    image formats (PNG, JPEG, BMP, etc.). Also configures the default
    texture sampler used when ImageSampler is set to Default.

    **Plugin Groups:**
    - Included in DefaultPlugins
    - Included in MinimalPlugins

    **Default Configuration:**
    - Default sampler: Linear filtering (smooth textures)
    - Supports all enabled image format features

    Examples:
        ```python
        from pybevy.app import App
        from pybevy.image import ImagePlugin

        # Basic usage (linear filtering by default)
        app = App().add_plugins(ImagePlugin())

        # Usually added via DefaultPlugins
        from pybevy.app import DefaultPlugins
        app = App().add_plugins(DefaultPlugins)
        ```

    Notes:
        - Automatically registers asset loaders for enabled image formats
        - The default sampler affects all images using ImageSampler.Default
        - For nearest-neighbor (pixel-perfect) filtering, use a custom config
    """
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class ImageDataContext:
    """Context manager for read-only zero-copy access to image pixel data.

    Yields a bounded uint8 array that borrows the image's pixel buffer without
    copying. On context exit the array closes and all later data access raises.

    Example:
        ```python
        with image.data() as pixels:
            mean = pixels.mean()
        ```
    """
    def __enter__(self) -> Array:
        """Enter the context and return the read-only bounded array."""

    def __exit__(self, exc_type, exc_val, exc_tb) -> bool:
        """Close the array and release the read borrow."""

class ImageDataContextMut:
    """Context manager for mutable zero-copy access to image pixel data.

    Yields a mutable bounded uint8 array over the image's pixel buffer.
    Modifications are immediately reflected in the image data. On exit the
    array closes and the exclusive write borrow is released.

    Example:
        ```python
        with image.data_mut() as pixels:
            pixels[:] = 255  # Modify in-place
        ```
    """
    def __enter__(self) -> Array:
        """Enter the context and return the mutable bounded array."""

    def __exit__(self, exc_type, exc_val, exc_tb) -> bool:
        """Close the array and release the write borrow."""

class ImagePixelContextMut:
    """Context manager for mutable zero-copy access to a single pixel's bytes.

    Yields a mutable bounded uint8 array over one pixel's bytes. Its length
    equals the pixel size (e.g., 4 for RGBA8), and it closes on context exit.

    Example:
        ```python
        from pybevy.math import UVec3

        with image.pixel_bytes_mut(UVec3(x, y, 0)) as pixel:
            pixel[0] = 255  # Modify R channel
        ```
    """
    def __enter__(self) -> Array:
        """Enter the context and return the mutable bounded pixel array."""

    def __exit__(self, exc_type, exc_val, exc_tb) -> bool:
        """Close the array and release the write borrow."""

class Image(Asset):
    """GPU texture asset storing pixel data and texture configuration.

    Represents a texture that can be used for rendering, loaded from files,
    or created procedurally. Supports various texture formats, dimensions,
    and mipmap levels.

    **Common Use Cases:**
    - Loading images from files (PNG, JPEG, etc.)
    - Creating procedural textures
    - Render targets for post-processing
    - Sprite sheet textures with TextureAtlas
    - UI textures and fonts

    Examples:
        ```python
        from pybevy.image import Image
        from pybevy.render import Extent3d, TextureFormat
        from pybevy.color import Color

        # Create a basic texture
        img = Image(Extent3d(256, 256, 1))

        # Create a filled texture
        img = Image.new_fill(
            Extent3d(64, 64, 1),
            [255, 0, 0, 255]  # Red RGBA pixel
        )

        # Load from file (via asset system)
        # In a system with asset_server: Res[AssetServer]
        # handle = asset_server.load("bevy/textures/sprite.png")
        ```

    See Also:
        - TextureAtlas: For sprite sheet rendering
        - ImagePlugin: Required plugin for image loading
    """
    def __init__(
        self,
        size: Extent3d = Extent3d(1, 1, 1),
        dimension: TextureDimension | None = None,
        data: Buffer | list[int] | tuple[int, ...] | np.ndarray | Array | None = None,
        format: TextureFormat | None = None,
        asset_usage: RenderAssetUsages | None = None,
    ) -> None:
        """Create a new image, mirroring bevy's `Image::new`.

        Args:
            size: Texture dimensions (width, height, depth_or_array_layers).
                  Default is 1x1x1 single pixel.
            dimension: Texture dimension. Defaults to TextureDimension.D2.
            data: Optional pixel data. If None, fills with max-value bytes
                  (white for 8-bit unorm formats). Must match size and format;
                  raises ValueError on length or shape mismatch. Accepts a
                  byte-format Buffer, a list/tuple of ints, a uint8 NumPy
                  array, or a uint8 bounded Array. Arrays may be flat or use
                  the natural (height, width, bytes_per_pixel) shape.
            format: Texture format. Defaults to TextureFormat.Rgba8UnormSrgb.
            asset_usage: Which worlds keep the asset. Defaults to both
                  (bevy's `RenderAssetUsages::default()`).

        Example:
            ```python
            from pybevy.image import Image
            from pybevy.render import Extent3d

            # Create white 64x64 texture
            img = Image(Extent3d(64, 64, 1))

            # Create with custom data
            pixels = [255, 0, 0, 255] * (64 * 64)  # Red texture
            img = Image(Extent3d(64, 64, 1), data=pixels)
            ```
        """

    @staticmethod
    def new_fill(
        size: Extent3d,
        pixel: Buffer | list[int] | tuple[int, ...] | np.ndarray | Array,
        format: TextureFormat | None = None,
        dimension: TextureDimension | None = None,
    ) -> Image:
        """Create a new image filled with a specific pixel value.

        Args:
            size: Image dimensions (width, height, depth_or_array_layers)
            pixel: Bytes representing a single pixel to fill the image with
            format: Texture format (default: RGBA8_UNORM_SRGB)
            dimension: Texture dimension (default: D2)

        Returns:
            New Image filled with the specified pixel
        """

    @staticmethod
    def transparent() -> Image:
        """Create a 1x1 transparent image.

        Returns:
            New 1x1 transparent Image (RGBA 0, 0, 0, 0)
        """

    @staticmethod
    def new_target_texture(width: int, height: int, format: TextureFormat) -> Image:
        """Create a render target texture optimized for rendering to.

        Creates a 2D texture with usage flags set for render targets:
        - TEXTURE_BINDING: Can be bound as a texture in shaders
        - COPY_DST: Can be copied to
        - RENDER_ATTACHMENT: Can be used as a render target

        Useful for off-screen rendering, post-processing effects, and
        render-to-texture workflows.

        Args:
            width: Width of the render target in pixels
            height: Height of the render target in pixels
            format: Texture format (e.g., RGBA8_UNORM_SRGB)

        Returns:
            New Image configured as a render target, filled with zeroes

        Raises:
            ValueError: If width or height is zero, or the format cannot be
                used for a render target.

        Example:
            >>> # Create a render target for post-processing
            >>> render_target = Image.new_target_texture(1920, 1080, TextureFormat.Rgba8UnormSrgb)
        """

    @staticmethod
    def new_render_target(width: int, height: int) -> Image:
        """Create a render target texture with RGBA8 sRGB format.

        Convenience method that creates a render target with the most commonly
        used format (RGBA8_UNORM_SRGB) and enables GPU readback. Unlike
        `Image.new_target_texture`, this constructor adds `COPY_SRC` usage.

        The texture is configured with proper usage flags for:
        - TEXTURE_BINDING: Can be sampled in shaders
        - COPY_SRC: Can be copied from (for readback)
        - RENDER_ATTACHMENT: Can be used as a render target

        Args:
            width: Width of the render target in pixels
            height: Height of the render target in pixels

        Returns:
            New Image configured as a render target, filled with zeroes

        Raises:
            ValueError: If width or height is zero.

        Example:
            >>> # Create render target for headless rendering
            >>> image = Image.new_render_target(width=800, height=600)
            >>> handle = images.add(image)
            >>> commands.spawn((Camera3d(), Camera(), RenderTarget.Image(ImageRenderTarget(handle))))
        """

    @staticmethod
    def from_buffer(
        buffer: Buffer | list[int] | tuple[int, ...] | np.ndarray | Array,
        is_srgb: bool = True,
    ) -> Image:
        """Load an image from encoded bytes (PNG, JPEG, etc.).

        Args:
            buffer: Encoded image data (PNG, JPEG, BMP, etc.)
            is_srgb: Whether to interpret as sRGB color space (default: True)

        Returns:
            Decoded Image

        Raises:
            RuntimeError: If the buffer cannot be decoded
        """

    # Dimension queries
    def width(self) -> int:
        """Get the width of the image in pixels."""

    def height(self) -> int:
        """Get the height of the image in pixels."""

    def size(self) -> UVec2:
        """Get the 2D size of the image as UVec2.

        Returns:
            UVec2 with (width, height) in pixels.
        """

    def size_f32(self) -> Vec2:
        """Get the 2D size of the image as a Vec2."""

    def aspect_ratio(self) -> float:
        """Get the aspect ratio (width / height) of the image."""

    def is_compressed(self) -> bool:
        """Check if the image format is a compressed format (BC1-7, etc.)."""

    def data_len(self) -> int:
        """Get the length of the pixel data buffer in bytes.

        Returns:
            Size of the pixel data in bytes, or 0 if data is None.

        Example:
            ```python
            from pybevy.image import Image
            from pybevy.render import Extent3d

            img = Image(Extent3d(64, 64, 1))
            # 64 * 64 * 4 bytes (RGBA8)
            print(f"Data size: {img.data_len()} bytes")
            ```
        """

    # Texture descriptor properties
    @property
    def format(self) -> TextureFormat:
        """Get the texture format of this image."""

    @property
    def dimension(self) -> TextureDimension:
        """Get the texture dimension (D1, D2, D3) of this image."""

    @property
    def texture_view_dimension(self) -> TextureViewDimension | None:
        """Explicit dimension used to interpret this image's texture view."""

    @texture_view_dimension.setter
    def texture_view_dimension(self, value: TextureViewDimension | None) -> None: ...

    @property
    def mip_level_count(self) -> int:
        """Get the number of mip levels in this image."""

    @property
    def sampler(self) -> ImageSampler:
        """Get the texture sampler configuration.

        Returns sampler settings like filter modes and address modes.
        """

    @sampler.setter
    def sampler(self, value: ImageSampler) -> None:
        """Set the texture sampler configuration.

        Args:
            value: New sampler configuration (use ImageSampler.linear() or ImageSampler.nearest())

        Example:
            >>> from pybevy.image import Image
            >>> from pybevy.image import ImageSampler
            >>> from pybevy.render import Extent3d
            >>> img = Image(Extent3d(64, 64, 1))
            >>> img.sampler = ImageSampler.linear()  # Smooth filtering
            >>> img.sampler = ImageSampler.nearest()  # Pixel-perfect filtering
        """

    @property
    def asset_usage(self) -> RenderAssetUsages:
        """Get the asset usage flags.

        Indicates which worlds (main/render) can access this asset.
        """
    @asset_usage.setter
    def asset_usage(self, value: RenderAssetUsages) -> None: ...

    @property
    def copy_on_resize(self) -> bool:
        """Whether to copy data when resizing the image.

        If true, pixel data is preserved when resizing.
        If false, pixel data is discarded on resize.
        """

    @copy_on_resize.setter
    def copy_on_resize(self, value: bool) -> None:
        """Set whether to copy data when resizing."""

    def data(self) -> ImageDataContext:
        """Get zero-copy read-only access to image pixel data via context manager.

        Returns a context manager yielding a bounded uint8 array. The array is
        zero-copy but valid only until the context exits.

        Returns:
            Context manager yielding a read-only bounded array

        Example:
            ```python
            from pybevy.image import Image
            from pybevy.render import Extent3d
            img = Image.new_fill(Extent3d(64, 64, 1), [255, 0, 0, 255])

            with img.data() as pixels:
                mean_value = pixels.mean()
                print(f"Mean pixel value: {mean_value}")
            ```

        Notes:
            - Zero-copy: no data is duplicated
            - Array is only valid within the `with` block
            - For mutable access, use `data_mut()`
            - For guaranteed safety across threads, use `data_copy()`
        """

    def data_mut(self) -> ImageDataContextMut:
        """Get zero-copy mutable access to image pixel data via context manager.

        Returns a context manager yielding a mutable bounded uint8 array.
        Modifications are immediately reflected in the image.

        Returns:
            Context manager yielding a mutable bounded array

        Example:
            ```python
            from pybevy.image import Image
            from pybevy.render import Extent3d
            img = Image(Extent3d(64, 64, 1))

            with img.data_mut() as pixels:
                pixels[:] = 255  # Fill entire image with white
            ```

        Notes:
            - Zero-copy: modifies data in-place
            - Changes persist after the `with` block
            - Array is only valid within the `with` block
            - Basic slices, `reshape()`, and `ravel()` share this buffer while
              the context is live, so writes through those views reach the image
            - For read-only access, use `data()`
        """

    def pixel_data_offset(self, coords: UVec3) -> int | None:
        """Compute the byte offset where a specific pixel's data is stored.

        Returns the byte offset into the image data buffer for the pixel at
        the given coordinates.

        Args:
            coords: Pixel coordinates as UVec3(x, y, z) where z is the array layer.
                    For 1D textures, y and z are ignored. For 2D textures, z is the layer.

        Returns:
            Byte offset into the data buffer, or None if coordinates are out of bounds.

        Example:
            ```python
            from pybevy.image import Image
            from pybevy.render import Extent3d
            from pybevy.math import UVec3

            img = Image(Extent3d(64, 64, 1))
            offset = img.pixel_data_offset(UVec3(10, 20, 0))
            if offset is not None:
                # offset = (20 * 64 + 10) * 4 = 5160 for RGBA8
                print(f"Pixel at (10, 20) starts at byte {offset}")
            ```
        """

    def pixel_bytes(self, coords: UVec3) -> bytes | None:
        """Get a copy of the bytes for a specific pixel.

        Returns a copy of the raw byte data for the pixel at the given coordinates.
        The number of bytes depends on the texture format (e.g., 4 bytes for RGBA8).

        Args:
            coords: Pixel coordinates as UVec3(x, y, z) where z is the array layer.

        Returns:
            Copy of the pixel bytes, or None if coordinates are out of bounds or no data.

        Example:
            ```python
            from pybevy.image import Image
            from pybevy.render import Extent3d
            from pybevy.math import UVec3

            img = Image.new_fill(Extent3d(64, 64, 1), [255, 0, 0, 255])  # Red
            pixel = img.pixel_bytes(UVec3(10, 20, 0))
            if pixel is not None:
                r, g, b, a = pixel
                print(f"RGBA: ({r}, {g}, {b}, {a})")
            ```

        Notes:
            - Returns a copy of the data, not a reference
            - For mutable access, use `pixel_bytes_mut()` instead
            - For bulk read access, use `data()` or `data_copy()` instead
        """

    def pixel_bytes_mut(self, coords: UVec3) -> ImagePixelContextMut:
        """Get zero-copy mutable access to a specific pixel's bytes via context manager.

        Returns a context manager yielding a mutable bounded uint8 array. The
        array length equals the pixel size for the image format
        (e.g., 4 bytes for RGBA8).

        Args:
            coords: Pixel coordinates as UVec3(x, y, z) where z is the array layer

        Returns:
            Context manager yielding a mutable bounded array of pixel bytes

        Raises:
            RuntimeError: If coordinates are invalid or image has no data

        Example:
            ```python
            from pybevy.image import Image
            from pybevy.render import Extent3d, TextureFormat
            from pybevy.math import UVec3

            img = Image.new_fill(Extent3d(64, 64, 1), TextureFormat.Rgba8UnormSrgb, bytes([0, 0, 0, 255]))

            # Modify single pixel
            with img.pixel_bytes_mut(UVec3(10, 20, 0)) as pixel:
                pixel[0] = 255  # R
                pixel[1] = 128  # G
                pixel[2] = 64   # B
                pixel[3] = 255  # A
            ```

        Notes:
            - Zero-copy: modifies data in-place
            - Changes persist after the `with` block
            - Array length depends on texture format (4 bytes for RGBA, 1 for R8, etc.)
            - For bulk modifications, use `data_mut()` instead
        """

    def data_copy(self) -> Array:
        """Get an owned copy of the image pixel data as a bounded array.

        Returns a new writable bounded uint8 array containing a copy of the data.
        This is the safest method as the returned array is fully owned
        by Python and can be used anywhere without lifetime concerns.

        Returns:
            Owned bounded array with shape (n,) where n is the total bytes

        Example:
            ```python
            from pybevy.image import Image
            from pybevy.render import Extent3d

            img = Image(Extent3d(64, 64, 1))

            # Get owned copy
            pixels = img.data_copy()
            # Can use pixels anywhere, even after img is dropped
            mean = pixels.mean()

            # Modify copy (doesn't affect original)
            pixels[0] = 100
            ```

        Notes:
            - Always safe to use, no lifetime restrictions
            - Creates a full copy of the data (memory cost)
            - Use `data()` for zero-copy read access
            - Use `set_data()` to copy data back to the image
        """

    def set_data(
        self,
        data: Buffer | list[int] | tuple[int, ...] | np.ndarray | Array,
    ) -> None:
        """Copy uint8 pixel data into the image.

        Copies the provided data into the image's pixel buffer.
        The array size must match the image's data size.

        Args:
            data: Bytes, list, NumPy array, or bounded array to copy

        Raises:
            ValueError: If data size doesn't match image data size
            RuntimeError: If image has no data buffer

        Example:
            ```python
            from pybevy.image import Image
            from pybevy.render import Extent3d
            import numpy as np

            img = Image(Extent3d(64, 64, 1))

            # Create new pixel data
            new_pixels = np.zeros(64 * 64 * 4, dtype=np.uint8)
            new_pixels[::4] = 255  # Set all red channels to 255

            # Copy into image
            img.set_data(new_pixels)
            ```

        Notes:
            - Creates a copy of the data
            - Use `data_mut()` for zero-copy write access
            - Data size must match exactly
        """

    # Pixel access methods
    def get_color_at_1d(self, x: int) -> Color:
        """Get the color at a specific pixel in a 1D texture.

        Args:
            x: X coordinate of the pixel

        Returns:
            Color at the specified pixel

        Raises:
            RuntimeError: If coordinates are out of bounds or texture format is incompatible
        """

    def get_color_at(self, x: int, y: int) -> Color:
        """Get the color at a specific pixel in a 2D texture.

        Args:
            x: X coordinate of the pixel
            y: Y coordinate of the pixel

        Returns:
            Color at the specified pixel

        Raises:
            RuntimeError: If coordinates are out of bounds or texture format is incompatible
        """

    def get_color_at_3d(self, x: int, y: int, z: int) -> Color:
        """Get the color at a specific pixel in a 3D texture or texture array.

        Args:
            x: X coordinate of the pixel
            y: Y coordinate of the pixel
            z: Z coordinate or array layer

        Returns:
            Color at the specified pixel

        Raises:
            RuntimeError: If coordinates are out of bounds or texture format is incompatible
        """

    def set_color_at_1d(self, x: int, color: Color) -> None:
        """Set the color at a specific pixel in a 1D texture.

        Args:
            x: X coordinate of the pixel
            color: Color to set at the pixel

        Raises:
            RuntimeError: If coordinates are out of bounds or texture format is incompatible
        """

    def set_color_at(self, x: int, y: int, color: Color) -> None:
        """Set the color at a specific pixel in a 2D texture.

        Args:
            x: X coordinate of the pixel
            y: Y coordinate of the pixel
            color: Color to set at the pixel

        Raises:
            RuntimeError: If coordinates are out of bounds or texture format is incompatible
        """

    def set_color_at_3d(self, x: int, y: int, z: int, color: Color) -> None:
        """Set the color at a specific pixel in a 3D texture or texture array.

        Args:
            x: X coordinate of the pixel
            y: Y coordinate of the pixel
            z: Z coordinate or array layer
            color: Color to set at the pixel

        Raises:
            RuntimeError: If coordinates are out of bounds or texture format is incompatible
        """

    # Resizing methods
    def resize(self, size: Extent3d) -> None:
        """Resize the image to new dimensions.

        Truncates or pads with default pixel values (discards pixel data).

        Args:
            size: New dimensions for the image

        Raises:
            RuntimeError: If resize fails
        """

    def resize_in_place(self, size: Extent3d) -> None:
        """Resize the image in-place, preserving pixel data where possible.

        More memory-efficient than resize() as it reuses the existing buffer.
        Copies overlapping pixel data from the old size to the new size.

        Args:
            size: New dimensions for the image

        Raises:
            RuntimeError: If resize fails
        """

    def reinterpret_size(self, size: Extent3d) -> None:
        """Reinterpret the image with new dimensions while keeping pixel count.

        The new size must have the same total number of pixels.

        Args:
            size: New dimensions (must have same pixel count)

        Raises:
            RuntimeError: If new size has different pixel count
        """

    def reinterpret_stacked_2d_as_array(self, layers: int) -> None:
        """Reinterpret a stacked 2D texture as a 2D texture array.

        Converts a single 2D texture with stacked layers into a proper
        2D array texture where each layer is a separate array element.

        Args:
            layers: Number of layers to split the texture into

        Raises:
            RuntimeError: If layer count is incompatible with texture height
        """

    def clear(
        self,
        pixel: Buffer | list[int] | tuple[int, ...] | np.ndarray | Array,
    ) -> None:
        """Fill the entire image with a single pixel value.

        Args:
            pixel: Bytes representing a single pixel to fill the image with

        Raises:
            RuntimeError: If pixel data is invalid
        """

    def convert(self, new_format: TextureFormat) -> Image | None:
        """Convert the image to a different texture format.

        Args:
            new_format: Target texture format

        Returns:
            New Image with converted format, or None if conversion is not supported

        Raises:
            RuntimeError: If conversion fails
        """

    def save_to_buffer(
        self, format: ImageFormat = ImageFormat.Png, quality: int | None = None
    ) -> bytes:
        """Encode the image to a byte buffer in the specified format.

        Args:
            format: Output image format (default: PNG)
            quality: JPEG quality (0-100), only used for JPEG format (default: 95)

        Returns:
            Encoded image data as bytes

        Format support:
            PNG and other 8-bit destinations accept R8/RG8/RGBA8/BGRA8
            unorm images. OpenEXR and HDR additionally accept R/RG/RGBA
            16-bit and 32-bit float images without narrowing. Float R values
            become RGB luminance; float RG values become (R, G, 0). For
            compatibility with Bevy's existing 8-bit converter, R8 means
            luminance and RG8 means luminance plus alpha. Farbfeld accepts
            8-bit inputs widened exactly to 16 bits. DDS and KTX2 encoding
            are not supported.

        Raises:
            ValueError: If the texture format cannot be encoded to the chosen
                file format, CPU pixel data is absent, or the image is not one
                two-dimensional layer.
            RuntimeError: If encoding fails

        Example:
            >>> image = Image.from_buffer(some_png_data)
            >>> jpeg_bytes = image.save_to_buffer(ImageFormat.Jpeg, quality=85)
            >>> png_bytes = image.save_to_buffer(ImageFormat.Png)
        """

    def save_to_file(
        self,
        path: str,
        format: ImageFormat = ImageFormat.Png,
        quality: int | None = None,
    ) -> None:
        """Save the image to a file in the specified format.

        Args:
            path: File path to save to. Relative paths are resolved from the
                native process launch directory.
            format: Output image format (default: PNG)
            quality: JPEG quality (0-100), only used for JPEG format (default: 95)

        Format support:
            PNG and other 8-bit destinations accept R8/RG8/RGBA8/BGRA8
            unorm images. OpenEXR and HDR additionally accept R/RG/RGBA
            16-bit and 32-bit float images without narrowing. Float R values
            become RGB luminance; float RG values become (R, G, 0). R8 means
            luminance and RG8 means luminance plus alpha. Farbfeld accepts
            8-bit inputs widened exactly to 16 bits. DDS and KTX2 encoding
            are not supported.

        Raises:
            ValueError: If the texture format cannot be encoded to the chosen
                file format, CPU pixel data is absent, or the image is not one
                two-dimensional layer.
            RuntimeError: If encoding or file writing fails

        Example:
            >>> image = Image.from_buffer(some_data)
            >>> image.save_to_file("output.png")
            >>> image.save_to_file("output.jpg", ImageFormat.Jpeg, quality=90)
        """

class TextureAtlasLayout(Asset):
    """Layout defining texture regions for a texture atlas.

    A texture atlas layout stores a collection of rectangular regions (URect)
    that define individual sprites or tiles within a larger texture image.
    Used with TextureAtlas components for sprite sheet rendering.
    """

    def __init__(
        self,
        size: UVec2 = ...,
        *,
        textures: list[URect] | None = None,
    ) -> None:
        """Create a new texture atlas layout.

        Args:
            size: Total size of the atlas texture in pixels.
            textures: Initial list of texture regions (optional).
        """

    @staticmethod
    def new_empty(size: UVec2) -> TextureAtlasLayout:
        """Create a new empty texture atlas layout.

        Args:
            size: Total size of the atlas texture in pixels.

        Returns:
            New empty TextureAtlasLayout ready for adding texture regions.

        Example:
            >>> from pybevy.image import TextureAtlasLayout
            >>> from pybevy.math import UVec2
            >>> layout = TextureAtlasLayout.new_empty(UVec2(512, 512))
        """

    @staticmethod
    def from_grid(
        tile_size: UVec2,
        columns: int,
        rows: int,
        padding: UVec2 | None = None,
        offset: UVec2 | None = None,
    ) -> TextureAtlasLayout:
        """Create a grid-based texture atlas layout.

        Generates a layout where tiles are arranged in a regular grid pattern.
        Each cell is tile_size pixels, optionally separated by padding and
        starting from an offset position. Indexed left-to-right, top-to-bottom.

        Args:
            tile_size: Size of each tile in pixels.
            columns: Number of columns in the grid.
            rows: Number of rows in the grid.
            padding: Optional spacing between tiles (default: no padding).
            offset: Optional offset from top-left corner (default: (0, 0)).

        Returns:
            TextureAtlasLayout with regions for each grid cell.

        Example:
            >>> from pybevy.image import TextureAtlasLayout
            >>> from pybevy.math import UVec2
            >>> # 4x4 grid of 32x32 tiles with 2px padding
            >>> layout = TextureAtlasLayout.from_grid(
            ...     UVec2(32, 32),
            ...     columns=4,
            ...     rows=4,
            ...     padding=UVec2(2, 2)
            ... )
        """

    @property
    def size(self) -> UVec2:
        """Get the total size of the atlas texture.

        Returns:
            Total dimensions of the atlas in pixels.
        """
    @size.setter
    def size(self, value: UVec2) -> None: ...

    @property
    def textures(self) -> LiveList[URect]:
        """Get list-like access to the texture regions in the atlas.

        Returns:
            Indexed rectangular regions, each representing one texture/sprite.
        """

    def add_texture(self, rect: URect) -> int:
        """Add a texture region to the layout.

        Args:
            rect: Rectangular region to add to the atlas.

        Returns:
            Index of the added texture (for use with TextureAtlas.index).

        Example:
            >>> from pybevy.image import TextureAtlasLayout
            >>> from pybevy.math import UVec2, URect
            >>> layout = TextureAtlasLayout.new_empty(UVec2(512, 512))
            >>> index = layout.add_texture(URect(0, 0, 64, 64))
            >>> print(f"Added texture at index {index}")
        """

    def len(self) -> int:
        """Get the number of textures in the layout.

        Returns:
            Count of texture regions in this layout.
        """

    def is_empty(self) -> bool:
        """Check if the layout contains no textures.

        Returns:
            True if the layout has no texture regions, False otherwise.
        """

    def __len__(self) -> int:
        """Get the number of textures (Python len() support)."""

    def __eq__(self, other: object) -> bool: ...

class TextureAtlas:
    """Reference to a texture region within a texture atlas.
    
    Used with Sprite components to render specific regions from a sprite sheet.
    """
    
    layout: Handle[TextureAtlasLayout]
    index: int
    
    def __init__(
        self,
        layout: Handle[TextureAtlasLayout] | None = None,
        index: int = 0,
    ) -> None:
        """Create a TextureAtlas reference.

        Args:
            layout: Handle to the texture atlas layout defining all regions
            index: Index of the specific region to use from the layout
        """

    def with_index(self, index: int) -> TextureAtlas:
        """Return a copy of this TextureAtlas with the given index.

        Args:
            index: New index for the texture region.

        Returns:
            A new TextureAtlas; this one is left unchanged.

        Example:
            ```python
            atlas = TextureAtlas(layout, 0).with_index(5)
            ```
        """

    def with_layout(self, layout: Handle[TextureAtlasLayout]) -> TextureAtlas:
        """Return a copy of this TextureAtlas with the given layout.

        Args:
            layout: New layout handle.

        Returns:
            A new TextureAtlas; this one is left unchanged.

        Example:
            ```python
            atlas = TextureAtlas(layout1, 0).with_layout(layout2)
            ```
        """

class TextureAtlasSources:
    """Maps from image handles to their index in the texture atlas.

    This is typically created by TextureAtlasBuilder and is used to look up
    which section of the atlas corresponds to a particular source image.
    """

    def __init__(self) -> None:
        """Create a new empty TextureAtlasSources."""

    def texture_index(self, texture: Handle[Image]) -> int | None:
        """Get the texture index for a given image handle.

        Args:
            texture: Handle to the source image.

        Returns:
            Index of the texture in the atlas, or None if not found.
        """

    def handle(
        self, layout: Handle[TextureAtlasLayout], texture: Handle[Image]
    ) -> TextureAtlas | None:
        """Create a TextureAtlas for a given texture handle, if found in sources.

        Args:
            layout: Handle to the texture atlas layout.
            texture: Handle to the source image.

        Returns:
            TextureAtlas with the correct index, or None if texture not found.
        """

    def len(self) -> int:
        """Get the number of textures in this sources map."""

    def is_empty(self) -> bool:
        """Check if the sources map is empty."""

    def indices(self) -> list[int]:
        """Get all texture indices as a list."""

    def __len__(self) -> int:
        """Get the number of textures (Python len() support)."""

class ImageSamplerDescriptor:
    """Descriptor for configuring image sampling behavior.

    Controls filtering, addressing, and other sampling parameters.
    Mipmap filtering and LOD clamps only affect images with more than one mip
    level. Ordinary image formats and programmatic images do not generate mip
    levels automatically; authored DDS and KTX2 files can contain them.
    """

    def __init__(
        self,
        address_mode_u: ImageAddressMode = ...,
        address_mode_v: ImageAddressMode = ...,
        address_mode_w: ImageAddressMode = ...,
        mag_filter: ImageFilterMode = ...,
        min_filter: ImageFilterMode = ...,
        mipmap_filter: ImageFilterMode = ...,
        lod_min_clamp: float = 0.0,
        lod_max_clamp: float = 32.0,
        compare: ImageCompareFunction | None = None,
        anisotropy_clamp: int = 1,
        border_color: ImageSamplerBorderColor | None = None,
        label: str | None = None,
    ) -> None:
        """Create a new image sampler descriptor with the specified settings.

        Args:
            address_mode_u: Addressing mode for U texture coordinate (default: ClampToEdge)
            address_mode_v: Addressing mode for V texture coordinate (default: ClampToEdge)
            address_mode_w: Addressing mode for W texture coordinate (default: ClampToEdge)
            mag_filter: Filter for magnification (default: Nearest)
            min_filter: Filter for minification (default: Nearest)
            mipmap_filter: Filter between mipmap levels (default: Nearest)
            lod_min_clamp: Minimum level-of-detail
            lod_max_clamp: Maximum level-of-detail
            compare: Comparison function for depth testing
            anisotropy_clamp: Maximum anisotropic filtering level
            border_color: Border color for ClampToBorder mode
            label: Debug label
        """

    @staticmethod
    def linear() -> ImageSamplerDescriptor:
        """Create a sampler with linear filtering (smooth interpolation)."""

    @staticmethod
    def nearest() -> ImageSamplerDescriptor:
        """Create a sampler with nearest-neighbor filtering (pixel-perfect)."""

    @property
    def address_mode_u(self) -> ImageAddressMode:
        """Addressing mode for U (horizontal) texture coordinate."""

    @address_mode_u.setter
    def address_mode_u(self, value: ImageAddressMode) -> None: ...

    @property
    def address_mode_v(self) -> ImageAddressMode:
        """Addressing mode for V (vertical) texture coordinate."""

    @address_mode_v.setter
    def address_mode_v(self, value: ImageAddressMode) -> None: ...

    @property
    def address_mode_w(self) -> ImageAddressMode:
        """Addressing mode for W (depth) texture coordinate."""

    @address_mode_w.setter
    def address_mode_w(self, value: ImageAddressMode) -> None: ...

    @property
    def mag_filter(self) -> ImageFilterMode:
        """Filter mode when texture is magnified."""

    @mag_filter.setter
    def mag_filter(self, value: ImageFilterMode) -> None: ...

    @property
    def min_filter(self) -> ImageFilterMode:
        """Filter mode when texture is minified."""

    @min_filter.setter
    def min_filter(self, value: ImageFilterMode) -> None: ...

    @property
    def mipmap_filter(self) -> ImageFilterMode:
        """Filter mode between mipmap levels."""

    @mipmap_filter.setter
    def mipmap_filter(self, value: ImageFilterMode) -> None: ...

    @property
    def lod_min_clamp(self) -> float:
        """Minimum level-of-detail clamp."""

    @lod_min_clamp.setter
    def lod_min_clamp(self, value: float) -> None: ...

    @property
    def lod_max_clamp(self) -> float:
        """Maximum level-of-detail clamp."""

    @lod_max_clamp.setter
    def lod_max_clamp(self, value: float) -> None: ...

    @property
    def compare(self) -> ImageCompareFunction | None:
        """Comparison function for depth/stencil testing."""

    @compare.setter
    def compare(self, value: ImageCompareFunction | None) -> None: ...

    @property
    def anisotropy_clamp(self) -> int:
        """Maximum anisotropic filtering level."""

    @anisotropy_clamp.setter
    def anisotropy_clamp(self, value: int) -> None: ...

    @property
    def border_color(self) -> ImageSamplerBorderColor | None:
        """Border color for ClampToBorder address mode."""

    @border_color.setter
    def border_color(self, value: ImageSamplerBorderColor | None) -> None: ...

    @property
    def label(self) -> str | None:
        """Debug label for this sampler."""

    @label.setter
    def label(self, value: str | None) -> None: ...

class ImageFilterMode:
    """Image filtering mode for texture sampling."""

    Nearest: ClassVar[ImageFilterMode]
    Linear: ClassVar[ImageFilterMode]

class ImageAddressMode:
    """Image addressing mode for texture coordinates outside [0, 1] range."""

    ClampToEdge: ClassVar[ImageAddressMode]
    Repeat: ClassVar[ImageAddressMode]
    MirrorRepeat: ClassVar[ImageAddressMode]
    ClampToBorder: ClassVar[ImageAddressMode]

class ImageSampler:
    """Image sampler - either use default or provide a custom descriptor.

    This is a complex enum with variants:
    - Default: Use the default sampler from ImagePlugin
    - Descriptor: Use a custom ImageSamplerDescriptor
    """

    class Default(ImageSampler):
        """Default image sampler variant."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Descriptor(ImageSampler):
        """Custom sampler descriptor variant."""
        __match_args__: ClassVar[tuple[Literal["desc"]]]
        desc: ImageSamplerDescriptor
        def __init__(self, desc: ImageSamplerDescriptor) -> None: ...

    @staticmethod
    def linear() -> ImageSampler.Descriptor:
        """Create a sampler with linear filtering."""

    @staticmethod
    def nearest() -> ImageSampler.Descriptor:
        """Create a sampler with nearest-neighbor filtering."""

class ImageArrayLayout:
    """Layout specification for image array textures."""

    class RowCount(ImageArrayLayout):
        __match_args__: ClassVar[tuple[Literal["rows"]]]
        rows: int
        def __init__(self, rows: int) -> None: ...

    class RowHeight(ImageArrayLayout):
        __match_args__: ClassVar[tuple[Literal["pixels"]]]
        pixels: int
        def __init__(self, pixels: int) -> None: ...

    class GridCount(ImageArrayLayout):
        __match_args__: ClassVar[tuple[Literal["columns"], Literal["rows"]]]
        columns: int
        rows: int
        def __init__(self, columns: int, rows: int) -> None: ...

    class GridSize(ImageArrayLayout):
        __match_args__: ClassVar[
            tuple[Literal["tile_width_pixels"], Literal["tile_height_pixels"]]
        ]
        tile_width_pixels: int
        tile_height_pixels: int
        def __init__(self, tile_width_pixels: int, tile_height_pixels: int) -> None: ...

class ImageLoaderSettings:
    """Settings for loading an Image using an ImageLoader.

    Controls format detection, color space, and sampling settings.
    """

    def __init__(
        self,
        is_srgb: bool = True,
        sampler: ImageSampler | None = None,
    ) -> None:
        """Create new image loader settings.

        Args:
            is_srgb: Whether to interpret image data as sRGB (default: True).
            sampler: Sampler to use for the loaded image (default: Default sampler).
        """

    @staticmethod
    def with_format(
        format: ImageFormat,
        is_srgb: bool = True,
        sampler: ImageSampler | None = None,
    ) -> ImageLoaderSettings:
        """Create settings with a specific image format.

        Args:
            format: The image format to use for loading.
            is_srgb: Whether to interpret image data as sRGB (default: True).
            sampler: Sampler to use for the loaded image.
        """

    @property
    def is_srgb(self) -> bool:
        """Whether image data is interpreted as sRGB."""

    @is_srgb.setter
    def is_srgb(self, value: bool) -> None: ...

    @property
    def sampler(self) -> ImageSampler:
        """The sampler configuration for this image."""

    @sampler.setter
    def sampler(self, value: ImageSampler) -> None: ...

    @property
    def format(self) -> ImageFormatSetting:
        """The format detection setting for this image."""

    @property
    def asset_usage(self) -> RenderAssetUsages:
        """Asset usage flags indicating which worlds can access this image."""

    @asset_usage.setter
    def asset_usage(self, value: RenderAssetUsages) -> None: ...
