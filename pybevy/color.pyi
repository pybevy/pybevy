from typing import TYPE_CHECKING, ClassVar

from pybevy.math import Vec3, Vec4

if TYPE_CHECKING:
    from pybevy.pbr import StandardMaterial

class Color:
    """Color enum supporting multiple color spaces.

    Note: Color does not support arithmetic operations (*, /, +, -).
    Use LinearRgba for scalar multiplication:

        # This does NOT work:
        emissive = Color.WHITE() * 0.5  # TypeError

        # Use LinearRgba instead:
        emissive = LinearRgba.WHITE() * 0.5

        # Or convert:
        emissive = Color.WHITE().to_linear() * 0.5

    This matches Bevy's Rust API where only LinearRgba implements Mul<f32>.
    """

    WHITE: ClassVar[Color]
    BLACK: ClassVar[Color]
    NONE: ClassVar[Color]

    def __init__(self) -> None:
        """Create a Color with default value (WHITE)."""

    # sRGB constructors
    @staticmethod
    def srgb_u8(red: int, green: int, blue: int) -> Color: ...
    @staticmethod
    def srgba_u8(red: int, green: int, blue: int, alpha: int) -> Color: ...
    @staticmethod
    def srgb(red: float, green: float, blue: float) -> Color: ...
    @staticmethod
    def srgba(red: float, green: float, blue: float, alpha: float) -> Color: ...
    @staticmethod
    def srgb_from_array(array: tuple[float, float, float]) -> Color: ...

    # Linear RGB constructors
    @staticmethod
    def linear_rgb(red: float, green: float, blue: float) -> Color: ...
    @staticmethod
    def linear_rgba(red: float, green: float, blue: float, alpha: float) -> Color: ...

    # HSL constructors
    @staticmethod
    def hsl(hue: float, saturation: float, lightness: float) -> Color: ...
    @staticmethod
    def hsla(hue: float, saturation: float, lightness: float, alpha: float) -> Color: ...

    # HSV constructors
    @staticmethod
    def hsv(hue: float, saturation: float, value: float) -> Color: ...
    @staticmethod
    def hsva(hue: float, saturation: float, value: float, alpha: float) -> Color: ...

    # HWB constructors
    @staticmethod
    def hwb(hue: float, whiteness: float, blackness: float) -> Color: ...
    @staticmethod
    def hwba(hue: float, whiteness: float, blackness: float, alpha: float) -> Color: ...

    # LAB constructors
    @staticmethod
    def lab(lightness: float, a: float, b: float) -> Color: ...
    @staticmethod
    def laba(lightness: float, a: float, b: float, alpha: float) -> Color: ...

    # LCH constructors
    @staticmethod
    def lch(lightness: float, chroma: float, hue: float) -> Color: ...
    @staticmethod
    def lcha(lightness: float, chroma: float, hue: float, alpha: float) -> Color: ...

    # Oklab constructors
    @staticmethod
    def oklab(lightness: float, a: float, b: float) -> Color: ...
    @staticmethod
    def oklaba(lightness: float, a: float, b: float, alpha: float) -> Color: ...

    # Oklch constructors
    @staticmethod
    def oklch(lightness: float, chroma: float, hue: float) -> Color: ...
    @staticmethod
    def oklcha(lightness: float, chroma: float, hue: float, alpha: float) -> Color: ...

    # XYZ constructors
    @staticmethod
    def xyz(x: float, y: float, z: float) -> Color: ...
    @staticmethod
    def xyza(x: float, y: float, z: float, alpha: float) -> Color: ...

    # Conversion methods
    def to_linear(self) -> LinearRgba: ...
    def to_srgba(self) -> Srgba: ...
    def to_hsla(self) -> Hsla: ...

    # Alpha trait methods
    def with_alpha(self, alpha: float) -> Color: ...
    def alpha(self) -> float: ...
    def set_alpha(self, alpha: float) -> None:
        """Set the alpha channel (mutates in place)."""
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Luminance trait methods
    def luminance(self) -> float: ...
    def with_luminance(self, value: float) -> Color: ...  # Color uses 'value'
    def darker(self, amount: float) -> Color: ...
    def lighter(self, amount: float) -> Color: ...

    # Mix trait methods
    def mix(self, other: Color, factor: float) -> Color: ...
    def mix_assign(self, other: Color, factor: float) -> None:
        """Mix with another color in place."""

    # Hue trait methods
    def hue(self) -> float: ...
    def with_hue(self, hue: float) -> Color: ...
    def set_hue(self, hue: float) -> None:
        """Set the hue channel (mutates in place)."""
    def rotate_hue(self, degrees: float) -> Color: ...

    # Saturation trait methods
    def saturation(self) -> float: ...
    def with_saturation(self, saturation: float) -> Color: ...
    def set_saturation(self, saturation: float) -> None:
        """Set the saturation channel (mutates in place)."""

    # EuclideanDistance trait methods
    def distance(self, other: Color) -> float: ...
    def distance_squared(self, other: Color) -> float: ...

    def materialize(self) -> StandardMaterial:
        """Convert this color to a StandardMaterial with this color as the base_color."""

