from collections.abc import Iterator
from datetime import timedelta
from typing import ClassVar, Final, Literal

import numpy as np

from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.assets import Handle as AssetHandle
from pybevy.color import Color
from pybevy.ecs import Batchable, Component, Resource, SystemSet
from pybevy.image import TextureAtlasLayout
from pybevy.math import Vec2

Text2dUpdateSystems: Final[SystemSet]
EditableTextSystems: Final[SystemSet]

class Font(Asset):
    """Font asset containing font file data.

    Fonts are loaded from TTF, OTF, or other font file formats and used
    by TextFont components to render text.

    Example:
        >>> from pybevy.text import Font, TextFont, Text2d
        >>> from pybevy.assets import AssetServer
        >>>
        >>> # Load a custom font
        >>> def setup(asset_server: Res[AssetServer]):
        >>>     font_handle = asset_server.load("fonts/custom.ttf")
        >>>     commands.spawn((
        >>>         Text2d("Custom Font!"),
        >>>         TextFont(font=font_handle, font_size=48.0),
        >>>     ))
        >>>
        >>> # Create font from bytes
        >>> font_data = open("fonts/custom.ttf", "rb").read()
        >>> font = Font.try_from_bytes(font_data)
    """

    @staticmethod
    def try_from_bytes(font_data: bytes) -> Font:
        """Create a Font from bytes.

        Args:
            font_data: Font file data as bytes (TTF, OTF, etc.)

        Returns:
            Font asset

        Raises:
            ValueError: If font data is invalid or cannot be parsed
        """

    def data(self) -> bytes:
        """Get the font data as bytes.

        Returns:
            Font file data as bytes
        """

    @property
    def alias(self) -> str: ...
    @alias.setter
    def alias(self, value: str) -> None: ...

class FontAtlas:
    """A font atlas containing rasterized glyphs.

    FontAtlas stores glyph textures for efficient text rendering.
    Access through FontAtlasSet resource.

    Each FontAtlas contains:
    - A texture image where glyphs are packed
    - A texture atlas layout describing glyph positions
    """

    @property
    def texture_atlas(self) -> TextureAtlasLayout:
        """The layout for the font atlas (a snapshot copy)."""

    @property
    def texture(self) -> Handle:
        """Handle to the Image containing the rasterized glyphs."""

class FontAtlasKey:
    """Key identifying a font atlas by size and smoothing method.

    Used as a key when iterating over FontAtlasSet.
    """

    @property
    def id(self) -> int:
        """Identifier of the font face this atlas belongs to."""

    @property
    def index(self) -> int:
        """Index of the font within its source collection."""

    @property
    def font_size_bits(self) -> int:
        """The font size encoded as bits (binary representation of f32)."""

    @property
    def variations_hash(self) -> int:
        """Hash of the font variation axis settings."""

    @property
    def hinting(self) -> FontHinting:
        """The font hinting strategy used for this atlas."""

    @property
    def font_smoothing(self) -> FontSmoothing:
        """The font smoothing method used for this atlas."""

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class FontAtlasSet(Resource):
    """A resource providing access to font atlases for all loaded fonts.

    FontAtlasSet maps font faces to their rasterized glyph atlases.
    Use this to inspect font atlas textures for debugging or visualization.

    Example:
        >>> from pybevy.text import FontAtlasSet
        >>> from pybevy.ecs import Res
        >>>
        >>> def debug_atlases(font_atlas_set: Res[FontAtlasSet]) -> None:
        >>>     for key, atlases in font_atlas_set.items():
        >>>         print(f"Font size bits: {key.font_size_bits}")
        >>>         for atlas in atlases:
        >>>             print(f"  Texture: {atlas.texture}")
    """

    def items(self) -> list[tuple[FontAtlasKey, list[FontAtlas]]]:
        """Return all (FontAtlasKey, list[FontAtlas]) entries in the set."""

    def __iter__(self) -> Iterator[FontAtlasKey]:
        """Iterate over the FontAtlasKey entries, like iterating a dict yields its keys."""

    def __len__(self) -> int:
        """Return the total number of font atlas entries."""

