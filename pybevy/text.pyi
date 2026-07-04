from collections.abc import Iterator
from typing import ClassVar

from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.color import Color
from pybevy.ecs import Component, Resource
from pybevy.image import TextureAtlasLayout
from pybevy.math import Vec2

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

    def __init__(self) -> None:
        """Create a LineHeight with the default value."""

    @staticmethod
    def Px(pixels: float) -> LineHeight:
        """Set line height to a specific number of pixels."""

    @staticmethod
    def RelativeToFont(scale: float) -> LineHeight:
        """Set line height as a multiple of the font size.

        Args:
            scale: Multiplier relative to font size (default: 1.2)
        """

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
    def __hash__(self) -> int: ...

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
    def __hash__(self) -> int: ...

class FontStyle:
    """The slant style of a font face: normal, italic, or oblique.

    Example:
        >>> from pybevy.text import TextFont, FontStyle
        >>> TextFont(font_size=24.0, style=FontStyle.Italic())
        >>> TextFont(font_size=24.0, style=FontStyle.Oblique(14.0))
    """

    @staticmethod
    def Normal() -> FontStyle:
        """A face that is neither italic nor obliqued (default)."""

    @staticmethod
    def Italic() -> FontStyle:
        """A form that is generally cursive in nature."""

    @staticmethod
    def Oblique(angle: float | None = None, /) -> FontStyle:
        """A sloped version of the regular face, with an optional slant angle in degrees."""

    @property
    def _0(self) -> float | None:
        """The Oblique slant angle in degrees (only present on Oblique)."""

    def __eq__(self, other: object) -> bool: ...

class FontHinting:
    """Font hinting strategy controlling glyph rasterization."""

    Disabled: FontHinting
    """Glyphs are rasterized without hinting (default)."""

    Enabled: FontHinting
    """Glyphs are rasterized with hinting."""

class FontFeatureTag:
    """A single OpenType feature tag (4 ASCII characters).

    Represents an OpenType feature like "liga" (ligatures) or "smcp" (small caps).

    Example:
        >>> tag = FontFeatureTag("liga")
        >>> assert tag == FontFeatureTag("liga")
    """

    STANDARD_LIGATURES: ClassVar[FontFeatureTag]
    """Standard ligatures ("liga")."""

    SMALL_CAPS: ClassVar[FontFeatureTag]
    """Small capitals ("smcp")."""

    OLDSTYLE_FIGURES: ClassVar[FontFeatureTag]
    """Old-style figures ("onum")."""

    TABULAR_FIGURES: ClassVar[FontFeatureTag]
    """Tabular figures ("tnum")."""

    FRACTIONS: ClassVar[FontFeatureTag]
    """Fractions ("frac")."""

    SLASHED_ZERO: ClassVar[FontFeatureTag]
    """Slashed zero ("zero")."""

    def __init__(self, tag: str) -> None:
        """Create a FontFeatureTag from a 4-character ASCII string.

        Raises:
            ValueError: If tag is not exactly 4 ASCII characters.
        """

    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...
    def __repr__(self) -> str: ...

class FontFeatures:
    """OpenType font features for .otf fonts that support them.

    Provides a builder-style API for specifying OpenType features like
    ligatures, small caps, and figure styles.

    Example:
        >>> from pybevy.text import TextFont, FontFeatures
        >>>
        >>> # Enable ligatures and small caps
        >>> features = FontFeatures().enable("liga").enable("smcp")
        >>> TextFont(font_size=24.0, font_features=features)
        >>>
        >>> # Set weight variation
        >>> features = FontFeatures().set("wght", 300)
        >>>
        >>> # Use convenience constructors
        >>> features = FontFeatures.small_caps()
        >>> features = FontFeatures.tabular_figures()
    """

    def __init__(self) -> None:
        """Create empty FontFeatures (no features enabled)."""

    def enable(self, tag: str) -> FontFeatures:
        """Enable an OpenType feature (sets its value to 1).

        Args:
            tag: 4-character OpenType feature tag (e.g., "liga", "smcp")

        Returns:
            self for method chaining
        """

    def disable(self, tag: str) -> FontFeatures:
        """Disable an OpenType feature (sets its value to 0).

        Args:
            tag: 4-character OpenType feature tag

        Returns:
            self for method chaining
        """

    def set(self, tag: str, value: int) -> FontFeatures:
        """Set an OpenType feature to a specific value.

        For most features, ``enable()`` or ``disable()`` should be used instead.
        Some features like "wght" take numeric values.

        Args:
            tag: 4-character OpenType feature tag
            value: Feature value

        Returns:
            self for method chaining
        """

    @staticmethod
    def standard_ligatures() -> FontFeatures:
        """Create FontFeatures with standard ligatures ("liga") enabled."""

    @staticmethod
    def small_caps() -> FontFeatures:
        """Create FontFeatures with small caps ("smcp") enabled."""

    @staticmethod
    def oldstyle_figures() -> FontFeatures:
        """Create FontFeatures with oldstyle figures ("onum") enabled."""

    @staticmethod
    def tabular_figures() -> FontFeatures:
        """Create FontFeatures with tabular figures ("tnum") enabled."""

    @staticmethod
    def slashed_zero() -> FontFeatures:
        """Create FontFeatures with slashed zero ("zero") enabled."""

    @staticmethod
    def fractions() -> FontFeatures:
        """Create FontFeatures with fractions ("frac") enabled."""

    def __eq__(self, other: object) -> bool: ...