class LinearRgba:
    red: float
    green: float
    blue: float
    alpha: float

    BLACK: ClassVar[LinearRgba]
    WHITE: ClassVar[LinearRgba]
    NONE: ClassVar[LinearRgba]
    NAN: ClassVar[LinearRgba]

    def __init__(
        self,
        red: float = 1.0,
        green: float = 1.0,
        blue: float = 1.0,
        alpha: float = 1.0,
    ) -> None: ...
    @staticmethod
    def rgb(red: float, green: float, blue: float) -> LinearRgba: ...
    @staticmethod
    def gray(lightness: float) -> LinearRgba:
        """Create a gray color with the given lightness (0.0 = black, 1.0 = white)."""

    def with_red(self, red: float) -> LinearRgba: ...
    def with_green(self, green: float) -> LinearRgba: ...
    def with_blue(self, blue: float) -> LinearRgba: ...
    def with_alpha(self, alpha: float) -> LinearRgba: ...

    # Luminance trait methods
    def luminance(self) -> float: ...
    def with_luminance(self, value: float) -> LinearRgba: ...
    def darker(self, amount: float) -> LinearRgba: ...
    def lighter(self, amount: float) -> LinearRgba: ...

    # Mix trait methods
    def mix(self, other: LinearRgba, factor: float) -> LinearRgba: ...
    def mix_assign(self, other: LinearRgba, factor: float) -> None:
        """Mix with another color in place."""

    # EuclideanDistance trait methods
    def distance_squared(self, other: LinearRgba) -> float: ...

    # Alpha trait methods (alpha is directly writable on LinearRgba)
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Arithmetic operators
    def __add__(self, other: LinearRgba) -> LinearRgba: ...
    def __sub__(self, other: LinearRgba) -> LinearRgba: ...
    def __mul__(self, scalar: float) -> LinearRgba: ...
    def __rmul__(self, scalar: float) -> LinearRgba: ...
    def __truediv__(self, scalar: float) -> LinearRgba: ...
    def __neg__(self) -> LinearRgba: ...

    # Utility methods
    def to_tuple(self) -> tuple[float, float, float, float]: ...
    @staticmethod
    def from_tuple(t: tuple[float, float, float, float]) -> LinearRgba: ...

    # ColorToComponents trait methods
    def to_f32_array(self) -> list[float]:
        """Convert to [r, g, b, a] float array."""
    def to_f32_array_no_alpha(self) -> list[float]:
        """Convert to [r, g, b] float array without alpha."""
    def to_vec4(self) -> Vec4:
        """Convert to Vec4(r, g, b, a)."""
    def to_vec3(self) -> Vec3:
        """Convert to Vec3(r, g, b) without alpha."""
    @staticmethod
    def from_f32_array(
        array: list[float] | tuple[float, float, float, float],
    ) -> LinearRgba:
        """Create from [r, g, b, a] float array."""
    @staticmethod
    def from_f32_array_no_alpha(
        color: list[float] | tuple[float, float, float],
    ) -> LinearRgba:
        """Create from [r, g, b] float array, alpha defaults to 1.0."""
    @staticmethod
    def from_vec4(color: Vec4) -> LinearRgba:
        """Create from Vec4(r, g, b, a)."""
    @staticmethod
    def from_vec3(color: Vec3) -> LinearRgba:
        """Create from Vec3(r, g, b), alpha defaults to 1.0."""

    # ColorToPacked trait methods
    def to_u8_array(self) -> list[int]:
        """Convert to [r, g, b, a] u8 array [0-255]."""
    def to_u8_array_no_alpha(self) -> list[int]:
        """Convert to [r, g, b] u8 array [0-255] without alpha."""
    @staticmethod
    def from_u8_array(color: list[int] | tuple[int, int, int, int]) -> LinearRgba:
        """Create from [r, g, b, a] u8 array [0-255]."""
    @staticmethod
    def from_u8_array_no_alpha(color: list[int] | tuple[int, int, int]) -> LinearRgba:
        """Create from [r, g, b] u8 array [0-255], alpha defaults to 255."""