class LineHeight(Component):
    """Line height specification for text.

    A component that controls the vertical spacing between lines of text.
    Can be specified in absolute pixels or relative to the font size.

    Example:
        >>> from pybevy.text import Text2d, LineHeight
        >>>
        >>> # Absolute pixel line height
        >>> commands.spawn((Text2d("Hello!"), LineHeight.Px(24.0)))
        >>>
        >>> # Relative to font size (1.5x)
        >>> commands.spawn((Text2d("Hello!"), LineHeight.RelativeToFont(1.5)))
    """

    class Px(LineHeight):
        """Line height in pixels."""

        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    class RelativeToFont(LineHeight):
        """Line height as a multiple of the font size."""

        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    def __eq__(self, other: object) -> bool: ...

class FontWeight:
    """How thick or bold the strokes of a font appear.

    Font weights can be any value between 1 and 1000, inclusive.
    Only supports variable weight fonts.

    Example:
        >>> from pybevy.text import TextFont, FontWeight
        >>>
        >>> # Use predefined constants
        >>> TextFont(font_size=24.0, weight=FontWeight.BOLD)
        >>>
        >>> # Use custom numeric weight
        >>> TextFont(font_size=24.0, weight=FontWeight(450))
    """

    THIN: ClassVar[FontWeight]
    """Weight 100."""

    EXTRA_LIGHT: ClassVar[FontWeight]
    """Weight 200."""

    LIGHT: ClassVar[FontWeight]
    """Weight 300."""

    NORMAL: ClassVar[FontWeight]
    """Weight 400."""

    MEDIUM: ClassVar[FontWeight]
    """Weight 500."""

    SEMIBOLD: ClassVar[FontWeight]
    """Weight 600."""

    BOLD: ClassVar[FontWeight]
    """Weight 700."""

    EXTRA_BOLD: ClassVar[FontWeight]
    """Weight 800."""

    BLACK: ClassVar[FontWeight]
    """Weight 900."""

    EXTRA_BLACK: ClassVar[FontWeight]
    """Weight 950."""

    DEFAULT: ClassVar[FontWeight]
    """The default font weight (NORMAL / 400)."""

    def __init__(self, value: int = 400) -> None:
        """Create a FontWeight with the given numeric value (1-1000).

        Args:
            value: Font weight value (default: 400 / NORMAL)
        """

    @property
    def value(self) -> int:
        """The numeric weight value (1-1000)."""

    def clamp(self) -> FontWeight:
        """Clamp the font weight to a valid range.

        Returns DEFAULT (400) if weight is 0, caps at 1000 if above.
        """

    def __eq__(self, other: object) -> bool: ...

class FontWidth:
    """The width (stretch) of a font face as a float ratio (0.5-2.0).

    Offers named presets matching bevy's `FontWidth` constants.

    Example:
        >>> from pybevy.text import TextFont, FontWidth
        >>> TextFont(font_size=24.0, width=FontWidth.CONDENSED)
        >>> TextFont(font_size=24.0, width=FontWidth(0.8))
    """

    ULTRA_CONDENSED: ClassVar[FontWidth]
    EXTRA_CONDENSED: ClassVar[FontWidth]
    CONDENSED: ClassVar[FontWidth]
    SEMI_CONDENSED: ClassVar[FontWidth]
    NORMAL: ClassVar[FontWidth]
    """The default font width."""
    SEMI_EXPANDED: ClassVar[FontWidth]
    EXPANDED: ClassVar[FontWidth]
    EXTRA_EXPANDED: ClassVar[FontWidth]
    ULTRA_EXPANDED: ClassVar[FontWidth]

    def __init__(self, value: float = 1.0) -> None:
        """Create a FontWidth with the given ratio (default: 1.0 / NORMAL)."""

    @property
    def value(self) -> float:
        """The width ratio (0.5-2.0)."""

    def __eq__(self, other: object) -> bool: ...