class FontSmoothing:
    """Antialiasing method for text rendering."""

    None_: FontSmoothing
    """No antialiasing - for pixel art aesthetic"""

    AntiAliased: FontSmoothing
    """Grayscale antialiasing (default)"""

    def __eq__(self, other: object) -> bool: ...

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
    """Maximum width and height constraints for text.

    Text will wrap according to bounds. Characters completely outside
    bounds after wrapping are truncated.
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

    def __eq__(self, other: object) -> bool: ...

class FontSize:
    """The vertical height of rasterized glyphs, in one of several units.

    Wherever a FontSize is accepted, a plain float is also accepted and
    treated as `FontSize.Px` (mirrors bevy's `From<f32> for FontSize`).
    """

    value: float
    """The numeric payload of the variant."""

    @staticmethod
    def Px(value: float) -> FontSize:
        """Font size in logical pixels."""

    @staticmethod
    def Vw(value: float) -> FontSize:
        """Font size as a percentage of the viewport width."""

    @staticmethod
    def Vh(value: float) -> FontSize:
        """Font size as a percentage of the viewport height."""

    @staticmethod
    def VMin(value: float) -> FontSize:
        """Font size as a percentage of the smaller viewport dimension."""

    @staticmethod
    def VMax(value: float) -> FontSize:
        """Font size as a percentage of the larger viewport dimension."""

    @staticmethod
    def Rem(value: float) -> FontSize:
        """Font size relative to the RemSize resource."""

    def __eq__(self, other: object) -> bool: ...

class FontSource:
    """Where a TextFont resolves its font face from.

    A specific Font asset handle, a font family name resolved through the
    font database, or a generic font category (serif, monospace, ...).
    Wherever a FontSource is accepted, a Handle is treated as
    `FontSource.Handle` and a str as `FontSource.Family` (mirrors bevy's
    `From` impls).
    """

    @staticmethod
    def Handle(value: Handle) -> FontSource:
        """A specific font face referenced by a Font asset handle."""

    @staticmethod
    def Family(value: str) -> FontSource:
        """Resolve the font by family name using the font database."""

    @staticmethod
    def Serif() -> FontSource:
        """Fonts with serifs."""

    @staticmethod
    def SansSerif() -> FontSource:
        """Fonts without serifs."""

    @staticmethod
    def Cursive() -> FontSource:
        """Fonts with a cursive or handwritten style."""

    @staticmethod
    def Fantasy() -> FontSource:
        """Decorative or expressive fonts."""

    @staticmethod
    def Monospace() -> FontSource:
        """Fonts with a fixed advance width."""

    @staticmethod
    def SystemUi() -> FontSource:
        """The default user interface system font."""

    @staticmethod
    def UiSerif() -> FontSource:
        """Alternative serif font for user interfaces."""

    @staticmethod
    def UiSansSerif() -> FontSource:
        """Alternative sans-serif font for user interfaces."""

    @staticmethod
    def UiMonospace() -> FontSource:
        """Alternative monospace font for user interfaces."""

    @staticmethod
    def UiRounded() -> FontSource:
        """Fonts with rounded features."""

    @staticmethod
    def Emoji() -> FontSource:
        """Fonts designed to render emoji."""

    @staticmethod
    def Math() -> FontSource:
        """Fonts for mathematical notation."""

    @staticmethod
    def FangSong() -> FontSource:
        """Chinese characters between Song and Kai forms."""

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
        """Vertical height of glyphs (default: FontSize.Px(20.0))."""

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
        """Create a text font component. Defaults: the default font, FontSize.Px(20.0)."""

    @staticmethod
    def from_font_size(font_size: FontSize | float) -> TextFont:
        """Create TextFont with specified font size and defaults."""

    def with_font(self, font: Handle) -> TextFont:
        """Return a new TextFont with the specified font handle."""

    def with_family(self, family: str) -> TextFont:
        """Return a new TextFont with the font resolved by family name."""

    def with_font_size(self, font_size: FontSize | float) -> TextFont:
        """Return a new TextFont with the specified font size."""

    def with_font_smoothing(self, font_smoothing: FontSmoothing) -> TextFont:
        """Return a new TextFont with the specified font smoothing."""

    def with_weight(self, weight: FontWeight) -> TextFont:
        """Return a new TextFont with the specified font weight."""

    def with_font_features(self, font_features: FontFeatures) -> TextFont:
        """Return a new TextFont with the specified font features."""

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
        >>>     Text2dShadow(Vec2(4.0, -4.0), Color.BLACK()),
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
            color: Shadow color (default: Color.BLACK())
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
        >>>         TextColor(Color.WHITE()),
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

    def __eq__(self, other: object) -> bool: ...

class Strikethrough(Component):
    """Marker component for strikethrough text decoration."""

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class StrikethroughColor(Component):
    """Color for strikethrough text decoration."""

    color: Color

    def __init__(self, color: Color = ...) -> None: ...

class Underline(Component):
    """Marker component for underline text decoration."""

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class UnderlineColor(Component):
    """Color for underline text decoration."""

    color: Color

    def __init__(self, color: Color = ...) -> None: ...

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