class Srgba:
    red: float
    green: float
    blue: float
    alpha: float

    BLACK: ClassVar[Srgba]
    WHITE: ClassVar[Srgba]
    NONE: ClassVar[Srgba]

    def __init__(
        self,
        red: float = 1.0,
        green: float = 1.0,
        blue: float = 1.0,
        alpha: float = 1.0,
    ) -> None: ...
    @staticmethod
    def rgb(red: float, green: float, blue: float) -> Srgba: ...
    @staticmethod
    def rgb_u8(r: int, g: int, b: int) -> Srgba: ...
    @staticmethod
    def rgba_u8(r: int, g: int, b: int, a: int) -> Srgba: ...
    @staticmethod
    def hex(hex: str) -> Srgba: ...
    @staticmethod
    def gray(lightness: float) -> Srgba:
        """Create a gray color with the given lightness (0.0 = black, 1.0 = white)."""
    @staticmethod
    def gamma_function(value: float) -> float:
        """Convert sRGB channel value to linear space."""
    @staticmethod
    def gamma_function_inverse(value: float) -> float:
        """Convert linear channel value to sRGB space."""

    def to_hex(self) -> str: ...
    def interpolate_stable(self, other: Srgba, t: float) -> Srgba:
        """Interpolate between two colors using stable linear interpolation."""
    def with_red(self, red: float) -> Srgba: ...
    def with_green(self, green: float) -> Srgba: ...
    def with_blue(self, blue: float) -> Srgba: ...
    def with_alpha(self, alpha: float) -> Srgba: ...

    # Alpha trait methods
    def set_alpha(self, alpha: float) -> None:
        """Set the alpha channel (mutates in place)."""
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Luminance trait methods
    def luminance(self) -> float: ...
    def with_luminance(self, value: float) -> Srgba: ...
    def darker(self, amount: float) -> Srgba: ...
    def lighter(self, amount: float) -> Srgba: ...

    # Mix trait methods
    def mix(self, other: Srgba, factor: float) -> Srgba: ...
    def mix_assign(self, other: Srgba, factor: float) -> None:
        """Mix with another color in place."""

    # EuclideanDistance trait methods
    def distance(self, other: Srgba) -> float: ...
    def distance_squared(self, other: Srgba) -> float: ...

    # ColorToComponents trait methods
    def to_f32_array(self) -> list[float]:
        """Convert to [r, g, b, a] float array."""
    def to_f32_array_no_alpha(self) -> list[float]:
        """Convert to [r, g, b] float array without alpha."""
    def to_vec4(self) -> Vec4:
        """Convert to Vec4(r, g, b, a)."""
    def to_vec3(self) -> Vec3:
        """Convert to Vec3(r, g, b) without alpha."""
    @staticmethod
    def from_f32_array(array: list[float] | tuple[float, float, float, float]) -> Srgba:
        """Create from [r, g, b, a] float array."""
    @staticmethod
    def from_f32_array_no_alpha(
        color: list[float] | tuple[float, float, float],
    ) -> Srgba:
        """Create from [r, g, b] float array, alpha defaults to 1.0."""
    @staticmethod
    def from_vec4(color: Vec4) -> Srgba:
        """Create from Vec4(r, g, b, a)."""
    @staticmethod
    def from_vec3(color: Vec3) -> Srgba:
        """Create from Vec3(r, g, b), alpha defaults to 1.0."""

    # ColorToPacked trait methods
    def to_u8_array(self) -> list[int]:
        """Convert to [r, g, b, a] u8 array [0-255]."""
    def to_u8_array_no_alpha(self) -> list[int]:
        """Convert to [r, g, b] u8 array [0-255] without alpha."""
    @staticmethod
    def from_u8_array(color: list[int] | tuple[int, int, int, int]) -> Srgba:
        """Create from [r, g, b, a] u8 array [0-255]."""
    @staticmethod
    def from_u8_array_no_alpha(color: list[int] | tuple[int, int, int]) -> Srgba:
        """Create from [r, g, b] u8 array [0-255], alpha defaults to 255."""