class FontStyle:
    """The slant style of a font face: normal, italic, or oblique.

    Example:
        >>> from pybevy.text import TextFont, FontStyle
        >>> TextFont(font_size=24.0, style=FontStyle.Italic())
        >>> TextFont(font_size=24.0, style=FontStyle.Oblique(14.0))
    """

    class Normal(FontStyle):
        """A face that is neither italic nor obliqued (default)."""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Italic(FontStyle):
        """A form that is generally cursive in nature."""

        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Oblique(FontStyle):
        """A sloped face with an optional slant angle in degrees."""

        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float | None
        def __init__(self, value: float | None = None) -> None: ...

    def __eq__(self, other: object) -> bool: ...

class FontHinting:
    """Font hinting strategy controlling glyph rasterization."""

    Disabled: FontHinting
    """Glyphs are rasterized without hinting (default)."""

    Enabled: FontHinting
    """Glyphs are rasterized with hinting."""

    def __hash__(self) -> int: ...

class FontFeatureTag:
    """A single OpenType feature tag (4 ASCII characters).

    Represents an OpenType feature like "liga" (ligatures) or "smcp" (small caps).

    Example:
        >>> tag = FontFeatureTag("liga")
        >>> assert tag == FontFeatureTag("liga")
    """

    STANDARD_LIGATURES: ClassVar[FontFeatureTag]
    """Standard ligatures ("liga")."""

    CONTEXTUAL_LIGATURES: ClassVar[FontFeatureTag]
    """Contextual ligatures ("clig")."""

    DISCRETIONARY_LIGATURES: ClassVar[FontFeatureTag]
    """Discretionary ligatures ("dlig")."""

    CONTEXTUAL_ALTERNATES: ClassVar[FontFeatureTag]
    """Contextual alternates ("calt")."""

    STYLISTIC_ALTERNATES: ClassVar[FontFeatureTag]
    """Stylistic alternates ("salt")."""

    SMALL_CAPS: ClassVar[FontFeatureTag]
    """Small capitals ("smcp")."""

    CAPS_TO_SMALL_CAPS: ClassVar[FontFeatureTag]
    """Uppercase-to-small-capitals substitution ("c2sc")."""

    SWASH: ClassVar[FontFeatureTag]
    """Swash variants ("swsh")."""

    TITLING_ALTERNATES: ClassVar[FontFeatureTag]
    """Titling alternates ("titl")."""

    FRACTIONS: ClassVar[FontFeatureTag]
    """Fractions ("frac")."""

    ORDINALS: ClassVar[FontFeatureTag]
    """Ordinal forms ("ordn")."""

    SLASHED_ZERO: ClassVar[FontFeatureTag]
    """Slashed zero ("zero")."""

    SUPERSCRIPT: ClassVar[FontFeatureTag]
    """Superscript figures ("sups")."""

    SUBSCRIPT: ClassVar[FontFeatureTag]
    """Subscript figures ("subs")."""

    OLDSTYLE_FIGURES: ClassVar[FontFeatureTag]
    """Old-style figures ("onum")."""

    LINING_FIGURES: ClassVar[FontFeatureTag]
    """Lining figures ("lnum")."""

    PROPORTIONAL_FIGURES: ClassVar[FontFeatureTag]
    """Proportional figures ("pnum")."""

    TABULAR_FIGURES: ClassVar[FontFeatureTag]
    """Tabular figures ("tnum")."""

    WEIGHT: ClassVar[FontFeatureTag]
    """Variable font weight ("wght")."""

    WIDTH: ClassVar[FontFeatureTag]
    """Variable font width ("wdth")."""

    SLANT: ClassVar[FontFeatureTag]
    """Variable font slant ("slnt")."""

    def __init__(self, tag: str) -> None:
        """Create a FontFeatureTag from a 4-character ASCII string.

        Raises:
            ValueError: If tag is not exactly 4 ASCII characters.
        """

    def __eq__(self, other: object) -> bool: ...
    def __repr__(self) -> str: ...
    def __str__(self) -> str: ...

    @property
    def value(self) -> str:
        """The four-character OpenType tag."""

class FontFeatures:
    """OpenType font features for .otf fonts that support them.

    Example:
        >>> from pybevy.text import FontFeatureTag, FontFeatures
        >>>
        >>> features = (FontFeatures.builder()
        ...     .enable(FontFeatureTag.STANDARD_LIGATURES)
        ...     .set(FontFeatureTag.WEIGHT, 300)
        ...     .build())
    """

    def __init__(self) -> None:
        """Create empty FontFeatures (no features enabled)."""

    @staticmethod
    def builder() -> FontFeaturesBuilder:
        """Create a FontFeaturesBuilder."""

    def __eq__(self, other: object) -> bool: ...

class FontFeaturesBuilder:
    """Builder for OpenType font features."""

    def __init__(self) -> None: ...
    def enable(self, feature_tag: FontFeatureTag) -> FontFeaturesBuilder:
        """Return a builder with the feature enabled."""
    def set(self, feature_tag: FontFeatureTag, value: int) -> FontFeaturesBuilder:
        """Return a builder with the feature set to a specific value."""
    def build(self) -> FontFeatures:
        """Build the FontFeatures value."""

class FontSmoothing:
    """Antialiasing method for text rendering."""

    None_: FontSmoothing
    """No antialiasing - for pixel art aesthetic"""

    AntiAliased: FontSmoothing
    """Grayscale antialiasing (default)"""

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class Justify:
    """Text alignment options."""

    Left: Justify
    """Left-aligned text"""

    Center: Justify
    """Center-aligned text"""

    Right: Justify
    """Right-aligned text"""

    Justified: Justify
    """Fully justified text"""

    Start: Justify
    """Aligned to the start of the line (left for LTR, right for RTL)"""

    End: Justify
    """Aligned to the end of the line (right for LTR, left for RTL)"""

    def __eq__(self, other: object) -> bool: ...

class LineBreak:
    """Line breaking behavior for text wrapping."""

    WordBoundary: LineBreak
    """Break at word boundaries using Unicode Line Breaking Algorithm (default)"""

    AnyCharacter: LineBreak
    """Break at any character"""

    WordOrCharacter: LineBreak
    """Break at word boundary, fallback to character if needed"""

    NoWrap: LineBreak
    """No soft wrapping (hard breaks like \\n still work)"""

    def __eq__(self, other: object) -> bool: ...

class TextBounds(Component):
    """Width and height constraints supplied to world-space text layout.

    Width controls line wrapping. Bevy 0.19 does not use height to clip or
    truncate laid-out lines. Rendering is still clipped by the camera target,
    so explicitly set a sufficiently wide bound when a long unwrapped Text2d
    string must extend beyond the viewport-sized default layout area.
    """

    UNBOUNDED: ClassVar[TextBounds]

    width: float | None
    """Maximum width in logical pixels (None = unbounded)"""

    height: float | None
    """Maximum height in logical pixels (None = unbounded)"""

    def __init__(self, width: float | None = None, height: float | None = None) -> None:
        """Create text bounds with optional width/height constraints."""

    @staticmethod
    def new_horizontal(width: float) -> TextBounds:
        """Create text bounds with width limit, unbounded height."""

    @staticmethod
    def new_vertical(height: float) -> TextBounds:
        """Create text bounds with height limit, unbounded width."""

class TextColor(Component):
    """Text color component."""

    BLACK: ClassVar[TextColor]
    WHITE: ClassVar[TextColor]

    color: Color

    def __init__(self, color: Color = Color.WHITE) -> None:
        """Create a text color component."""

    @staticmethod
    def from_numpy(*, color: np.typing.ArrayLike | None = None) -> Batchable: ...  # type: ignore[override]

    def __eq__(self, other: object) -> bool: ...