class Hsla:
    hue: float
    saturation: float
    lightness: float
    alpha: float

    def __init__(
        self,
        hue: float = 0.0,
        saturation: float = 0.0,
        lightness: float = 1.0,
        alpha: float = 1.0,
    ) -> None: ...
    @staticmethod
    def hsl(hue: float, saturation: float, lightness: float) -> Hsla: ...
    @staticmethod
    def gray(lightness: float) -> Hsla:
        """Create a gray color with the given lightness (0.0 = black, 1.0 = white)."""
    @staticmethod
    def sequential_dispersed(index: int) -> Hsla:
        """Create a color with maximum hue separation using the golden ratio."""

    # Hue trait methods
    def with_hue(self, hue: float) -> Hsla: ...
    def set_hue(self, hue: float) -> None:
        """Set the hue channel (mutates in place)."""
    def rotate_hue(self, degrees: float) -> Hsla: ...

    # Saturation trait methods
    def with_saturation(self, saturation: float) -> Hsla: ...
    def set_saturation(self, saturation: float) -> None:
        """Set the saturation channel (mutates in place)."""

    # Luminance trait methods
    def with_lightness(self, lightness: float) -> Hsla: ...
    def luminance(self) -> float: ...
    def with_luminance(self, lightness: float) -> Hsla: ...
    def darker(self, amount: float) -> Hsla: ...
    def lighter(self, amount: float) -> Hsla: ...

    # Alpha trait methods
    def with_alpha(self, alpha: float) -> Hsla: ...
    def set_alpha(self, alpha: float) -> None:
        """Set the alpha channel (mutates in place)."""
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Mix trait methods
    def mix(self, other: Hsla, factor: float) -> Hsla: ...
    def mix_assign(self, other: Hsla, factor: float) -> None:
        """Mix with another color in place."""

    # Conversion methods
    def to_linear(self) -> LinearRgba: ...
    def to_srgba(self) -> Srgba: ...

    # Array/Vector conversions
    def to_f32_array(self) -> list[float]: ...
    def to_f32_array_no_alpha(self) -> list[float]: ...
    @staticmethod
    def from_f32_array(array: list[float] | tuple[float, float, float, float]) -> Hsla: ...
    @staticmethod
    def from_f32_array_no_alpha(color: list[float] | tuple[float, float, float]) -> Hsla: ...
    def to_vec4(self) -> Vec4: ...
    def to_vec3(self) -> Vec3: ...
    @staticmethod
    def from_vec4(color: Vec4) -> Hsla: ...
    @staticmethod
    def from_vec3(color: Vec3) -> Hsla: ...