class FontSize:
    """The vertical height of rasterized glyphs, in one of several units.

    Wherever a FontSize is accepted, a plain float is also accepted and
    treated as `FontSize.Px` (mirrors bevy's `From<f32> for FontSize`).
    TextFont requires the value to be finite and non-negative.
    """

    @property
    def value(self) -> float:
        """The numeric payload of the variant."""

    class Px(FontSize):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    class Vw(FontSize):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    class Vh(FontSize):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    class VMin(FontSize):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    class VMax(FontSize):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    class Rem(FontSize):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    def __eq__(self, other: object) -> bool: ...

class FontSource:
    """Where a TextFont resolves its font face from.

    A specific Font asset handle, a font family name resolved through the
    font database, or a generic font category (serif, monospace, ...).
    Wherever a FontSource is accepted, a Handle is treated as
    `FontSource.Handle` and a str as `FontSource.Family` (mirrors bevy's
    `From` impls).
    """

    class Handle(FontSource):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: AssetHandle
        def __init__(self, value: AssetHandle) -> None: ...

    class Family(FontSource):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    class Serif(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SansSerif(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Cursive(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Fantasy(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Monospace(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SystemUi(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class UiSerif(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class UiSansSerif(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class UiMonospace(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class UiRounded(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Emoji(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Math(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FangSong(FontSource):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    def __eq__(self, other: object) -> bool: ...

class TextFont(Component):
    """Font styling for text spans.

    Determines font face, size, and antialiasing.
    """

    @property
    def font(self) -> FontSource:
        """The font source (default: the default font handle)."""

    @font.setter
    def font(self, value: FontSource | Handle | str) -> None: ...
    @property
    def font_size(self) -> FontSize:
        """Finite, non-negative vertical height of glyphs (default: FontSize.Px(20.0))."""

    @font_size.setter
    def font_size(self, value: FontSize | float) -> None: ...

    font_smoothing: FontSmoothing
    """Antialiasing method (default: AntiAliased)"""

    weight: FontWeight
    """Font weight / boldness (default: FontWeight.NORMAL)"""

    width: FontWidth
    """Font width / stretch (default: FontWidth.NORMAL)"""

    style: FontStyle
    """Font slant style (default: FontStyle.Normal())"""

    font_features: FontFeatures
    """OpenType font features (default: none)"""

    def __init__(
        self,
        font: FontSource | Handle | str | None = None,
        font_size: FontSize | float | None = None,
        font_smoothing: FontSmoothing = FontSmoothing.AntiAliased,
        weight: FontWeight = FontWeight.NORMAL,
        width: FontWidth = FontWidth.NORMAL,
        style: FontStyle = FontStyle.Normal(),
        font_features: FontFeatures = ...,
    ) -> None:
        """Create a text font component. Defaults to FontSize.Px(20.0); supplied sizes must be finite and non-negative."""

    @staticmethod
    def from_font_size(font_size: FontSize | float) -> TextFont:
        """Create TextFont with a finite, non-negative font size and defaults."""

    @staticmethod
    def from_font_weight(weight: FontWeight) -> TextFont:
        """Create TextFont with the specified font weight and defaults."""

    def with_font(self, font: Handle) -> TextFont:
        """Return a new TextFont with the specified font handle."""

    def with_family(self, family: str) -> TextFont:
        """Return a new TextFont with the font resolved by family name."""

    def with_font_size(self, font_size: FontSize | float) -> TextFont:
        """Return a new TextFont with the specified finite, non-negative font size."""

    def with_font_smoothing(self, font_smoothing: FontSmoothing) -> TextFont:
        """Return a new TextFont with the specified font smoothing."""

    def with_font_weight(self, weight: FontWeight) -> TextFont:
        """Return a new TextFont with the specified font weight."""

    def __eq__(self, other: object) -> bool: ...

class TextLayout(Component):
    """Text layout configuration for alignment and wrapping."""

    justify: Justify
    """Text alignment"""

    linebreak: LineBreak
    """Line breaking behavior"""

    def __init__(
        self, justify: Justify = Justify.Left, linebreak: LineBreak = LineBreak.WordBoundary
    ) -> None:
        """Create text layout configuration."""

    @staticmethod
    def new_with_justify(justify: Justify) -> TextLayout:
        """Create a new TextLayout with specific justification."""

    @staticmethod
    def new_with_linebreak(linebreak: LineBreak) -> TextLayout:
        """Create a new TextLayout with specific line break behavior."""

    @staticmethod
    def new_with_no_wrap() -> TextLayout:
        """Create a new TextLayout with wrapping disabled."""

    def with_justify(self, justify: Justify) -> TextLayout:
        """Return a new TextLayout with the specified justification."""

    def with_linebreak(self, linebreak: LineBreak) -> TextLayout:
        """Return a new TextLayout with the specified line break behavior."""

    def with_no_wrap(self) -> TextLayout:
        """Return a new TextLayout with wrapping disabled."""

class Text2d(Component):
    """2D text component for rendering text in world space.

    Automatically includes TextLayout, TextFont, TextColor, and TextBounds
    components via Bevy's component requirements system.

    Example:
        >>> from pybevy.text import Text2d, TextFont, TextColor
        >>> from pybevy.color import Color
        >>> from pybevy.transform import Transform
        >>>
        >>> # Basic text
        >>> app.spawn(Text2d("Hello World!"))
        >>>
        >>> # Styled text
        >>> app.spawn((
        >>>     Text2d("Hello!"),
        >>>     TextFont.from_font_size(48.0),
        >>>     TextColor(Color.srgb(1.0, 0.5, 0.0)),
        >>>     Transform.from_xyz(0.0, 0.0, 0.0),
        >>> ))
    """

    def __init__(self, text: str) -> None:
        """Create a 2D text component.

        Args:
            text: The text string to display
        """

    @property
    def text(self) -> str:
        """The text string to display."""

    @text.setter
    def text(self, value: str) -> None: ...

class Text2dShadow(Component):
    """Adds a shadow behind Text2d text.

    Creates a visual shadow effect by rendering the text twice -
    once offset with the shadow color, then the main text on top.

    Example:
        >>> from pybevy.text import Text2d, Text2dShadow
        >>> from pybevy.color import Color
        >>> from pybevy.math import Vec2
        >>>
        >>> # Add shadow to text
        >>> app.spawn((
        >>>     Text2d("Hello!"),
        >>>     Text2dShadow(Vec2(4.0, -4.0), Color.BLACK),
        >>> ))
        >>>
        >>> # Colored shadow with custom offset
        >>> app.spawn((
        >>>     Text2d("Glowing!"),
        >>>     Text2dShadow(Vec2(2.0, -2.0), Color.srgb(1.0, 0.0, 0.0)),
        >>> ))
    """

    offset: Vec2
    """Shadow displacement from text position.

    With a value of (0, 0) the shadow will be hidden directly behind the text.
    Positive x moves right, negative y moves down.
    """

    color: Color
    """Color of the shadow (default: black)"""

    def __init__(
        self, offset: Vec2 | None = None, color: Color | None = None
    ) -> None:
        """Create a text shadow component.

        Args:
            offset: Shadow displacement (default: Vec2(4.0, -4.0))
            color: Shadow color (default: Color.BLACK)
        """

class TextSpan(Component):
    """A span of text in a tree of spans.

    TextSpan is used to create multi-style text by having child entities
    with different text styles. A TextSpan must be a child of an entity
    with Text2d (or Text in UI). Each TextSpan can have its own TextFont
    and TextColor, allowing rich formatted text.

    Note:
        TextSpan automatically requires TextFont and TextColor components
        via Bevy's component requirements system. Bevy will add defaults
        if not provided.

    Example:
        >>> from pybevy.text import Text2d, TextSpan, TextFont, TextColor
        >>> from pybevy.color import Color
        >>> from pybevy.ecs import ChildOf
        >>>
        >>> # Create multi-style text
        >>> def setup(commands: Commands):
        >>>     # Root text entity
        >>>     root = commands.spawn((
        >>>         Text2d("Hello "),
        >>>         TextFont.from_font_size(32.0),
        >>>         TextColor(Color.WHITE),
        >>>     ))
        >>>
        >>>     # Add bold red child span
        >>>     commands.spawn((
        >>>         TextSpan("World!"),
        >>>         TextFont.from_font_size(48.0),
        >>>         TextColor(Color.srgb(1.0, 0.0, 0.0)),
        >>>         ChildOf(root),
        >>>     ))
    """

    text: str
    """The text string to display"""

    def __init__(self, text: str = "") -> None:
        """Create a new text span component.

        Args:
            text: The text string to display (default: empty string)
        """

class TextBackgroundColor(Component):
    """The background color of the text for this section.

    Used to create highlighting effects or emphasis on text spans.

    Example:
        >>> from pybevy.text import Text2d, TextBackgroundColor
        >>> from pybevy.color import Color
        >>>
        >>> # Create highlighted text
        >>> commands.spawn((
        >>>     Text2d("Highlighted!"),
        >>>     TextBackgroundColor(Color.srgb(1.0, 1.0, 0.0)),  # Yellow
        >>> ))
    """

    BLACK: ClassVar[TextBackgroundColor]
    WHITE: ClassVar[TextBackgroundColor]

    color: Color
    """Background color"""

    def __init__(self, color: Color = Color.BLACK) -> None:
        """Create a text background color component.

        Args:
            color: Background color (default: black)
        """

    @staticmethod
    def from_numpy(*, color: np.typing.ArrayLike | None = None) -> Batchable: ...  # type: ignore[override]

    def __eq__(self, other: object) -> bool: ...

class Strikethrough(Component):
    """Marker component for strikethrough text decoration."""

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class StrikethroughColor(Component):
    """Color for strikethrough text decoration."""

    color: Color

    def __init__(self, color: Color = ...) -> None: ...
    @staticmethod
    def from_numpy(*, color: np.typing.ArrayLike | None = None) -> Batchable: ...  # type: ignore[override]

class Underline(Component):
    """Marker component for underline text decoration."""

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class UnderlineColor(Component):
    """Color for underline text decoration."""

    color: Color

    def __init__(self, color: Color = ...) -> None: ...
    @staticmethod
    def from_numpy(*, color: np.typing.ArrayLike | None = None) -> Batchable: ...  # type: ignore[override]

class LetterSpacing(Component):
    """Spacing between characters. Construct via :meth:`px` or :meth:`rem`.

    The default constructor yields ``Px(0.0)``.
    """

    class Px(LetterSpacing):
        """Spacing in pixels."""

        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    class Rem(LetterSpacing):
        """Spacing as a multiple of the font size."""

        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float) -> None: ...

    def __eq__(self, other: object) -> bool: ...


class TextEdit:
    """Deferred text edit/navigation command applied by the text systems."""

    class Copy(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Cut(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Paste(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Insert(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    class Backspace(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BackspaceWord(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Delete(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class DeleteWord(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Left(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class Right(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class WordLeft(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class WordRight(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class Up(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class Down(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class TextStart(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class TextEnd(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class HardLineStart(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class HardLineEnd(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class LineStart(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class LineEnd(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: bool
        def __init__(self, value: bool) -> None: ...

    class CollapseSelection(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SelectAll(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SelectAllIfCollapsed(TextEdit):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MoveToPoint(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: tuple[float, float]
        def __init__(self, value: tuple[float, float]) -> None: ...

    class SelectWordAtPoint(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: tuple[float, float]
        def __init__(self, value: tuple[float, float]) -> None: ...

    class SelectLineAtPoint(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: tuple[float, float]
        def __init__(self, value: tuple[float, float]) -> None: ...

    class SelectedHardLineAtPoint(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: tuple[float, float]
        def __init__(self, value: tuple[float, float]) -> None: ...

    class ExtendSelectionToPoint(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: tuple[float, float]
        def __init__(self, value: tuple[float, float]) -> None: ...

    class ShiftClickExtension(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: tuple[float, float]
        def __init__(self, value: tuple[float, float]) -> None: ...

    class ImeSetCompose(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"], Literal["cursor"]]]
        value: str
        cursor: tuple[int, int] | None
        def __init__(self, value: str, cursor: tuple[int, int] | None) -> None: ...

    class ImeCommit(TextEdit):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    def __eq__(self, other: object) -> bool: ...


class EditableText(Component):
    """An editable text input field.

    Spawning this component creates an editable text widget; typing, cursor
    movement, selection, and clipboard are handled by Bevy's systems at runtime.
    """

    def __init__(
        self,
        text: str = "",
        cursor_width: float = 0.2,
        cursor_blink_period: timedelta | float | int | None = None,
        max_characters: int | None = None,
        visible_lines: float | None = 1.0,
        visible_width: float | None = None,
        allow_newlines: bool = False,
    ) -> None: ...

    @property
    def value(self) -> str:
        """The current text content."""

    def clear(self) -> None:
        """Clear the text buffer and any pending edits."""

    def queue_edit(self, edit: TextEdit) -> None:
        """Queue a text edit command; applied by the text systems next frame."""

    def is_composing(self) -> bool:
        """True while the IME is composing text for this input."""

    @property
    def max_characters(self) -> int | None: ...
    @max_characters.setter
    def max_characters(self, value: int | None) -> None: ...

    @property
    def allow_newlines(self) -> bool: ...
    @allow_newlines.setter
    def allow_newlines(self, value: bool) -> None: ...

    @property
    def cursor_width(self) -> float: ...
    @cursor_width.setter
    def cursor_width(self, value: float) -> None: ...

    @property
    def cursor_blink_period(self) -> timedelta: ...
    @cursor_blink_period.setter
    def cursor_blink_period(self, value: timedelta | float | int) -> None: ...

    @property
    def visible_lines(self) -> float | None: ...
    @visible_lines.setter
    def visible_lines(self, value: float | None) -> None: ...

    @property
    def visible_width(self) -> float | None: ...
    @visible_width.setter
    def visible_width(self, value: float | None) -> None: ...

class TextCursorStyle(Component):
    """Cursor and selection appearance for an entity's EditableText. Bevy defaults:
    slate-700 cursor, sky-300 selection, sky-400 unfocused selection."""

    def __init__(
        self,
        color: Color = ...,
        selection_color: Color = ...,
        unfocused_selection_color: Color = ...,
        selected_text_color: Color | None = None,
    ) -> None: ...
    @property
    def color(self) -> Color: ...
    @color.setter
    def color(self, value: Color) -> None: ...
    @property
    def selection_color(self) -> Color: ...
    @selection_color.setter
    def selection_color(self, value: Color) -> None: ...
    @property
    def unfocused_selection_color(self) -> Color: ...
    @unfocused_selection_color.setter
    def unfocused_selection_color(self, value: Color) -> None: ...
    @property
    def selected_text_color(self) -> Color | None: ...
    @selected_text_color.setter
    def selected_text_color(self, value: Color | None) -> None: ...

class TextPlugin(Plugin):
    """Plugin that adds text rendering support to the app.

    This plugin is included by default in DefaultPlugins. You only need to
    add it manually if you're building a custom plugin group or headless app
    that needs text support.

    The plugin automatically:
    - Registers Font asset loader
    - Initializes text rendering resources (TextPipeline, CosmicFontSystem, etc.)
    - Sets up font atlas management
    - Adds text update systems to PostUpdate schedule
    - Loads default font (FiraMono) if available

    Example:
        >>> from pybevy.text import TextPlugin
        >>> from pybevy.app import App
        >>>
        >>> # Manually add text plugin (usually not needed)
        >>> app = App()
        >>> app.add_plugins(TextPlugin())
        >>>
        >>> # Text plugin is included in DefaultPlugins
        >>> from pybevy.app import DefaultPlugins
        >>> app.add_plugins(DefaultPlugins)  # TextPlugin auto-included
    """

    def __init__(self) -> None:
        """Create a new TextPlugin."""

    def build(self, app: App) -> None:
        """Build and apply the plugin to the app."""