class Oklcha:
    """Perceptually uniform Oklch color space with alpha.

    Oklch is a perceptually uniform color space based on Oklab, designed for better
    color manipulation and interpolation. It uses cylindrical coordinates with:
    - Lightness: perceived brightness [0.0-1.0]
    - Chroma: colorfulness/saturation [0.0-1.0]
    - Hue: color angle [0.0-360.0]
    - Alpha: transparency [0.0-1.0]

    This color space is ideal for creating smooth gradients and perceptually uniform
    color variations, as equal distances represent equal perceived color differences.
    """

    lightness: float
    chroma: float
    hue: float
    alpha: float

    def __init__(
        self,
        lightness: float = 1.0,
        chroma: float = 0.0,
        hue: float = 0.0,
        alpha: float = 1.0,
    ) -> None: ...
    @staticmethod
    def lch(lightness: float, chroma: float, hue: float) -> Oklcha:
        """Create Oklcha color with default alpha (1.0)."""

    @staticmethod
    def gray(lightness: float) -> Oklcha:
        """Create a gray color with the given lightness (0.0 = black, 1.0 = white)."""
    @staticmethod
    def sequential_dispersed(index: int) -> Oklcha:
        """Create a color with maximum hue separation using the golden ratio."""

    # Field methods
    def with_lightness(self, lightness: float) -> Oklcha: ...
    def with_chroma(self, chroma: float) -> Oklcha: ...

    # Hue trait methods
    def with_hue(self, hue: float) -> Oklcha: ...
    def set_hue(self, hue: float) -> None:
        """Set the hue channel (mutates in place)."""
    def rotate_hue(self, degrees: float) -> Oklcha: ...

    # Luminance trait methods
    def luminance(self) -> float: ...
    def with_luminance(self, lightness: float) -> Oklcha: ...
    def darker(self, amount: float) -> Oklcha: ...
    def lighter(self, amount: float) -> Oklcha: ...

    # Alpha trait methods
    def with_alpha(self, alpha: float) -> Oklcha: ...
    def set_alpha(self, alpha: float) -> None:
        """Set the alpha channel (mutates in place)."""
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Mix trait methods
    def mix(self, other: Oklcha, factor: float) -> Oklcha: ...
    def mix_assign(self, other: Oklcha, factor: float) -> None:
        """Mix with another color in place."""

    # EuclideanDistance trait methods
    def distance(self, other: Oklcha) -> float: ...
    def distance_squared(self, other: Oklcha) -> float: ...

    # Conversion methods
    def to_linear(self) -> LinearRgba: ...
    def to_srgba(self) -> Srgba: ...

    # Array/Vector conversions
    def to_f32_array(self) -> list[float]: ...
    def to_f32_array_no_alpha(self) -> list[float]: ...
    @staticmethod
    def from_f32_array(array: list[float] | tuple[float, float, float, float]) -> Oklcha: ...
    @staticmethod
    def from_f32_array_no_alpha(color: list[float] | tuple[float, float, float]) -> Oklcha: ...
    def to_vec4(self) -> Vec4: ...
    def to_vec3(self) -> Vec3: ...
    @staticmethod
    def from_vec4(color: Vec4) -> Oklcha: ...
    @staticmethod
    def from_vec3(color: Vec3) -> Oklcha: ...


class Lcha:
    """CIE LCH color space with alpha.

    LCH (Lightness, Chroma, Hue) is a cylindrical representation of the CIELAB color space.
    It uses:
    - Lightness: perceived brightness [0.0-1.5]
    - Chroma: colorfulness/saturation [0.0-1.5]
    - Hue: color angle [0.0-360.0]
    - Alpha: transparency [0.0-1.0]

    This color space is designed for perceptually uniform color manipulation, though
    Oklch is generally preferred for modern applications due to better perceptual uniformity.
    """

    lightness: float
    chroma: float
    hue: float
    alpha: float

    def __init__(
        self,
        lightness: float = 1.0,
        chroma: float = 0.0,
        hue: float = 0.0,
        alpha: float = 1.0,
    ) -> None: ...
    @staticmethod
    def lch(lightness: float, chroma: float, hue: float) -> Lcha:
        """Create Lcha color with default alpha (1.0)."""

    @staticmethod
    def gray(lightness: float) -> Lcha:
        """Create a gray color with the given lightness (0.0 = black, 1.0 = white)."""
    @staticmethod
    def sequential_dispersed(index: int) -> Lcha:
        """Create a color with maximum hue separation using the golden ratio."""

    # Field methods
    def with_lightness(self, lightness: float) -> Lcha: ...
    def with_chroma(self, chroma: float) -> Lcha: ...

    # Hue trait methods
    def with_hue(self, hue: float) -> Lcha: ...
    def set_hue(self, hue: float) -> None:
        """Set the hue channel (mutates in place)."""
    def rotate_hue(self, degrees: float) -> Lcha: ...

    # Luminance trait methods
    def luminance(self) -> float: ...
    def with_luminance(self, lightness: float) -> Lcha: ...
    def darker(self, amount: float) -> Lcha: ...
    def lighter(self, amount: float) -> Lcha: ...

    # Alpha trait methods
    def with_alpha(self, alpha: float) -> Lcha: ...
    def set_alpha(self, alpha: float) -> None:
        """Set the alpha channel (mutates in place)."""
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Mix trait methods
    def mix(self, other: Lcha, factor: float) -> Lcha: ...
    def mix_assign(self, other: Lcha, factor: float) -> None:
        """Mix with another color in place."""

    # Conversion methods
    def to_linear(self) -> LinearRgba: ...
    def to_srgba(self) -> Srgba: ...

    # Array/Vector conversions
    def to_f32_array(self) -> list[float]: ...
    def to_f32_array_no_alpha(self) -> list[float]: ...
    @staticmethod
    def from_f32_array(array: list[float] | tuple[float, float, float, float]) -> Lcha: ...
    @staticmethod
    def from_f32_array_no_alpha(color: list[float] | tuple[float, float, float]) -> Lcha: ...
    def to_vec4(self) -> Vec4: ...
    def to_vec3(self) -> Vec3: ...
    @staticmethod
    def from_vec4(color: Vec4) -> Lcha: ...
    @staticmethod
    def from_vec3(color: Vec3) -> Lcha: ...


class Hsva:
    """HSV (Hue, Saturation, Value) color space with alpha.

    HSV is a cylindrical color model that represents colors in terms of:
    - Hue: color angle [0.0-360.0]
    - Saturation: colorfulness [0.0-1.0]
    - Value: brightness [0.0-1.0]
    - Alpha: transparency [0.0-1.0]

    Similar to HSL but uses "value" (brightness) instead of "lightness".
    Commonly used in color pickers and image editing software.
    """

    hue: float
    saturation: float
    value: float
    alpha: float

    def __init__(
        self,
        hue: float = 0.0,
        saturation: float = 0.0,
        value: float = 1.0,
        alpha: float = 1.0,
    ) -> None: ...
    @staticmethod
    def hsv(hue: float, saturation: float, value: float) -> Hsva:
        """Create Hsva color with default alpha (1.0)."""

    @staticmethod
    def gray(lightness: float) -> Hsva:
        """Create a gray color with the given lightness (0.0 = black, 1.0 = white)."""

    # Field methods
    def with_value(self, value: float) -> Hsva: ...

    # Hue trait methods
    def with_hue(self, hue: float) -> Hsva: ...
    def set_hue(self, hue: float) -> None:
        """Set the hue channel (mutates in place)."""
    def rotate_hue(self, degrees: float) -> Hsva: ...

    # Saturation trait methods
    def with_saturation(self, saturation: float) -> Hsva: ...
    def set_saturation(self, saturation: float) -> None:
        """Set the saturation channel (mutates in place)."""

    # Alpha trait methods
    def with_alpha(self, alpha: float) -> Hsva: ...
    def set_alpha(self, alpha: float) -> None:
        """Set the alpha channel (mutates in place)."""
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Mix trait methods
    def mix(self, other: Hsva, factor: float) -> Hsva: ...
    def mix_assign(self, other: Hsva, factor: float) -> None:
        """Mix with another color in place."""

    # Conversion methods
    def to_linear(self) -> LinearRgba: ...
    def to_srgba(self) -> Srgba: ...

    # Array/Vector conversions
    def to_f32_array(self) -> list[float]: ...
    def to_f32_array_no_alpha(self) -> list[float]: ...
    @staticmethod
    def from_f32_array(array: list[float] | tuple[float, float, float, float]) -> Hsva: ...
    @staticmethod
    def from_f32_array_no_alpha(color: list[float] | tuple[float, float, float]) -> Hsva: ...
    def to_vec4(self) -> Vec4: ...
    def to_vec3(self) -> Vec3: ...
    @staticmethod
    def from_vec4(color: Vec4) -> Hsva: ...
    @staticmethod
    def from_vec3(color: Vec3) -> Hsva: ...


class Laba:
    """CIE LAB (L*a*b*) color space with alpha.

    A perceptually uniform color space where:
    - L (lightness): [0.0-1.0] from black to white
    - a: green-red axis (negative=green, positive=red)
    - b: blue-yellow axis (negative=blue, positive=yellow)
    - alpha: transparency [0.0-1.0]

    LAB is device-independent and designed to approximate human vision.
    """

    lightness: float
    a: float
    b: float
    alpha: float

    BLACK: ClassVar[Laba]
    WHITE: ClassVar[Laba]

    def __init__(
        self,
        lightness: float = 1.0,
        a: float = 0.0,
        b: float = 0.0,
        alpha: float = 1.0,
    ) -> None: ...
    @staticmethod
    def lab(lightness: float, a: float, b: float) -> Laba:
        """Create Laba color with default alpha (1.0)."""

    @staticmethod
    def gray(lightness: float) -> Laba:
        """Create a gray color with the given lightness (0.0 = black, 1.0 = white)."""

    # Field methods
    def with_lightness(self, lightness: float) -> Laba: ...

    # Luminance trait methods
    def luminance(self) -> float: ...
    def with_luminance(self, lightness: float) -> Laba: ...
    def darker(self, amount: float) -> Laba: ...
    def lighter(self, amount: float) -> Laba: ...

    # Alpha trait methods
    def with_alpha(self, alpha: float) -> Laba: ...
    def set_alpha(self, alpha: float) -> None:
        """Set the alpha channel (mutates in place)."""
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Mix trait methods
    def mix(self, other: Laba, factor: float) -> Laba: ...
    def mix_assign(self, other: Laba, factor: float) -> None:
        """Mix with another color in place."""

    # Interpolation methods
    def interpolate_stable(self, other: Laba, t: float) -> Laba:
        """Interpolate between two colors using stable linear interpolation."""

    # Conversion methods
    def to_linear(self) -> LinearRgba: ...
    def to_srgba(self) -> Srgba: ...

    # Array/Vector conversions
    def to_f32_array(self) -> list[float]: ...
    def to_f32_array_no_alpha(self) -> list[float]:
        """Convert to [l, a, b] float array without alpha."""
    @staticmethod
    def from_f32_array(array: list[float]) -> Laba: ...
    @staticmethod
    def from_f32_array_no_alpha(
        color: list[float] | tuple[float, float, float],
    ) -> Laba:
        """Create from [l, a, b] float array, alpha defaults to 1.0."""
    def to_vec4(self) -> Vec4: ...
    @staticmethod
    def from_vec4(color: Vec4) -> Laba: ...
    def to_vec3(self) -> Vec3: ...
    @staticmethod
    def from_vec3(color: Vec3) -> Laba: ...


class Oklaba:
    """Oklab color space with alpha.

    A perceptually uniform color space (improvement over LAB) where:
    - L (lightness): [0.0-1.0] from black to white
    - a: green-red axis (negative=green, positive=red)
    - b: blue-yellow axis (negative=blue, positive=yellow)
    - alpha: transparency [0.0-1.0]

    Oklab provides better perceptual uniformity than CIE LAB,
    especially for blues and high-saturation colors.
    """

    lightness: float
    a: float
    b: float
    alpha: float

    BLACK: ClassVar[Oklaba]
    WHITE: ClassVar[Oklaba]

    def __init__(
        self,
        lightness: float = 1.0,
        a: float = 0.0,
        b: float = 0.0,
        alpha: float = 1.0,
    ) -> None: ...
    @staticmethod
    def lab(lightness: float, a: float, b: float) -> Oklaba:
        """Create Oklaba color with default alpha (1.0)."""

    @staticmethod
    def gray(lightness: float) -> Oklaba:
        """Create a gray color with the given lightness (0.0 = black, 1.0 = white)."""

    # Field methods
    def with_lightness(self, lightness: float) -> Oklaba: ...
    def with_a(self, a: float) -> Oklaba: ...
    def with_b(self, b: float) -> Oklaba: ...

    # Luminance trait methods
    def luminance(self) -> float: ...
    def with_luminance(self, lightness: float) -> Oklaba: ...
    def darker(self, amount: float) -> Oklaba: ...
    def lighter(self, amount: float) -> Oklaba: ...

    # Alpha trait methods
    def with_alpha(self, alpha: float) -> Oklaba: ...
    def set_alpha(self, alpha: float) -> None:
        """Set the alpha channel (mutates in place)."""
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Mix trait methods
    def mix(self, other: Oklaba, factor: float) -> Oklaba: ...
    def mix_assign(self, other: Oklaba, factor: float) -> None:
        """Mix with another color in place."""

    # Distance methods
    def distance(self, other: Oklaba) -> float: ...
    def distance_squared(self, other: Oklaba) -> float: ...

    # Interpolation methods
    def interpolate_stable(self, other: Oklaba, t: float) -> Oklaba:
        """Interpolate between two colors using stable linear interpolation."""

    # Conversion methods
    def to_linear(self) -> LinearRgba: ...
    def to_srgba(self) -> Srgba: ...

    # Array/Vector conversions
    def to_f32_array(self) -> list[float]: ...
    def to_f32_array_no_alpha(self) -> list[float]:
        """Convert to [l, a, b] float array without alpha."""
    @staticmethod
    def from_f32_array(array: list[float]) -> Oklaba: ...
    @staticmethod
    def from_f32_array_no_alpha(
        color: list[float] | tuple[float, float, float],
    ) -> Oklaba:
        """Create from [l, a, b] float array, alpha defaults to 1.0."""
    def to_vec4(self) -> Vec4: ...
    @staticmethod
    def from_vec4(color: Vec4) -> Oklaba: ...
    def to_vec3(self) -> Vec3:
        """Convert to Vec3(l, a, b) without alpha."""
    @staticmethod
    def from_vec3(color: Vec3) -> Oklaba:
        """Create from Vec3(l, a, b), alpha defaults to 1.0."""


class Xyza:
    """CIE XYZ color space with alpha.

    A device-independent color space derived from human vision studies:
    - X: red/green mixture
    - Y: luminance (brightness)
    - Z: blue stimulation
    - alpha: transparency [0.0-1.0]

    XYZ is the foundation of most color management systems and serves
    as an intermediate space for converting between other color spaces.
    """

    x: float
    y: float
    z: float
    alpha: float

    BLACK: ClassVar[Xyza]
    WHITE: ClassVar[Xyza]

    def __init__(
        self,
        x: float = 0.0,
        y: float = 0.0,
        z: float = 0.0,
        alpha: float = 1.0,
    ) -> None: ...
    @staticmethod
    def xyz(x: float, y: float, z: float) -> Xyza:
        """Create Xyza color with default alpha (1.0)."""

    @staticmethod
    def gray(lightness: float) -> Xyza:
        """Create a gray color with the given lightness (0.0 = black, 1.0 = white)."""

    # Field methods
    def with_x(self, x: float) -> Xyza: ...
    def with_y(self, y: float) -> Xyza: ...
    def with_z(self, z: float) -> Xyza: ...

    # Luminance trait methods (Y channel is luminance in XYZ)
    def luminance(self) -> float: ...
    def with_luminance(self, lightness: float) -> Xyza: ...
    def darker(self, amount: float) -> Xyza: ...
    def lighter(self, amount: float) -> Xyza: ...

    # Alpha trait methods
    def with_alpha(self, alpha: float) -> Xyza: ...
    def set_alpha(self, alpha: float) -> None:
        """Set the alpha channel (mutates in place)."""
    def is_fully_transparent(self) -> bool: ...
    def is_fully_opaque(self) -> bool: ...

    # Mix trait methods
    def mix(self, other: Xyza, factor: float) -> Xyza: ...
    def mix_assign(self, other: Xyza, factor: float) -> None:
        """Mix with another color in place."""

    # Interpolation methods
    def interpolate_stable(self, other: Xyza, t: float) -> Xyza:
        """Interpolate between two colors using stable linear interpolation."""

    # Conversion methods
    def to_linear(self) -> LinearRgba: ...
    def to_srgba(self) -> Srgba: ...

    # Array/Vector conversions
    def to_f32_array(self) -> list[float]: ...
    def to_f32_array_no_alpha(self) -> list[float]:
        """Convert to [x, y, z] float array without alpha."""
    @staticmethod
    def from_f32_array(array: list[float]) -> Xyza: ...
    @staticmethod
    def from_f32_array_no_alpha(
        color: list[float] | tuple[float, float, float],
    ) -> Xyza:
        """Create from [x, y, z] float array, alpha defaults to 1.0."""
    def to_vec4(self) -> Vec4: ...
    @staticmethod
    def from_vec4(color: Vec4) -> Xyza: ...
    def to_vec3(self) -> Vec3:
        """Convert to Vec3(x, y, z) without alpha."""
    @staticmethod
    def from_vec3(color: Vec3) -> Xyza:
        """Create from Vec3(x, y, z), alpha defaults to 1.0."""
