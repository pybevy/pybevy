"""Bevy UI system bindings for PyBevy.

Provides UI text rendering and layout components. For text styling, use
TextFont, TextColor, and TextLayout from pybevy.text (same components
work for both Text2d and UI Text).
"""

from typing import ClassVar, Literal

import numpy as np

from pybevy.assets import Handle
from pybevy.color import Color
from pybevy.ecs import Batchable, Component, Entity, Resource
from pybevy.image import Image
from pybevy.math import Rect, Rot2, Vec2
from pybevy.sprite import TextureSlicer

class Val:
    """UI sizing value.

    Represents different ways to specify sizes in the UI system.
    Supports pixels, percentages, auto-sizing, and viewport-relative units.

    Example:
        ```python
        from pybevy.ui import Val, Node

        # Different ways to specify sizes
        node = Node()
        node.width = 100.0  # Simplified (treated as pixels)

        # Or use Val explicitly for more control
        width_px = Val.px(100.0)  # 100 pixels
        width_percent = Val.percent(50.0)  # 50% of parent
        width_auto = Val.AUTO()  # Auto-size based on content
        width_vw = Val.vw(50.0)  # 50% of viewport width
        ```
    """

    @staticmethod
    def px(value: float) -> Val:
        """Create a pixel value.

        Args:
            value: Size in pixels

        Returns:
            Val instance representing pixel size
        """

    @staticmethod
    def percent(value: float) -> Val:
        """Create a percentage value.

        Args:
            value: Percentage (0-100)

        Returns:
            Val instance representing percentage of parent size
        """

    @staticmethod
    def auto() -> Val:
        """Create an auto value.

        Auto-sizes based on content.

        Returns:
            Val instance for auto-sizing
        """

    @staticmethod
    def vw(value: float) -> Val:
        """Create a viewport width percentage value.

        Args:
            value: Percentage of viewport width (0-100)

        Returns:
            Val instance for viewport-relative width
        """

    @staticmethod
    def vh(value: float) -> Val:
        """Create a viewport height percentage value.

        Args:
            value: Percentage of viewport height (0-100)

        Returns:
            Val instance for viewport-relative height
        """

    @staticmethod
    def vmin(value: float) -> Val:
        """Create a viewport minimum dimension percentage value.

        Args:
            value: Percentage of smallest viewport dimension

        Returns:
            Val instance for min(viewport width, viewport height) percentage
        """

    @staticmethod
    def vmax(value: float) -> Val:
        """Create a viewport maximum dimension percentage value.

        Args:
            value: Percentage of largest viewport dimension

        Returns:
            Val instance for max(viewport width, viewport height) percentage
        """

    @property
    def px_value(self) -> float | None:
        """Get pixel value if this is a Px variant, None otherwise."""

    @property
    def percent_value(self) -> float | None:
        """Get percentage value if this is a Percent variant, None otherwise."""

    @property
    def is_auto(self) -> bool:
        """True if this is an Auto value."""

    ZERO: ClassVar[Val]

    def left(self) -> UiRect:
        """Returns a UiRect with only left set to this value, others ZERO."""

    def right(self) -> UiRect:
        """Returns a UiRect with only right set to this value, others ZERO."""

    def top(self) -> UiRect:
        """Returns a UiRect with only top set to this value, others ZERO."""

    def bottom(self) -> UiRect:
        """Returns a UiRect with only bottom set to this value, others ZERO."""

    def all(self) -> UiRect:
        """Returns a UiRect with all sides set to this value."""

    def horizontal(self) -> UiRect:
        """Returns a UiRect with left and right set to this value, top/bottom ZERO."""

    def vertical(self) -> UiRect:
        """Returns a UiRect with top and bottom set to this value, left/right ZERO."""

    def __eq__(self, other: object) -> bool: ...


class UiRect:
    """A rectangular UI region defined by left, right, top, and bottom values.

    Used for margins, padding, and borders in UI layout.

    Example:
        ```python
        from pybevy.ui import UiRect, Val

        # Same value for all sides
        margin = UiRect.all(Val.px(10))

        # Different values per side
        padding = UiRect(Val.px(5), Val.px(10), Val.px(15), Val.px(20))

        # Horizontal and vertical
        border = UiRect.axes(Val.px(2), Val.px(4))
        ```
    """

    ZERO: ClassVar[UiRect]
    AUTO: ClassVar[UiRect]
    DEFAULT: ClassVar[UiRect]

    def __init__(
        self,
        left: Val = ...,
        right: Val = ...,
        top: Val = ...,
        bottom: Val = ...,
    ) -> None: ...

    @staticmethod
    def new(left: Val, right: Val, top: Val, bottom: Val) -> UiRect:
        """Create a UiRect with the specified values for each side."""

    @staticmethod
    def all(value: Val) -> UiRect:
        """Create a UiRect with the same value for all sides."""

    @staticmethod
    def px(left: float, right: float, top: float, bottom: float) -> UiRect:
        """Create a UiRect with pixel values for each side."""

    @staticmethod
    def percent(left: float, right: float, top: float, bottom: float) -> UiRect:
        """Create a UiRect with percentage values for each side."""

    @staticmethod
    def horizontal(value: Val) -> UiRect:
        """Create a UiRect with left and right set to value, top/bottom ZERO."""

    @staticmethod
    def vertical(value: Val) -> UiRect:
        """Create a UiRect with top and bottom set to value, left/right ZERO."""

    @staticmethod
    def axes(horizontal: Val, vertical: Val) -> UiRect:
        """Create a UiRect with horizontal value for left/right, vertical for top/bottom."""

    @staticmethod
    def left(left: Val) -> UiRect:
        """Create a UiRect with only left set, others ZERO."""

    @staticmethod
    def right(right: Val) -> UiRect:
        """Create a UiRect with only right set, others ZERO."""

    @staticmethod
    def top(top: Val) -> UiRect:
        """Create a UiRect with only top set, others ZERO."""

    @staticmethod
    def bottom(bottom: Val) -> UiRect:
        """Create a UiRect with only bottom set, others ZERO."""

    def with_left(self, left: Val) -> UiRect:
        """Return a new UiRect with left modified."""

    def with_right(self, right: Val) -> UiRect:
        """Return a new UiRect with right modified."""

    def with_top(self, top: Val) -> UiRect:
        """Return a new UiRect with top modified."""

    def with_bottom(self, bottom: Val) -> UiRect:
        """Return a new UiRect with bottom modified."""

    def get_left(self) -> Val:
        """Get the left value."""

    def set_left(self, value: Val) -> None:
        """Set the left value."""

    def get_right(self) -> Val:
        """Get the right value."""

    def set_right(self, value: Val) -> None:
        """Set the right value."""

    def get_top(self) -> Val:
        """Get the top value."""

    def set_top(self, value: Val) -> None:
        """Set the top value."""

    def get_bottom(self) -> Val:
        """Get the bottom value."""

    def set_bottom(self, value: Val) -> None:
        """Set the bottom value."""

    def __eq__(self, other: object) -> bool: ...


class FlexDirection:
    """Flex direction enum for Node layout.

    Defines the direction of the main axis in a flex container.

    Example:
        ```python
        from pybevy.ui import Node, FlexDirection

        node = Node()
        node.flex_direction = FlexDirection.Column  # Stack children vertically
        ```
    """

    Row: FlexDirection
    """Horizontal layout, left to right (default)."""

    Column: FlexDirection
    """Vertical layout, top to bottom."""

    RowReverse: FlexDirection
    """Horizontal layout, right to left."""

    ColumnReverse: FlexDirection
    """Vertical layout, bottom to top."""


class Display:
    """Display mode enum for Node.

    Defines how the node participates in layout.

    Example:
        ```python
        from pybevy.ui import Node, Display

        node = Node()
        node.display = Display.Flex  # Use flexbox layout (default)
        ```
    """

    Flex: Display
    """Use flexbox layout (default)."""

    Grid: Display
    """Use grid layout."""

    Block: Display
    """Use block layout."""

    None_: Display
    """Don't display (hidden)."""


class AlignItems:
    """Align items enum for Node flexbox layout.

    Defines how children are aligned along the cross axis.

    Example:
        ```python
        from pybevy.ui import Node, AlignItems

        node = Node()
        node.align_items = AlignItems.Center  # Center children vertically
        ```
    """

    Default: AlignItems
    """Default alignment."""

    Start: AlignItems
    """Align to start of cross axis."""

    End: AlignItems
    """Align to end of cross axis."""

    FlexStart: AlignItems
    """Align to flex start."""

    FlexEnd: AlignItems
    """Align to flex end."""

    Center: AlignItems
    """Center on cross axis."""

    Baseline: AlignItems
    """Align to text baseline."""

    Stretch: AlignItems
    """Stretch to fill cross axis."""


class JustifyContent:
    """Justify content enum for Node flexbox layout.

    Defines how children are distributed along the main axis.

    Example:
        ```python
        from pybevy.ui import Node, JustifyContent

        node = Node()
        node.justify_content = JustifyContent.SpaceBetween  # Space items evenly
        ```
    """

    Default: JustifyContent
    """Default justification."""

    Start: JustifyContent
    """Pack items to start."""

    End: JustifyContent
    """Pack items to end."""

    FlexStart: JustifyContent
    """Pack items to flex start."""

    FlexEnd: JustifyContent
    """Pack items to flex end."""

    Center: JustifyContent
    """Pack items to center."""

    SpaceBetween: JustifyContent
    """Space evenly with no space at edges."""

    SpaceAround: JustifyContent
    """Space evenly with half space at edges."""

    SpaceEvenly: JustifyContent
    """Space evenly with full space at edges."""

    Stretch: JustifyContent
    """Stretch items to fill."""


class AlignSelf:
    """Align self enum for individual node alignment.

    Overrides parent's align_items for this specific node.

    Example:
        ```python
        from pybevy.ui import Node, AlignSelf

        node = Node()
        node.align_self = AlignSelf.Center  # This node centers itself
        ```
    """

    Auto: AlignSelf
    """Use parent's align_items (default)."""

    Start: AlignSelf
    """Align to start of cross axis."""

    End: AlignSelf
    """Align to end of cross axis."""

    FlexStart: AlignSelf
    """Align to flex start."""

    FlexEnd: AlignSelf
    """Align to flex end."""

    Center: AlignSelf
    """Center on cross axis."""

    Baseline: AlignSelf
    """Align to text baseline."""

    Stretch: AlignSelf
    """Stretch to fill cross axis."""


class FlexWrap:
    """Flex wrap enum for Node flexbox layout.

    Defines whether children wrap to multiple lines.

    Example:
        ```python
        from pybevy.ui import Node, FlexWrap

        node = Node()
        node.flex_wrap = FlexWrap.Wrap  # Allow wrapping to multiple lines
        ```
    """

    NoWrap: FlexWrap
    """Don't wrap, single line (default)."""

    Wrap: FlexWrap
    """Wrap to multiple lines."""

    WrapReverse: FlexWrap
    """Wrap in reverse order."""


class InlineDirection:
    """Inline (reading) direction for Node layout.

    Example:
        ```python
        from pybevy.ui import Node, InlineDirection

        node = Node()
        node.direction = InlineDirection.Rtl  # Right-to-left
        ```
    """

    Ltr: InlineDirection
    """Left-to-right (default)."""

    Rtl: InlineDirection
    """Right-to-left."""


class AlignContent:
    """Align content enum for Node flexbox layout.

    Controls alignment of lines within a flex container when there's extra
    space in the cross-axis (only applies with flex-wrap).

    Example:
        ```python
        from pybevy.ui import Node, AlignContent, FlexWrap

        node = Node()
        node.flex_wrap = FlexWrap.Wrap
        node.align_content = AlignContent.SpaceBetween
        ```
    """

    Default: AlignContent
    """Default alignment."""

    Start: AlignContent
    """Pack lines to start of cross axis."""

    End: AlignContent
    """Pack lines to end of cross axis."""

    FlexStart: AlignContent
    """Pack lines to flex start."""

    FlexEnd: AlignContent
    """Pack lines to flex end."""

    Center: AlignContent
    """Pack lines to center."""

    Stretch: AlignContent
    """Stretch lines to fill."""

    SpaceBetween: AlignContent
    """Space evenly with no space at edges."""

    SpaceEvenly: AlignContent
    """Space evenly with full space at edges."""

    SpaceAround: AlignContent
    """Space evenly with half space at edges."""


class OverflowAxis:
    """Overflow behavior for a single axis.

    Defines how overflowing content is handled on one axis.

    Example:
        ```python
        from pybevy.ui import Overflow, OverflowAxis

        overflow = Overflow()
        overflow.x = OverflowAxis.Scroll
        overflow.y = OverflowAxis.Clip
        ```
    """

    Visible: OverflowAxis
    """Show overflowing content (default)."""

    Clip: OverflowAxis
    """Clip overflowing content."""

    Hidden: OverflowAxis
    """Hide overflowing content (affects layout then clips)."""

    Scroll: OverflowAxis
    """Allow scrolling of overflowing content."""

    def is_visible(self) -> bool:
        """True if overflow is visible on this axis."""


class Overflow:
    """Overflow control for UI nodes.

    Controls how content that exceeds the node's bounds is handled.
    Supports independent control of x and y axes.

    Example:
        ```python
        from pybevy.ui import Node, Overflow

        node = Node()
        node.overflow = Overflow.clip()  # Clip both axes

        # Or scroll only vertically
        node.overflow = Overflow.scroll_y()

        # Or use individual axis control
        overflow = Overflow()
        overflow.x = OverflowAxis.Visible
        overflow.y = OverflowAxis.Scroll
        ```
    """

    DEFAULT: ClassVar[Overflow]

    def __init__(
        self, x: OverflowAxis = ..., y: OverflowAxis = ...
    ) -> None: ...

    @staticmethod
    def visible() -> Overflow:
        """Show overflowing content on both axes."""

    @staticmethod
    def clip() -> Overflow:
        """Clip overflowing content on both axes."""

    @staticmethod
    def clip_x() -> Overflow:
        """Clip on x axis, visible on y axis."""

    @staticmethod
    def clip_y() -> Overflow:
        """Clip on y axis, visible on x axis."""

    @staticmethod
    def hidden() -> Overflow:
        """Hide overflowing content on both axes (affects layout)."""

    @staticmethod
    def hidden_x() -> Overflow:
        """Hidden on x axis, visible on y axis."""

    @staticmethod
    def hidden_y() -> Overflow:
        """Hidden on y axis, visible on x axis."""

    @staticmethod
    def scroll() -> Overflow:
        """Enable scrolling on both axes."""

    @staticmethod
    def scroll_x() -> Overflow:
        """Scroll on x axis, visible on y axis."""

    @staticmethod
    def scroll_y() -> Overflow:
        """Scroll on y axis, visible on x axis."""

    def is_visible(self) -> bool:
        """True if overflow is visible on both axes."""

    @property
    def x(self) -> OverflowAxis:
        """Overflow behavior on x axis."""

    @x.setter
    def x(self, value: OverflowAxis) -> None: ...

    @property
    def y(self) -> OverflowAxis:
        """Overflow behavior on y axis."""

    @y.setter
    def y(self, value: OverflowAxis) -> None: ...

    def __eq__(self, other: object) -> bool: ...


class PositionType:
    """Position type enum for Node layout.

    Defines how the node is positioned in relation to other nodes.

    Example:
        ```python
        from pybevy.ui import Node, PositionType

        node = Node()
        node.position_type = PositionType.Absolute  # Position absolutely
        node.top = 100.0
        node.left = 50.0
        ```
    """

    Relative: PositionType
    """Position relative to siblings (default)."""

    Absolute: PositionType
    """Position absolutely relative to parent."""


class BoxSizing:
    """Defines how width and height are calculated for a UI node.

    Similar to CSS box-sizing property.

    Example:
        ```python
        from pybevy.ui import BoxSizing

        # Border box includes padding and border in dimensions
        box_sizing = BoxSizing.BorderBox

        # Content box excludes padding and border
        box_sizing = BoxSizing.ContentBox
        ```
    """

    BorderBox: BoxSizing
    """Width/height refer to the border box (including padding and border)."""

    ContentBox: BoxSizing
    """Width/height refer to the content box (excluding padding and border)."""



class VisualBox:
    """Defines which box is used as the clipping boundary for overflow.

    Controls where content is clipped when overflow is set to clip/hidden.

    Example:
        ```python
        from pybevy.ui import VisualBox

        clip_box = VisualBox.ContentBox
        clip_box = VisualBox.PaddingBox
        clip_box = VisualBox.BorderBox
        ```
    """

    ContentBox: VisualBox
    """Clip content outside the content box."""

    PaddingBox: VisualBox
    """Clip content outside the padding box."""

    BorderBox: VisualBox
    """Clip content outside the border box."""



class OverflowClipMargin:
    """Margin around the clipping boundary for overflow.

    Allows specifying a visual box and margin for overflow clipping.

    Example:
        ```python
        from pybevy.ui import OverflowClipMargin, VisualBox

        # Default (content box, 0 margin)
        margin = OverflowClipMargin()

        # Content box clipping
        margin = OverflowClipMargin.content_box()

        # With custom margin
        margin = OverflowClipMargin.border_box().with_margin(10.0)
        ```
    """

    def __init__(
        self, visual_box: VisualBox | None = None, margin: float = 0.0
    ) -> None:
        """Create an overflow clip margin.

        Args:
            visual_box: The clipping boundary box (default: ContentBox)
            margin: Margin width on each edge in logical pixels (default: 0.0)
        """

    @staticmethod
    def content_box() -> OverflowClipMargin:
        """Clip content outside the content box."""

    @staticmethod
    def padding_box() -> OverflowClipMargin:
        """Clip content outside the padding box."""

    @staticmethod
    def border_box() -> OverflowClipMargin:
        """Clip content outside the border box."""

    def with_margin(self, margin: float) -> OverflowClipMargin:
        """Add a margin on each edge of the visual box in logical pixels."""

    @property
    def visual_box(self) -> VisualBox:
        """The visible unclipped area box."""

    @property
    def margin(self) -> float:
        """The margin width on each edge in logical pixels."""

    def __eq__(self, other: object) -> bool: ...


class Val2:
    """A 2D UI value with x and y components.

    Used for specifying 2D values in UI layout, where each component
    can be a different type of Val (Px, Percent, Auto, etc.).

    Example:
        ```python
        from pybevy.ui import Val2, Val

        # Using pixel values
        translation = Val2.px(10.0, 20.0)

        # Using percentages
        offset = Val2.percent(50.0, 50.0)

        # Mixed values
        pos = Val2(Val.px(10.0), Val.Percent(50.0))
        ```
    """

    ZERO: Val2
    """A zero-valued Val2."""

    def __init__(self, x: Val, y: Val) -> None:
        """Create a Val2 with the given x and y values."""

    @staticmethod
    def px(x: float, y: float) -> Val2:
        """Create a Val2 with both components in logical pixels."""

    @staticmethod
    def percent(x: float, y: float) -> Val2:
        """Create a Val2 with both components as percentages."""

    @property
    def x(self) -> Val:
        """The x component."""

    @property
    def y(self) -> Val:
        """The y component."""

    def __eq__(self, other: object) -> bool: ...


class UiTransform(Component):
    """Transforms a UI node with translation, rotation, and scale.

    Applies transformations to UI elements for visual effects and layout
    adjustments. Unlike CSS transforms, this operates in UI coordinate space.

    Example:
        ```python
        from pybevy.ui import UiTransform, Val2
        from pybevy.math import Vec2, Rot2

        # Identity transform
        transform = UiTransform()

        # With translation
        transform = UiTransform.from_translation(Val2.px(100.0, 50.0))

        # With rotation
        transform = UiTransform.from_rotation(Rot2.from_radians(0.5))

        # Full transform
        transform = UiTransform(
            translation=Val2.px(10.0, 20.0),
            scale=Vec2(2.0, 2.0),
            rotation=Rot2.from_radians(0.1),
        )
        ```
    """

    def __init__(
        self,
        translation: Val2 | None = None,
        scale: Vec2 | None = None,
        rotation: Rot2 | None = None,
    ) -> None:
        """Create a UiTransform with optional translation, scale, and rotation."""

    @staticmethod
    def identity() -> UiTransform:
        """Create the identity transform (no translation, rotation, or scale)."""

    @staticmethod
    def from_translation(translation: Val2) -> UiTransform:
        """Create a transform with only translation."""

    @staticmethod
    def from_rotation(rotation: Rot2) -> UiTransform:
        """Create a transform with only rotation."""

    @staticmethod
    def from_scale(scale: Vec2) -> UiTransform:
        """Create a transform with only scale."""

    @property
    def translation(self) -> Val2:
        """The translation in UI coordinates."""

    @translation.setter
    def translation(self, value: Val2) -> None:
        """Set the translation."""

    @property
    def scale(self) -> Vec2:
        """The scale factor."""

    @scale.setter
    def scale(self, value: Vec2) -> None:
        """Set the scale."""

    @property
    def rotation(self) -> Rot2:
        """The rotation."""

    @rotation.setter
    def rotation(self, value: Rot2) -> None:
        """Set the rotation."""

    def __eq__(self, other: object) -> bool: ...


class BorderGradient(Component):
    """A gradient displayed on a UI node's border.

    Example:
        ```python
        from pybevy.ui import BorderGradient, LinearGradient, Gradient

        # Create border with gradient
        gradient = LinearGradient(...)
        border = BorderGradient([Gradient.linear(gradient)])
        ```
    """

    def __init__(self, gradients: list[Gradient]) -> None:
        """Create a BorderGradient with the given gradients."""

    def add_gradient(self, gradient: Gradient) -> None:
        """Add a gradient to the border."""

    @property
    def gradients(self) -> list[Gradient]:
        """Get all gradients."""

    def len(self) -> int:
        """Number of gradients."""

    def is_empty(self) -> bool:
        """Check if empty."""

    def __eq__(self, other: object) -> bool: ...


class Text(Component):
    """UI-space text component.

    Text is used for rendering text within Bevy's UI layout system. It requires
    a Node component for layout positioning, and uses TextFont, TextColor, and
    TextLayout for styling (imported from pybevy.text).

    Example:
        ```python
        from pybevy.ui import Node, PositionType, Text
        from pybevy.text import TextFont, TextColor, TextLayout, Justify
        from pybevy.color import Color

        # Spawn UI text with styling
        node = Node()
        node.position_type = PositionType.Absolute
        node.top = Val.px(10.0)
        node.left = Val.px(10.0)
        commands.spawn((
            Text("Hello, UI!"),
            node,
            TextFont.from_font_size(24.0),
            TextColor(Color.WHITE()),
            TextLayout.with_justify(Justify.Center),
        ))
        ```

    Args:
        content: The text string to display
    """
    def __init__(self, content: str) -> None: ...

    @property
    def content(self) -> str:
        """The text content."""

    @content.setter
    def content(self, value: str) -> None: ...


class Node(Component):
    """UI layout node component.

    Node is the core component for Bevy's flexbox-based UI layout system.
    All UI elements require a Node component to participate in layout.

    Example:
        ```python
        from pybevy.ui import Node, Text, FlexDirection, Display, PositionType

        # Default node (relative positioning)
        commands.spawn((Node(), Text("Centered")))

        # Absolutely positioned node
        node = Node()
        node.position_type = PositionType.Absolute
        node.top = Val.px(10.0)
        node.left = Val.px(10.0)
        commands.spawn((node, Text("Top Left")))

        # Flexbox layout
        node = Node()
        node.flex_direction = FlexDirection.Column  # Stack vertically
        node.display = Display.Flex
        node.width = Val.px(300.0)
        node.height = Val.px(200.0)
        commands.spawn(node)

        # Custom positioning
        node = Node()
        node.position_type = PositionType.Absolute
        node.top = Val.px(50.0)
        node.left = Val.px(100.0)
        commands.spawn((node, Text("Custom Position")))
        ```

    Attributes:
        position_type: PositionType enum (Relative or Absolute)
        flex_direction: FlexDirection enum (Row, Column, RowReverse, ColumnReverse)
        display: Display enum (Flex, Grid, Block, None_)
        align_items: AlignItems enum for cross-axis alignment
        justify_content: JustifyContent enum for main-axis distribution
        align_self: AlignSelf enum for individual alignment
        flex_wrap: FlexWrap enum (NoWrap, Wrap, WrapReverse)
        top: Top position as Val (when absolute)
        left: Left position as Val (when absolute)
        width: Width as Val
        height: Height as Val
    """
    def __init__(self) -> None: ...

    @property
    def position_type(self) -> PositionType:
        """Whether positioning is relative or absolute."""

    @position_type.setter
    def position_type(self, value: PositionType) -> None: ...

    @property
    def top(self) -> Val:
        """Top position (for absolute positioning)."""

    @top.setter
    def top(self, value: Val | float) -> None: ...

    @property
    def left(self) -> Val:
        """Left position (for absolute positioning)."""

    @left.setter
    def left(self, value: Val | float) -> None: ...

    @property
    def width(self) -> Val:
        """Width."""

    @width.setter
    def width(self, value: Val | float) -> None: ...

    @property
    def height(self) -> Val:
        """Height."""

    @height.setter
    def height(self, value: Val | float) -> None: ...

    @property
    def right(self) -> Val:
        """Right position (for absolute positioning)."""

    @right.setter
    def right(self, value: Val | float) -> None: ...

    @property
    def bottom(self) -> Val:
        """Bottom position (for absolute positioning)."""

    @bottom.setter
    def bottom(self, value: Val | float) -> None: ...

    @property
    def min_width(self) -> Val:
        """Minimum width."""

    @min_width.setter
    def min_width(self, value: Val | float) -> None: ...

    @property
    def max_width(self) -> Val:
        """Maximum width."""

    @max_width.setter
    def max_width(self, value: Val | float) -> None: ...

    @property
    def min_height(self) -> Val:
        """Minimum height."""

    @min_height.setter
    def min_height(self, value: Val | float) -> None: ...

    @property
    def max_height(self) -> Val:
        """Maximum height."""

    @max_height.setter
    def max_height(self, value: Val | float) -> None: ...

    @property
    def flex_direction(self) -> FlexDirection:
        """Flex direction for layout."""

    @flex_direction.setter
    def flex_direction(self, value: FlexDirection) -> None: ...

    @property
    def direction(self) -> InlineDirection:
        """The inline (text/reading) direction for layout."""

    @direction.setter
    def direction(self, value: InlineDirection) -> None: ...

    @property
    def display(self) -> Display:
        """Display mode."""

    @display.setter
    def display(self, value: Display) -> None: ...

    @property
    def align_items(self) -> AlignItems:
        """Alignment of children along cross axis."""

    @align_items.setter
    def align_items(self, value: AlignItems) -> None: ...

    @property
    def justify_content(self) -> JustifyContent:
        """Distribution of children along main axis."""

    @justify_content.setter
    def justify_content(self, value: JustifyContent) -> None: ...

    @property
    def align_self(self) -> AlignSelf:
        """Override parent's align_items for this node."""

    @align_self.setter
    def align_self(self, value: AlignSelf) -> None: ...

    @property
    def flex_wrap(self) -> FlexWrap:
        """Whether children wrap to multiple lines."""

    @flex_wrap.setter
    def flex_wrap(self, value: FlexWrap) -> None: ...

    @property
    def align_content(self) -> AlignContent:
        """Alignment of lines within a flex container (when wrapping)."""

    @align_content.setter
    def align_content(self, value: AlignContent) -> None: ...

    @property
    def justify_items(self) -> JustifyItems:
        """Default inline axis alignment for grid items."""

    @justify_items.setter
    def justify_items(self, value: JustifyItems) -> None: ...

    @property
    def justify_self(self) -> JustifySelf:
        """Override parent's justify_items for this item."""

    @justify_self.setter
    def justify_self(self, value: JustifySelf) -> None: ...

    @property
    def overflow(self) -> Overflow:
        """How overflow is handled on each axis."""

    @overflow.setter
    def overflow(self, value: Overflow) -> None: ...

    @property
    def box_sizing(self) -> BoxSizing:
        """How width and height are calculated (border-box or content-box)."""

    @box_sizing.setter
    def box_sizing(self, value: BoxSizing) -> None: ...

    @property
    def overflow_clip_margin(self) -> OverflowClipMargin:
        """Margin for overflow clipping."""

    @overflow_clip_margin.setter
    def overflow_clip_margin(self, value: OverflowClipMargin) -> None: ...

    @property
    def border_radius(self) -> BorderRadius:
        """Border radius for rounded corners."""

    @border_radius.setter
    def border_radius(self, value: BorderRadius) -> None: ...

    @property
    def grid_auto_flow(self) -> GridAutoFlow:
        """How auto-placed items are inserted into the grid."""

    @grid_auto_flow.setter
    def grid_auto_flow(self, value: GridAutoFlow) -> None: ...

    @property
    def margin(self) -> UiRect:
        """Space around the node outside its border."""

    @margin.setter
    def margin(self, value: UiRect) -> None: ...

    @property
    def padding(self) -> UiRect:
        """Space between the node's border and its contents."""

    @padding.setter
    def padding(self, value: UiRect) -> None: ...

    @property
    def border(self) -> UiRect:
        """Width of the node's border on each side."""

    @border.setter
    def border(self, value: UiRect) -> None: ...

    @property
    def flex_grow(self) -> float:
        """How much this item should grow to fill available space (default: 0.0)."""

    @flex_grow.setter
    def flex_grow(self, value: float) -> None: ...

    @property
    def flex_shrink(self) -> float:
        """How much this item should shrink if there's not enough space (default: 1.0)."""

    @flex_shrink.setter
    def flex_shrink(self, value: float) -> None: ...

    @property
    def flex_basis(self) -> Val:
        """Initial size of item before growing/shrinking."""

    @flex_basis.setter
    def flex_basis(self, value: Val | float) -> None: ...

    @property
    def row_gap(self) -> Val:
        """Gap between rows in flex/grid layout."""

    @row_gap.setter
    def row_gap(self, value: Val | float) -> None: ...

    @property
    def column_gap(self) -> Val:
        """Gap between columns in flex/grid layout."""

    @column_gap.setter
    def column_gap(self, value: Val | float) -> None: ...

    @property
    def aspect_ratio(self) -> float | None:
        """Aspect ratio constraint (width / height)."""

    @aspect_ratio.setter
    def aspect_ratio(self, value: float | None) -> None: ...

    @property
    def scrollbar_width(self) -> float:
        """Space reserved for scrollbars in pixels."""

    @scrollbar_width.setter
    def scrollbar_width(self, value: float) -> None: ...

    # Grid layout properties

    @property
    def grid_column(self) -> GridPlacement:
        """Column placement for this grid item."""

    @grid_column.setter
    def grid_column(self, value: GridPlacement) -> None: ...

    @property
    def grid_row(self) -> GridPlacement:
        """Row placement for this grid item."""

    @grid_row.setter
    def grid_row(self, value: GridPlacement) -> None: ...

    @property
    def grid_template_columns(self) -> list[RepeatedGridTrack]:
        """Defines the columns of a grid layout."""

    @grid_template_columns.setter
    def grid_template_columns(self, value: list[RepeatedGridTrack]) -> None: ...

    @property
    def grid_template_rows(self) -> list[RepeatedGridTrack]:
        """Defines the rows of a grid layout."""

    @grid_template_rows.setter
    def grid_template_rows(self, value: list[RepeatedGridTrack]) -> None: ...

    @property
    def grid_auto_columns(self) -> list[GridTrack]:
        """Size of implicitly created grid columns."""

    @grid_auto_columns.setter
    def grid_auto_columns(self, value: list[GridTrack]) -> None: ...

    @property
    def grid_auto_rows(self) -> list[GridTrack]:
        """Size of implicitly created grid rows."""

    @grid_auto_rows.setter
    def grid_auto_rows(self, value: list[GridTrack]) -> None: ...


class BackgroundColor(Component):
    """Background color component for UI nodes.

    Sets the background color of a UI element. Essential for creating visible
    UI panels, buttons, and other styled elements.

    Example:
        ```python
        from pybevy.ui import Node, BackgroundColor
        from pybevy.color import Color

        # Create a red panel
        commands.spawn((
            Node(),
            BackgroundColor(Color.srgb(1.0, 0.0, 0.0)),
        ))

        # Change color dynamically
        def update_color(query: Query[Mut[BackgroundColor]]):
            for bg in query:
                bg.color = Color.srgb(0.0, 0.0, 1.0)
        ```

    Args:
        color: The background color (optional, defaults to transparent)
    """
    def __init__(self, color: Color | None = None) -> None: ...

    @property
    def color(self) -> Color:
        """The background color."""

    @color.setter
    def color(self, value: Color) -> None: ...

    def __eq__(self, other: object) -> bool: ...


class BorderColor(Component):
    """Border color component for UI nodes.

    Sets the color of a UI element's border. Supports different colors for each side
    (top, right, bottom, left), similar to CSS.

    Example:
        ```python
        from pybevy.ui import Node, BorderColor
        from pybevy.color import Color

        # Create node with same border color on all sides
        commands.spawn((
            Node(),
            BorderColor(Color.srgb(0.0, 1.0, 0.0)),  # Green border all sides
        ))

        # Different colors per side
        def update_border(query: Query[Mut[BorderColor]]):
            for border in query:
                border.top = Color.srgb(1.0, 0.0, 0.0)  # Red top
                border.right = Color.srgb(0.0, 1.0, 0.0)  # Green right
                border.bottom = Color.srgb(0.0, 0.0, 1.0)  # Blue bottom
                border.left = Color.srgb(1.0, 1.0, 0.0)  # Yellow left

                # Or set all at once
                border.set_all(Color.srgb(1.0, 0.0, 0.0))  # Red all sides
        ```

    Args:
        color: Base color for all border sides (optional, defaults to transparent)
        top: Top border color (overrides base color)
        right: Right border color (overrides base color)
        bottom: Bottom border color (overrides base color)
        left: Left border color (overrides base color)
    """
    def __init__(
        self,
        color: Color | None = None,
        *,
        top: Color | None = None,
        right: Color | None = None,
        bottom: Color | None = None,
        left: Color | None = None,
    ) -> None: ...

    @staticmethod
    def all(color: Color) -> BorderColor:
        """Create BorderColor with same color for all sides."""

    @property
    def top(self) -> Color:
        """Top border color."""

    @top.setter
    def top(self, value: Color) -> None: ...

    @property
    def right(self) -> Color:
        """Right border color."""

    @right.setter
    def right(self, value: Color) -> None: ...

    @property
    def bottom(self) -> Color:
        """Bottom border color."""

    @bottom.setter
    def bottom(self, value: Color) -> None: ...

    @property
    def left(self) -> Color:
        """Left border color."""

    @left.setter
    def left(self, value: Color) -> None: ...

    def set_all(self, color: Color) -> None:
        """Set all border sides to the same color.

        Args:
            color: Color to apply to all sides
        """

    def is_fully_transparent(self) -> bool:
        """Check if all border colors are fully transparent."""


class BorderRadius:
    """Border radius for rounded corners on UI nodes.

    BorderRadius is a frozen data class (not a Component). It is used to set
    the radius of each corner independently. All corners use Val sizing,
    supporting pixels, percentages, auto-sizing, and viewport-relative units.

    In Bevy 0.18, BorderRadius is no longer a Component - it cannot be spawned
    or queried from the ECS directly.

    Example:
        ```python
        from pybevy.ui import BorderRadius, Val

        # Same radius for all corners (10px)
        br = BorderRadius.px(10.0, 10.0, 10.0, 10.0)

        # Using all() for uniform radius
        br = BorderRadius.all(Val.percent(50.0))

        # Per-corner via constructor kwargs
        br = BorderRadius(Val.px(5.0), top_left=Val.px(20.0))

        # Access corner values (read-only properties)
        print(br.top_left)   # Val.px(20.0)
        print(br.top_right)  # Val.px(5.0)
        ```

    Args:
        radius: Val for all corners (default radius)
        top_left: Override for top-left corner
        top_right: Override for top-right corner
        bottom_left: Override for bottom-left corner
        bottom_right: Override for bottom-right corner

    Note:
        Use the constructor with keyword args for per-corner construction:
        ``BorderRadius(top_left=Val.px(10.0))``.
    """

    ZERO: BorderRadius
    """Zero curvature. All corners will be right-angled."""

    MAX: BorderRadius
    """Maximum curvature. Creates capsule/circular shape."""

    def __init__(
        self,
        radius: Val = ...,
        *,
        top_left: Val | None = None,
        top_right: Val | None = None,
        bottom_left: Val | None = None,
        bottom_right: Val | None = None,
    ) -> None: ...

    @staticmethod
    def new(
        top_left: Val, top_right: Val, bottom_right: Val, bottom_left: Val
    ) -> BorderRadius:
        """Create BorderRadius with different radius for each corner."""

    @staticmethod
    def all(radius: Val) -> BorderRadius:
        """Create BorderRadius with same radius for all corners.

        Args:
            radius: Val to apply to all corners

        Returns:
            BorderRadius with uniform corner radii
        """

    @staticmethod
    def px(
        top_left: float, top_right: float, bottom_right: float, bottom_left: float
    ) -> BorderRadius:
        """Create BorderRadius with pixel values for each corner."""

    @staticmethod
    def percent(
        top_left: float, top_right: float, bottom_right: float, bottom_left: float
    ) -> BorderRadius:
        """Create BorderRadius with percentage values for each corner."""

    @property
    def top_left(self) -> Val:
        """Top-left corner radius."""

    @property
    def top_right(self) -> Val:
        """Top-right corner radius."""

    @property
    def bottom_left(self) -> Val:
        """Bottom-left corner radius."""

    @property
    def bottom_right(self) -> Val:
        """Bottom-right corner radius."""

    # Builder methods
    def with_top_left(self, radius: Val) -> BorderRadius: ...
    def with_top_right(self, radius: Val) -> BorderRadius: ...
    def with_bottom_right(self, radius: Val) -> BorderRadius: ...
    def with_bottom_left(self, radius: Val) -> BorderRadius: ...
    def with_left(self, radius: Val) -> BorderRadius: ...
    def with_right(self, radius: Val) -> BorderRadius: ...
    def with_top(self, radius: Val) -> BorderRadius: ...
    def with_bottom(self, radius: Val) -> BorderRadius: ...


class Outline(Component):
    """Outline component for UI nodes.

    Draws an outline around the UI node's border. Unlike borders, outlines do not
    affect layout and are drawn outside the border box.

    Example:
        ```python
        from pybevy.ui import Node, Outline, Val
        from pybevy.color import Color

        # 2px solid red outline
        commands.spawn((
            Node(),
            Outline(Val.px(2.0), Val.px(0.0), Color.srgb(1.0, 0.0, 0.0)),
        ))

        # Outline with offset (space between border and outline)
        outline = Outline(
            Val.px(3.0),           # width
            Val.px(5.0),           # offset from border
            Color.srgb(0.0, 0.0, 1.0)  # color
        )
        commands.spawn((Node(), outline))

        # Modify at runtime
        def update_outline(query: Query[Mut[Outline]]):
            for outline in query:
                outline.width = Val.px(4.0)
                outline.color = Color.srgb(0.0, 1.0, 0.0)
        ```

    Args:
        width: Width of the outline (Val, defaults to Val.ZERO())
        offset: Offset between border and outline (Val, defaults to Val.ZERO())
        color: Outline color (optional, defaults to transparent)
    """
    def __init__(
        self,
        width: Val = ...,
        offset: Val = ...,
        color: Color | None = None,
    ) -> None: ...

    @property
    def width(self) -> Val:
        """Width of the outline."""

    @width.setter
    def width(self, value: Val) -> None: ...

    @property
    def offset(self) -> Val:
        """Offset between the border and outline."""

    @offset.setter
    def offset(self, value: Val) -> None: ...

    @property
    def color(self) -> Color:
        """Outline color."""

    @color.setter
    def color(self, value: Color) -> None: ...


class ZIndex(Component):
    """Z-index component for controlling UI element layering.

    Controls the rendering order of UI elements. Higher z-index values render
    on top of lower values.

    Example:
        ```python
        from pybevy.ui import Node, ZIndex, BackgroundColor
        from pybevy.color import Color

        # Create UI element with z-index
        commands.spawn((
            Node(),
            ZIndex(10),
            BackgroundColor(Color.srgb(1.0, 0.0, 0.0)),
        ))

        # Modify at runtime
        def update_z_index(query: Query[Mut[ZIndex]]):
            for z_index in query:
                z_index.value = 20
        ```

    Args:
        value: Integer z-index value (defaults to 0)
    """
    def __init__(self, value: int = 0) -> None: ...

    @property
    def value(self) -> int:
        """The z-index value."""

    @value.setter
    def value(self, value: int) -> None: ...

    def __eq__(self, other: object) -> bool: ...


class NodeImageMode:
    """Controls how an image fits within its UI node.

    Example:
        ```python
        from pybevy.ui import NodeImageMode, ImageNode, Node
        from pybevy.sprite import TextureSlicer, BorderRect

        # Auto sizing
        auto_mode = NodeImageMode.Auto()

        # Stretch to fill
        stretch_mode = NodeImageMode.Stretch()

        # 9-slice for UI panels
        slicer = TextureSlicer(BorderRect.all(16.0))
        sliced_mode = NodeImageMode.Sliced(slicer)

        # Tiled background
        tiled_mode = NodeImageMode.Tiled(tile_x=True, tile_y=True, stretch_value=1.0)
        ```
    """

    class Auto(NodeImageMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Stretch(NodeImageMode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Sliced(NodeImageMode):
        __match_args__: ClassVar[tuple[Literal["slicer"]]]
        slicer: TextureSlicer
        def __init__(self, slicer: TextureSlicer) -> None: ...

    class Tiled(NodeImageMode):
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
        """Returns true if this mode uses slices (Sliced or Tiled)."""


class ImageNode(Component):
    """UI image component for displaying images in UI nodes.

    Displays an image texture on a UI node. The image is stretched to fill
    the node's size (use BorderRadius for rounded corners).

    Example:
        ```python
        from pybevy.ui import Node, ImageNode, BackgroundColor
        from pybevy.assets import Assets, AssetServer
        from pybevy.image import Image
        from pybevy.ecs import Res, Commands

        def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
            # Load image and create UI node with image
            texture = asset_server.load("bevy/textures/icon.png")
            commands.spawn((
                Node(),
                ImageNode(texture),
            ))

        # Change texture at runtime
        def update_image(query: Query[Mut[ImageNode]], asset_server: Res[AssetServer]) -> None:
            for image_node in query:
                new_texture = asset_server.load("bevy/textures/new_icon.png")
                image_node.image = new_texture
        ```

    Args:
        handle: Handle to the Image asset
    """
    def __init__(self, handle: Handle[Image]) -> None: ...

    @staticmethod
    def solid_color(color: Color) -> ImageNode:
        """Create a solid color ImageNode (useful for debugging layout)."""

    @property
    def image(self) -> Handle[Image]:
        """The image texture handle."""

    @image.setter
    def image(self, value: Handle[Image]) -> None: ...

    @property
    def color(self) -> Color:
        """Tint color multiplied with the image (default: WHITE)."""

    @color.setter
    def color(self, value: Color) -> None: ...

    @property
    def flip_x(self) -> bool:
        """Whether to flip the image along its x-axis."""

    @flip_x.setter
    def flip_x(self, value: bool) -> None: ...

    @property
    def flip_y(self) -> bool:
        """Whether to flip the image along its y-axis."""

    @flip_y.setter
    def flip_y(self, value: bool) -> None: ...

    @property
    def rect(self) -> Rect | None:
        """Optional region of the image to render."""

    @rect.setter
    def rect(self, value: Rect | None) -> None: ...

    @property
    def image_mode(self) -> NodeImageMode:
        """How the image fits within the node."""

    @image_mode.setter
    def image_mode(self, value: NodeImageMode) -> None: ...

    @property
    def visual_box(self) -> VisualBox:
        """Which box (content/padding/border) the image is clipped/laid out against."""

    @visual_box.setter
    def visual_box(self, value: VisualBox) -> None: ...


class FocusPolicy(Component):
    """Focus policy component for UI input handling.

    Controls whether a UI node blocks or passes input focus events to nodes behind it.
    Essential for creating interactive UI hierarchies where some elements should capture
    all input (like buttons) while others allow input to pass through (like panels).

    Example:
        ```python
        from pybevy.ui import Node, FocusPolicy, Button, Interaction

        # Button that blocks input (default for interactive elements)
        commands.spawn((
            Node(),
            Button(),
            Interaction.None_,
            FocusPolicy.Block,  # Blocks input, doesn't pass to nodes behind
        ))

        # Transparent panel that passes input through
        commands.spawn((
            Node(),
            FocusPolicy.Pass,  # Allows input to pass to nodes behind
        ))

        # Change policy at runtime
        def update_focus(query: Query[Mut[FocusPolicy]]) -> None:
            for policy in query:
                if policy.is_block:
                    policy.set_pass()  # Switch to pass mode
        ```
    """
    def __init__(self) -> None: ...

    Block: ClassVar[FocusPolicy]
    """Blocking focus policy that captures input."""

    Pass: ClassVar[FocusPolicy]
    """Passing focus policy that allows input through."""

    @property
    def is_block(self) -> bool:
        """True if this policy blocks input."""

    @property
    def is_pass(self) -> bool:
        """True if this policy passes input through."""

    def set_block(self) -> None:
        """Change to blocking mode."""

    def set_pass(self) -> None:
        """Change to passing mode."""


class Interaction(Component):
    """Interaction state for UI elements.

    Describes whether a UI element is being interacted with by the user.
    Updated automatically by Bevy's UI interaction system when used with
    clickable UI elements.

    Example:
        ```python
        from pybevy.ui import Node, Interaction, BackgroundColor
        from pybevy.ecs import Query, Mut
        from pybevy.color import Color

        # Create interactive button
        commands.spawn((
            Node(),
            Interaction.None_,
            BackgroundColor(Color.srgb(0.5, 0.5, 0.5)),
        ))

        # Respond to interactions
        def button_system(query: Query[tuple[Interaction, Mut[BackgroundColor]]]):
            for interaction, bg in query:
                if interaction.is_pressed:
                    bg.color = Color.srgb(0.0, 1.0, 0.0)
                elif interaction.is_hovered:
                    bg.color = Color.srgb(0.8, 0.8, 0.8)
                else:
                    bg.color = Color.srgb(0.5, 0.5, 0.5)
        ```

    The interaction state is automatically updated by Bevy when the user
    hovers over or clicks on UI nodes.
    """
    def __init__(self) -> None:
        """Create Interaction in None state."""

    None_: ClassVar[Interaction]
    """No pointer interaction."""

    Hovered: ClassVar[Interaction]
    """Pointer is hovering over the node."""

    Pressed: ClassVar[Interaction]
    """Pointer is pressing the node."""

    @property
    def is_none(self) -> bool:
        """True if the UI element is not being interacted with."""

    @property
    def is_hovered(self) -> bool:
        """True if the UI element is being hovered."""

    @property
    def is_pressed(self) -> bool:
        """True if the UI element is being pressed."""

    @property
    def state(self) -> str:
        """Get the interaction state as a string: 'none', 'hovered', or 'pressed'."""


class Button(Component):
    """Marker component for interactive buttons.

    Button is a simple marker component that indicates an entity is an interactive
    button. It's typically used together with Node, BackgroundColor, and Interaction
    to create clickable UI elements.

    Example:
        ```python
        from pybevy.ui import Node, Button, Interaction, BackgroundColor
        from pybevy.ecs import Query, Mut
        from pybevy.color import Color

        # Create a button
        commands.spawn((
            Node(),
            Button(),
            Interaction.None_,
            BackgroundColor(Color.srgb(0.2, 0.2, 0.2)),
        ))

        # React to button clicks
        def button_system(query: Query[tuple[Button, Interaction, Mut[BackgroundColor]]]):
            for _button, interaction, bg in query:
                if interaction.is_pressed:
                    bg.color = Color.srgb(0.1, 0.5, 0.1)
                elif interaction.is_hovered:
                    bg.color = Color.srgb(0.3, 0.3, 0.3)
                else:
                    bg.color = Color.srgb(0.2, 0.2, 0.2)
        ```
    """
    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...


class Label(Component):
    """Marker component for accessibility labels.

    Label is a marker component used to identify text elements that serve as
    labels for accessibility purposes. It helps screen readers and other
    assistive technologies understand the UI structure.

    Example:
        ```python
        from pybevy.ui import Node, Text, Label

        # Create a text label
        commands.spawn((
            Node(),
            Text("Username:"),
            Label(),  # Marks this as an accessibility label
        ))
        ```
    """
    def __init__(self) -> None: ...


class InteractionDisabled(Component):
    """Marker component indicating a widget is disabled.

    Used to prevent user interaction with a UI element while keeping it
    visible. The widget should appear "grayed out" when this component
    is present.

    Example:
        ```python
        from pybevy.ui import Node, Button, InteractionDisabled

        # Create a disabled button
        commands.spawn((Node(), Button(), InteractionDisabled()))
        ```
    """
    def __init__(self) -> None: ...


class Pressed(Component):
    """Marker component indicating a button is currently pressed.

    Tracks whether a button or widget is in a pressed/"held down" state.

    Example:
        ```python
        from pybevy.ui import Node, Button, Pressed
        from pybevy.ecs import Query, With

        # Query for pressed buttons
        def check_pressed(query: Query[Button, With[Pressed]]) -> None:
            for _button in query:
                print("Button is pressed!")
        ```
    """
    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...


class Checked(Component):
    """Marker component indicating a checkbox/radio button is checked.

    Used to track the checked state of toggle-able UI elements.

    Example:
        ```python
        from pybevy.ui import Node, Checked
        from pybevy.ecs import Commands

        # Create a checked checkbox
        commands.spawn((Node(), Checked()))
        ```
    """
    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...


class JustifyItems:
    """Justify items enum for grid layout.

    Controls how items are aligned within their grid cells along the inline axis.

    Example:
        ```python
        from pybevy.ui import Node, JustifyItems

        node = Node()
        node.justify_items = JustifyItems.Center
        ```
    """

    Default: JustifyItems
    """Default alignment."""

    Start: JustifyItems
    """Align to start."""

    End: JustifyItems
    """Align to end."""

    Center: JustifyItems
    """Align to center."""

    Baseline: JustifyItems
    """Align to baseline."""

    Stretch: JustifyItems
    """Stretch to fill."""


class JustifySelf:
    """Justify self enum for individual item alignment in grid.

    Overrides parent's justify_items for a specific item.

    Example:
        ```python
        from pybevy.ui import Node, JustifySelf

        node = Node()
        node.justify_self = JustifySelf.Center
        ```
    """

    Auto: JustifySelf
    """Use parent's justify_items (default)."""

    Start: JustifySelf
    """Align to start."""

    End: JustifySelf
    """Align to end."""

    Center: JustifySelf
    """Align to center."""

    Baseline: JustifySelf
    """Align to baseline."""

    Stretch: JustifySelf
    """Stretch to fill."""


class GlobalZIndex(Component):
    """Global z-index for cross-tree UI layering.

    Unlike ZIndex which is relative to siblings, GlobalZIndex controls
    rendering order across the entire UI tree.

    Example:
        ```python
        from pybevy.ui import Node, GlobalZIndex

        # Render on top of everything
        commands.spawn((Node(), GlobalZIndex(100)))

        # Render below default (0)
        commands.spawn((Node(), GlobalZIndex(-10)))
        ```

    Args:
        value: Integer z-index value (higher = on top, defaults to 0)
    """
    def __init__(self, value: int = 0) -> None: ...

    @property
    def value(self) -> int:
        """The global z-index value."""

    @value.setter
    def value(self, value: int) -> None: ...

    def __eq__(self, other: object) -> bool: ...


class GridAutoFlow:
    """Grid auto-placement algorithm for CSS Grid layout.

    Controls how auto-placed items are inserted into the grid.

    Example:
        ```python
        from pybevy.ui import Node, Display, GridAutoFlow

        node = Node()
        node.display = Display.Grid
        node.grid_auto_flow = GridAutoFlow.Row  # Fill rows first
        ```
    """

    Row: GridAutoFlow
    """Place items by filling each row in turn."""

    Column: GridAutoFlow
    """Place items by filling each column in turn."""

    RowDense: GridAutoFlow
    """Fill rows first, using dense packing (fills gaps)."""

    ColumnDense: GridAutoFlow
    """Fill columns first, using dense packing (fills gaps)."""


class GridTrack:
    """Size specification for a single grid track (row or column).

    Used to define the size of individual grid tracks or as part
    of RepeatedGridTrack definitions.

    Example:
        ```python
        from pybevy.ui import GridTrack

        # Fixed pixel size
        track = GridTrack.px(100.0)

        # Fractional (flexible) size
        track = GridTrack.fr(1.0)

        # Auto-sized based on content
        track = GridTrack.auto()
        ```
    """

    @staticmethod
    def px(value: float) -> GridTrack:
        """Create a track with fixed pixel size."""

    @staticmethod
    def percent(value: float) -> GridTrack:
        """Create a track with percentage size."""

    @staticmethod
    def fr(value: float) -> GridTrack:
        """Create a flexible track with fractional unit size."""

    @staticmethod
    def flex(value: float) -> GridTrack:
        """Create a flexible track with minmax(0, Nfr) size."""

    @staticmethod
    def auto() -> GridTrack:
        """Create an auto-sized track."""

    @staticmethod
    def min_content() -> GridTrack:
        """Create a track sized to min-content."""

    @staticmethod
    def max_content() -> GridTrack:
        """Create a track sized to max-content."""

    @staticmethod
    def fit_content_px(limit: float) -> GridTrack:
        """Create a fit-content track with pixel limit."""

    @staticmethod
    def fit_content_percent(limit: float) -> GridTrack:
        """Create a fit-content track with percentage limit."""

    @staticmethod
    def vw(value: float) -> GridTrack:
        """Create a track sized to viewport width percentage."""

    @staticmethod
    def vh(value: float) -> GridTrack:
        """Create a track sized to viewport height percentage."""

    @staticmethod
    def vmin(value: float) -> GridTrack:
        """Create a track sized to viewport minimum dimension percentage."""

    @staticmethod
    def vmax(value: float) -> GridTrack:
        """Create a track sized to viewport maximum dimension percentage."""

    def __eq__(self, other: object) -> bool: ...


class GridPlacement:
    """Grid item placement specification.

    Specifies where a grid item is placed and how many tracks it spans.
    Lines are 1-indexed, negative values count from the end.

    Example:
        ```python
        from pybevy.ui import GridPlacement

        # Place at line 2, span 1 (default)
        placement = GridPlacement.start(2)

        # Span 3 tracks (auto placement)
        placement = GridPlacement.span(3)

        # Explicit start and end
        placement = GridPlacement.start_end(1, 4)
        ```
    """

    @staticmethod
    def auto() -> GridPlacement:
        """Automatic placement (span defaults to 1)."""

    @staticmethod
    def span(span: int) -> GridPlacement:
        """Auto-place but span multiple tracks."""

    @staticmethod
    def start(start: int) -> GridPlacement:
        """Start at a specific grid line (1-indexed)."""

    @staticmethod
    def end(end: int) -> GridPlacement:
        """End at a specific grid line (1-indexed)."""

    @staticmethod
    def start_span(start: int, span: int) -> GridPlacement:
        """Start at a line and span multiple tracks."""

    @staticmethod
    def start_end(start: int, end: int) -> GridPlacement:
        """Specify both start and end lines."""

    @staticmethod
    def end_span(end: int, span: int) -> GridPlacement:
        """End at a line and span backwards."""

    def get_start(self) -> int | None:
        """Get the start line (None if auto)."""

    def get_span(self) -> int | None:
        """Get the span (None if auto, usually 1)."""

    def get_end(self) -> int | None:
        """Get the end line (None if auto)."""

    def set_start(self, start: int) -> GridPlacement:
        """Return a new placement with the start line set."""

    def set_end(self, end: int) -> GridPlacement:
        """Return a new placement with the end line set."""

    def set_span(self, span: int) -> GridPlacement:
        """Return a new placement with the span set."""


class RepeatedGridTrack:
    """Repeated grid track definition for CSS Grid templates.

    Used to define multiple repeated tracks in grid_template_rows
    or grid_template_columns.

    Example:
        ```python
        from pybevy.ui import Node, Display, RepeatedGridTrack

        node = Node()
        node.display = Display.Grid
        # Create 3 columns of 100px each
        node.grid_template_columns = [RepeatedGridTrack.px(3, 100.0)]
        # Create 4 flexible rows
        node.grid_template_rows = [RepeatedGridTrack.fr(4, 1.0)]
        ```
    """

    @staticmethod
    def px(repetition: int, value: float) -> RepeatedGridTrack:
        """Create repeated tracks with fixed pixel size."""

    @staticmethod
    def percent(repetition: int, value: float) -> RepeatedGridTrack:
        """Create repeated tracks with percentage size."""

    @staticmethod
    def fr(repetition: int, value: float) -> RepeatedGridTrack:
        """Create repeated flexible tracks with fractional size."""

    @staticmethod
    def flex(repetition: int, value: float) -> RepeatedGridTrack:
        """Create repeated flexible tracks with minmax(0, Nfr)."""

    @staticmethod
    def auto(repetition: int) -> RepeatedGridTrack:
        """Create repeated auto-sized tracks."""

    @staticmethod
    def min_content(repetition: int) -> RepeatedGridTrack:
        """Create repeated tracks sized to min-content."""

    @staticmethod
    def max_content(repetition: int) -> RepeatedGridTrack:
        """Create repeated tracks sized to max-content."""

    @staticmethod
    def fit_content_px(repetition: int, limit: float) -> RepeatedGridTrack:
        """Create repeated fit-content tracks with pixel limit."""

    @staticmethod
    def fit_content_percent(repetition: int, limit: float) -> RepeatedGridTrack:
        """Create repeated fit-content tracks with percentage limit."""

    def __eq__(self, other: object) -> bool: ...


class IsDefaultUiCamera(Component):
    """Marker component for the default UI camera.

    When multiple cameras exist, this component identifies which camera
    should render the UI by default.

    Example:
        ```python
        from pybevy.ui import IsDefaultUiCamera
        from pybevy.camera import Camera2d

        # Mark a camera as the default UI camera
        commands.spawn((Camera2d(), IsDefaultUiCamera()))
        ```
    """
    def __init__(self) -> None: ...


class UiTargetCamera(Component):
    """Component that specifies which camera should render a UI node.

    UI nodes with this component will be rendered by the specified camera.
    Only effective on root UI nodes.

    Example:
        ```python
        from pybevy.ui import Node, UiTargetCamera
        from pybevy.ecs import Entity

        def setup(commands: Commands, camera_entity: Entity) -> None:
            # Create UI that renders to a specific camera
            commands.spawn((
                Node(),
                UiTargetCamera(camera_entity),
            ))
        ```
    """
    def __init__(self, entity: Entity) -> None: ...

    @property
    def entity(self) -> Entity:
        """The camera entity this UI targets."""

    def __eq__(self, other: object) -> bool: ...


class RelativeCursorPosition(Component):
    """Component that tracks cursor position relative to a UI node.

    When added to a UI node, this component is automatically updated
    with the cursor's position relative to the node bounds.

    Example:
        ```python
        from pybevy.ui import Node, RelativeCursorPosition, Interaction

        def setup(commands: Commands) -> None:
            commands.spawn((
                Node(),
                Interaction.None_,
                RelativeCursorPosition(),
            ))

        def check_cursor(query: Query[RelativeCursorPosition]) -> None:
            for pos in query:
                if pos.cursor_over:
                    if pos.normalized:
                        print(f"Cursor at normalized: {pos.normalized}")
        ```

    Attributes:
        cursor_over: True if cursor is over an unclipped area of the node.
        normalized: Cursor position in normalized coordinates (0-1), or None if unknown.
    """
    def __init__(self) -> None: ...

    @property
    def cursor_over(self) -> bool:
        """True if the cursor is over an unclipped area of this node."""

    @cursor_over.setter
    def cursor_over(self, value: bool) -> None: ...

    @property
    def normalized(self) -> Vec2 | None:
        """Cursor position normalized to node bounds (0-1), or None if unknown."""

    @normalized.setter
    def normalized(self, value: Vec2 | None) -> None: ...

    def is_cursor_over(self) -> bool:
        """Helper function to check if cursor is over the node."""

    def __eq__(self, other: object) -> bool: ...


class InterpolationColorSpace:
    """Color space enum for gradient interpolation.

    Defines which color space to use when interpolating between colors
    in UI gradients.

    Example:
        ```python
        from pybevy.ui import InterpolationColorSpace

        # Linear RGB interpolation (most common)
        space = InterpolationColorSpace.LinearRgba

        # Perceptually uniform interpolation
        space = InterpolationColorSpace.Oklaba
        ```
    """

    Oklaba: InterpolationColorSpace
    """Oklaba color space (perceptually uniform)."""

    Oklcha: InterpolationColorSpace
    """Oklcha color space (perceptually uniform, cylindrical)."""

    OklchaLong: InterpolationColorSpace
    """Oklcha with long path hue interpolation."""

    Srgba: InterpolationColorSpace
    """Standard RGB color space."""

    LinearRgba: InterpolationColorSpace
    """Linear RGB color space."""

    Hsla: InterpolationColorSpace
    """HSL color space."""

    HslaLong: InterpolationColorSpace
    """HSL with long path hue interpolation."""

    Hsva: InterpolationColorSpace
    """HSV color space."""

    HsvaLong: InterpolationColorSpace
    """HSV with long path hue interpolation."""


class ScrollPosition(Component):
    """Scroll position component for scrollable UI nodes.

    Tracks the current scroll offset of a scrollable container.
    Used with Overflow.scroll() to create scrollable UI regions.

    Example:
        ```python
        from pybevy.ui import Node, ScrollPosition, Overflow
        from pybevy.ecs import Query, Mut

        # Create a scrollable container
        commands.spawn((
            Node(),
            Overflow.scroll(),
            ScrollPosition(0.0, 0.0),
        ))

        # Read/modify scroll position
        def scroll_system(query: Query[Mut[ScrollPosition]]) -> None:
            for scroll in query:
                scroll.y += 10.0  # Scroll down
        ```

    Args:
        x: Horizontal scroll offset (default: 0.0)
        y: Vertical scroll offset (default: 0.0)
    """
    def __init__(self, x: float = 0.0, y: float = 0.0) -> None: ...

    @property
    def x(self) -> float:
        """Horizontal scroll offset."""

    @x.setter
    def x(self, value: float) -> None: ...

    @property
    def y(self) -> float:
        """Vertical scroll offset."""

    @y.setter
    def y(self, value: float) -> None: ...

    @property
    def offset(self) -> Vec2:
        """Scroll offset as a Vec2."""

    @offset.setter
    def offset(self, value: Vec2) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *, x: np.typing.ArrayLike | None = None, y: np.typing.ArrayLike | None = None
    ) -> Batchable: ...


class ComputedStackIndex(Component):
    """The draw order of a UI node, computed by the UI stacking system.

    Nodes with a higher stack index are drawn on top of and receive
    interactions before nodes with lower stack indices. Automatically added
    and updated by the UI system - you cannot create it manually.
    """

    @property
    def value(self) -> int:
        """The node's position in the UI stack (higher is drawn on top)."""

class ComputedNode(Component):
    """Read-only computed layout information for a UI node.

    Contains the final computed sizes and layout information after Bevy's
    UI layout system has processed the node. This component is automatically
    added and updated by the UI system - you cannot create it manually.

    Useful for reading final pixel sizes after layout, debugging layout issues,
    or implementing custom rendering based on actual node dimensions.

    Example:
        ```python
        from pybevy.ui import Node, ComputedNode
        from pybevy.ecs import Query

        def debug_layout(query: Query[tuple[Node, ComputedNode]]) -> None:
            for node, computed in query:
                print(f"Node size: {computed.size}")
        ```
    """

    @property
    def size(self) -> Vec2:
        """The final computed size of the node in physical pixels."""

    @property
    def content_size(self) -> Vec2:
        """The size of the node's content area (excluding padding)."""

    @property
    def unrounded_size(self) -> Vec2:
        """The size before rounding to physical pixels."""

    @property
    def outline_width(self) -> float:
        """The computed outline width in physical pixels."""

    @property
    def outline_offset(self) -> float:
        """The computed outline offset in physical pixels."""

    @property
    def outlined_node_size(self) -> Vec2:
        """The total size including the outline."""

    @property
    def inverse_scale_factor(self) -> float:
        """Inverse of the UI scale factor."""

    @property
    def scrollbar_size(self) -> Vec2:
        """Size reserved for scrollbars in physical pixels."""

    @property
    def scroll_position(self) -> Vec2:
        """Current computed scroll position in physical pixels."""

    def is_empty(self) -> bool:
        """Check if the node has zero size."""


class UiScale(Resource):
    """Global scale factor for UI elements.

    Multiplies the logical size of all UI elements by this factor.
    Useful for implementing accessibility zoom features or supporting
    different display densities.

    Example:
        ```python
        from pybevy.ui import UiScale
        from pybevy.ecs import ResMut

        def setup(commands: Commands) -> None:
            # Make UI 1.5x larger
            commands.insert_resource(UiScale(1.5))

        def update_scale(ui_scale: ResMut[UiScale]) -> None:
            ui_scale.scale = 2.0  # Double size
        ```

    Args:
        scale: Scale factor (default: 1.0)
    """
    def __init__(self, scale: float = 1.0) -> None: ...

    @property
    def scale(self) -> float:
        """The global UI scale factor."""

    @scale.setter
    def scale(self, value: float) -> None: ...

    def __float__(self) -> float:
        """Float conversion returns the scale value."""


class ColorStop:
    """A color stop for gradients.

    Defines a color at a specific position along a gradient.

    Example:
        ```python
        from pybevy.ui import ColorStop, LinearGradient
        from pybevy.color import Color

        # Create color stops
        stop1 = ColorStop.auto(Color.RED)
        stop2 = ColorStop.percent(Color.BLUE, 50.0)
        stop3 = ColorStop.px(Color.GREEN, 100.0)
        ```
    """

    def __init__(
        self,
        color: Color | None = None,
        point: Val = ...,
        *,
        hint: float = 0.5,
    ) -> None:
        """Create a color stop with explicit position.

        Args:
            color: The color at this stop
            point: Position along the gradient
            hint: Interpolation midpoint hint (0.0 to 1.0)
        """

    @staticmethod
    def auto(color: Color) -> ColorStop:
        """Create an automatic color stop (position interpolated evenly)."""

    @staticmethod
    def px(color: Color, px: float) -> ColorStop:
        """Create a color stop at a pixel position."""

    @staticmethod
    def percent(color: Color, percent: float) -> ColorStop:
        """Create a color stop at a percentage position."""

    def with_hint(self, hint: float) -> ColorStop:
        """Set the interpolation midpoint hint (0.0 to 1.0)."""

    @property
    def color(self) -> Color:
        """The color at this stop."""

    @property
    def point(self) -> Val:
        """The position along the gradient."""

    @property
    def hint(self) -> float:
        """The interpolation midpoint hint."""

    def __eq__(self, other: object) -> bool: ...


class AngularColorStop:
    """A color stop for conic gradients.

    Similar to ColorStop but uses angles instead of linear positions.
    """

    def __init__(
        self,
        color: Color | None = None,
        angle: float | None = None,
        *,
        hint: float = 0.5,
    ) -> None:
        """Create an angular color stop at a specific angle (radians).

        Args:
            color: The color at this stop
            angle: Angle in radians (None for auto positioning)
            hint: Interpolation midpoint hint (0.0 to 1.0)
        """

    @staticmethod
    def auto(color: Color) -> AngularColorStop:
        """Create an automatic angular color stop."""

    def with_hint(self, hint: float) -> AngularColorStop:
        """Set the interpolation midpoint hint."""

    @property
    def color(self) -> Color:
        """The color at this stop."""

    @property
    def angle(self) -> float | None:
        """The angle in radians (None for auto stops)."""

    @property
    def hint(self) -> float:
        """The interpolation midpoint hint."""

    def __eq__(self, other: object) -> bool: ...


class UiPosition:
    """Responsive position relative to a UI node.

    Used for positioning gradients and other UI elements.

    Example:
        ```python
        from pybevy.ui import UiPosition

        pos = UiPosition.center()
        pos = UiPosition.top_left()
        pos = UiPosition.center().at_px(10.0, 20.0)
        ```
    """

    def __init__(self, anchor: Vec2, x: Val, y: Val) -> None:
        """Create a position with explicit anchor and offsets."""

    @staticmethod
    def anchor(anchor: Vec2) -> UiPosition:
        """Create a position from an anchor point."""

    @staticmethod
    def center(x: Val | None = None, y: Val | None = None) -> UiPosition:
        """Center position, with optional offsets."""

    @staticmethod
    def top(x: Val | None = None, y: Val | None = None) -> UiPosition:
        """Top center position, with optional offsets."""

    @staticmethod
    def bottom(x: Val | None = None, y: Val | None = None) -> UiPosition:
        """Bottom center position, with optional offsets."""

    @staticmethod
    def left(x: Val | None = None, y: Val | None = None) -> UiPosition:
        """Left center position, with optional offsets."""

    @staticmethod
    def right(x: Val | None = None, y: Val | None = None) -> UiPosition:
        """Right center position, with optional offsets."""

    @staticmethod
    def top_left(x: Val | None = None, y: Val | None = None) -> UiPosition:
        """Top-left corner position, with optional offsets."""

    @staticmethod
    def top_right(x: Val | None = None, y: Val | None = None) -> UiPosition:
        """Top-right corner position, with optional offsets."""

    @staticmethod
    def bottom_left(x: Val | None = None, y: Val | None = None) -> UiPosition:
        """Bottom-left corner position, with optional offsets."""

    @staticmethod
    def bottom_right(x: Val | None = None, y: Val | None = None) -> UiPosition:
        """Bottom-right corner position, with optional offsets."""

    def at(self, x: Val, y: Val) -> UiPosition:
        """Set offsets from anchor."""

    def at_x(self, x: Val) -> UiPosition:
        """Set horizontal offset from anchor."""

    def at_y(self, y: Val) -> UiPosition:
        """Set vertical offset from anchor."""

    def at_px(self, x: float, y: float) -> UiPosition:
        """Set pixel offsets from anchor."""

    def at_percent(self, x: float, y: float) -> UiPosition:
        """Set percentage offsets from anchor."""

    @property
    def anchor_value(self) -> Vec2:
        """The anchor point (normalized 0-1)."""

    @property
    def x(self) -> Val:
        """Horizontal offset."""

    @property
    def y(self) -> Val:
        """Vertical offset."""


class RadialGradientShape:
    """Shape of a radial gradient.

    Example:
        ```python
        from pybevy.ui import RadialGradientShape, Val

        shape = RadialGradientShape.circle(Val.px(50.0))
        shape = RadialGradientShape.ellipse(Val.px(100.0), Val.px(50.0))
        shape = RadialGradientShape.farthest_corner()
        ```
    """

    @staticmethod
    def closest_side() -> RadialGradientShape:
        """Circle with radius to closest side."""

    @staticmethod
    def farthest_side() -> RadialGradientShape:
        """Circle with radius to farthest side."""

    @staticmethod
    def closest_corner() -> RadialGradientShape:
        """Ellipse with extents to closest corner."""

    @staticmethod
    def farthest_corner() -> RadialGradientShape:
        """Ellipse with extents to farthest corner."""

    @staticmethod
    def circle(radius: Val) -> RadialGradientShape:
        """Circle with explicit radius."""

    @staticmethod
    def ellipse(width: Val, height: Val) -> RadialGradientShape:
        """Ellipse with explicit dimensions."""


class LinearGradient:
    """A linear gradient.

    Example:
        ```python
        from pybevy.ui import LinearGradient, ColorStop, BackgroundGradient, Gradient
        from pybevy.color import Color

        # Create a vertical gradient from red to blue
        gradient = LinearGradient.to_bottom([
            ColorStop.auto(Color.RED),
            ColorStop.auto(Color.BLUE),
        ])

        # Use with BackgroundGradient component
        commands.spawn((
            Node(),
            BackgroundGradient([Gradient.linear(gradient)]),
        ))
        ```
    """

    def __init__(self, angle: float, stops: list[ColorStop]) -> None:
        """Create a gradient with angle in radians (0 = up, clockwise)."""

    @staticmethod
    def degrees(degrees: float, stops: list[ColorStop]) -> LinearGradient:
        """Create a gradient with angle in degrees."""

    @staticmethod
    def to_top(stops: list[ColorStop]) -> LinearGradient:
        """Gradient pointing upward."""

    @staticmethod
    def to_bottom(stops: list[ColorStop]) -> LinearGradient:
        """Gradient pointing downward."""

    @staticmethod
    def to_left(stops: list[ColorStop]) -> LinearGradient:
        """Gradient pointing left."""

    @staticmethod
    def to_right(stops: list[ColorStop]) -> LinearGradient:
        """Gradient pointing right."""

    @staticmethod
    def to_top_left(stops: list[ColorStop]) -> LinearGradient:
        """Gradient pointing to top-left corner."""

    @staticmethod
    def to_top_right(stops: list[ColorStop]) -> LinearGradient:
        """Gradient pointing to top-right corner."""

    @staticmethod
    def to_bottom_left(stops: list[ColorStop]) -> LinearGradient:
        """Gradient pointing to bottom-left corner."""

    @staticmethod
    def to_bottom_right(stops: list[ColorStop]) -> LinearGradient:
        """Gradient pointing to bottom-right corner."""

    def in_color_space(self, color_space: InterpolationColorSpace) -> LinearGradient:
        """Set interpolation color space."""

    def in_oklaba(self) -> LinearGradient:
        """Use Oklaba color space for interpolation."""

    def in_srgb(self) -> LinearGradient:
        """Use sRGB color space for interpolation."""

    def in_linear_rgb(self) -> LinearGradient:
        """Use linear RGB color space for interpolation."""

    @property
    def color_space(self) -> InterpolationColorSpace:
        """The interpolation color space."""

    @property
    def angle(self) -> float:
        """The gradient angle in radians."""

    @property
    def stops(self) -> list[ColorStop]:
        """The color stops."""


class RadialGradient:
    """A radial gradient.

    Example:
        ```python
        from pybevy.ui import RadialGradient, RadialGradientShape, UiPosition, ColorStop
        from pybevy.color import Color

        gradient = RadialGradient(
            UiPosition.center(),
            RadialGradientShape.farthest_corner(),
            [ColorStop.auto(Color.WHITE()), ColorStop.auto(Color.BLACK())],
        )
        ```
    """

    def __init__(
        self, position: UiPosition, shape: RadialGradientShape, stops: list[ColorStop]
    ) -> None:
        """Create a radial gradient."""

    def in_color_space(self, color_space: InterpolationColorSpace) -> RadialGradient:
        """Set interpolation color space."""

    def in_oklaba(self) -> RadialGradient:
        """Use Oklaba color space for interpolation."""

    def in_srgb(self) -> RadialGradient:
        """Use sRGB color space for interpolation."""

    def in_linear_rgb(self) -> RadialGradient:
        """Use linear RGB color space for interpolation."""

    @property
    def color_space(self) -> InterpolationColorSpace:
        """The interpolation color space."""

    @property
    def position(self) -> UiPosition:
        """The center position."""

    @property
    def shape(self) -> RadialGradientShape:
        """The gradient shape."""

    @property
    def stops(self) -> list[ColorStop]:
        """The color stops."""

    def __eq__(self, other: object) -> bool: ...


class ConicGradient:
    """A conic (angular) gradient.

    Example:
        ```python
        from pybevy.ui import ConicGradient, UiPosition, AngularColorStop
        from pybevy.color import Color

        gradient = ConicGradient(
            UiPosition.center(),
            [AngularColorStop.auto(Color.RED), AngularColorStop.auto(Color.BLUE)],
        )
        ```
    """

    def __init__(self, position: UiPosition, stops: list[AngularColorStop]) -> None:
        """Create a conic gradient."""

    def with_start(self, start: float) -> ConicGradient:
        """Set starting angle in radians."""

    def with_position(self, position: UiPosition) -> ConicGradient:
        """Set center position."""

    def in_color_space(self, color_space: InterpolationColorSpace) -> ConicGradient:
        """Set interpolation color space."""

    def in_oklaba(self) -> ConicGradient:
        """Use Oklaba color space for interpolation."""

    def in_srgb(self) -> ConicGradient:
        """Use sRGB color space for interpolation."""

    def in_linear_rgb(self) -> ConicGradient:
        """Use linear RGB color space for interpolation."""

    @property
    def color_space(self) -> InterpolationColorSpace:
        """The interpolation color space."""

    @property
    def start(self) -> float:
        """The starting angle in radians."""

    @property
    def position(self) -> UiPosition:
        """The center position."""

    @property
    def stops(self) -> list[AngularColorStop]:
        """The angular color stops."""

    def __eq__(self, other: object) -> bool: ...


class Gradient:
    """A gradient (linear, radial, or conic).

    Wrapper enum for different gradient types.

    Example:
        ```python
        from pybevy.ui import Gradient, LinearGradient, ColorStop
        from pybevy.color import Color

        linear = LinearGradient.to_right([
            ColorStop.auto(Color.RED),
            ColorStop.auto(Color.BLUE),
        ])
        gradient = Gradient.linear(linear)
        ```
    """

    @staticmethod
    def linear(gradient: LinearGradient) -> Gradient:
        """Create from a linear gradient."""

    @staticmethod
    def radial(gradient: RadialGradient) -> Gradient:
        """Create from a radial gradient."""

    @staticmethod
    def conic(gradient: ConicGradient) -> Gradient:
        """Create from a conic gradient."""

    def is_empty(self) -> bool:
        """Check if gradient has no stops."""

    def get_single(self) -> Color | None:
        """Get single color if gradient has only one stop."""

    def __eq__(self, other: object) -> bool: ...


class ShadowStyle:
    """Style definition for UI shadows.

    Defines the appearance of a single shadow including color, offset,
    spread, and blur properties.

    Example:
        ```python
        from pybevy.ui import ShadowStyle, Val
        from pybevy.color import Color

        # Create a drop shadow
        shadow = ShadowStyle(
            color=Color.srgba(0.0, 0.0, 0.0, 0.5),
            x_offset=Val.px(4.0),
            y_offset=Val.px(4.0),
            spread_radius=Val.px(0.0),
            blur_radius=Val.px(8.0),
        )
        ```
    """

    def __init__(
        self,
        color: Color | None = None,
        x_offset: Val = ...,
        y_offset: Val = ...,
        spread_radius: Val = ...,
        blur_radius: Val = ...,
    ) -> None:
        """Create a shadow style.

        Args:
            color: The shadow's color
            x_offset: Horizontal offset
            y_offset: Vertical offset
            spread_radius: How much the shadow spreads (negative shrinks)
            blur_radius: Blurriness of the shadow
        """


    @property
    def color(self) -> Color:
        """The shadow's color."""

    @color.setter
    def color(self, value: Color) -> None: ...

    @property
    def x_offset(self) -> Val:
        """Horizontal offset."""

    @x_offset.setter
    def x_offset(self, value: Val) -> None: ...

    @property
    def y_offset(self) -> Val:
        """Vertical offset."""

    @y_offset.setter
    def y_offset(self, value: Val) -> None: ...

    @property
    def spread_radius(self) -> Val:
        """How much the shadow spreads outward (negative shrinks)."""

    @spread_radius.setter
    def spread_radius(self, value: Val) -> None: ...

    @property
    def blur_radius(self) -> Val:
        """Blurriness of the shadow."""

    @blur_radius.setter
    def blur_radius(self, value: Val) -> None: ...

    def __eq__(self, other: object) -> bool: ...


class BoxShadow(Component):
    """Component for drawing shadows behind UI nodes.

    Multiple shadows can be added and they render back-to-front.

    Example:
        ```python
        from pybevy.ui import BoxShadow, ShadowStyle, Val, Node
        from pybevy.color import Color

        def setup(commands: Commands) -> None:
            # Single shadow using convenience method
            shadow = BoxShadow.single(
                color=Color.srgba(0.0, 0.0, 0.0, 0.5),
                x_offset=Val.px(4.0),
                y_offset=Val.px(4.0),
                spread_radius=Val.px(0.0),
                blur_radius=Val.px(8.0),
            )

            commands.spawn((Node(), shadow))

            # Or with multiple shadows
            multi_shadow = BoxShadow([
                ShadowStyle(
                    Color.srgba(1.0, 0.0, 0.0, 0.3),
                    Val.px(-2.0), Val.px(-2.0),
                    Val.px(0.0), Val.px(4.0),
                ),
                ShadowStyle(
                    Color.srgba(0.0, 0.0, 1.0, 0.3),
                    Val.px(2.0), Val.px(2.0),
                    Val.px(0.0), Val.px(4.0),
                ),
            ])
        ```
    """

    def __init__(self, shadows: list[ShadowStyle] | None = None) -> None:
        """Create a BoxShadow with multiple shadow styles.

        Args:
            shadows: Optional list of shadow styles
        """

    @staticmethod
    def single(
        color: Color,
        x_offset: Val,
        y_offset: Val,
        spread_radius: Val,
        blur_radius: Val,
    ) -> BoxShadow:
        """Create a single drop shadow.

        Args:
            color: The shadow's color
            x_offset: Horizontal offset
            y_offset: Vertical offset
            spread_radius: How much the shadow spreads
            blur_radius: Blurriness of the shadow

        Returns:
            BoxShadow with a single shadow style
        """

    @property
    def shadows(self) -> list[ShadowStyle]:
        """All shadow styles."""

    @shadows.setter
    def shadows(self, value: list[ShadowStyle]) -> None: ...

    def push(self, style: ShadowStyle) -> None:
        """Add a shadow style to the list."""

    def pop(self) -> ShadowStyle | None:
        """Remove and return the last shadow style."""

    def __len__(self) -> int:
        """Number of shadow styles."""

    def is_empty(self) -> bool:
        """Check if there are no shadow styles."""

    def clear(self) -> None:
        """Clear all shadow styles."""

    def __getitem__(self, index: int) -> ShadowStyle:
        """Get shadow style by index."""

    def __setitem__(self, index: int, style: ShadowStyle) -> None:
        """Set shadow style by index."""


class BackgroundGradient(Component):
    """Component for displaying gradients on UI nodes.

    Example:
        ```python
        from pybevy.ui import BackgroundGradient, Gradient, LinearGradient, ColorStop, Node
        from pybevy.color import Color

        def setup(commands: Commands) -> None:
            gradient = LinearGradient.to_bottom([
                ColorStop.auto(Color.linear_rgb(0.2, 0.2, 0.8)),
                ColorStop.auto(Color.linear_rgb(0.8, 0.2, 0.2)),
            ])

            commands.spawn((
                Node(),
                BackgroundGradient([Gradient.linear(gradient)]),
            ))
        ```
    """

    def __init__(self, gradients: list[Gradient]) -> None:
        """Create with a list of gradients."""

    def add_gradient(self, gradient: Gradient) -> None:
        """Add a gradient to the list."""

    @property
    def gradients(self) -> list[Gradient]:
        """The list of gradients."""

    def len(self) -> int:
        """Number of gradients."""

    def is_empty(self) -> bool:
        """Check if no gradients."""

    def __eq__(self, other: object) -> bool: ...


class TextShadow(Component):
    """Adds a shadow behind text.

    Use Text2dShadow for Text2d shadows (not yet implemented).

    Example:
        ```python
        from pybevy.ui import Text, TextShadow, Node
        from pybevy.math import Vec2
        from pybevy.color import Color

        def setup(commands: Commands) -> None:
            commands.spawn((
                Node(),
                Text("Hello World"),
                TextShadow(
                    offset=Vec2(2.0, 2.0),
                    color=Color.srgba(0.0, 0.0, 0.0, 0.5),
                ),
            ))
        ```
    """

    def __init__(
        self,
        offset: Vec2 | None = None,
        color: Color | None = None,
    ) -> None:
        """Create a text shadow.

        Args:
            offset: Shadow displacement in logical pixels (default: Vec2.ZERO)
            color: Color of the shadow (default: BLACK)
        """

    @property
    def offset(self) -> Vec2:
        """Shadow displacement in logical pixels."""

    @offset.setter
    def offset(self, value: Vec2) -> None: ...

    @property
    def color(self) -> Color:
        """Color of the shadow."""

    @color.setter
    def color(self, value: Color) -> None: ...

    def __eq__(self, other: object) -> bool: ...


# Backwards compatibility alias: UiImage was the old PyBevy name
UiImage = ImageNode

__all__ = [
    "AlignContent",
    "AlignItems",
    "AlignSelf",
    "AngularColorStop",
    "BackgroundColor",
    "BackgroundGradient",
    "BorderColor",
    "BorderGradient",
    "BorderRadius",
    "BoxShadow",
    "BoxSizing",
    "Button",
    "Checked",
    "ColorStop",
    "ComputedNode",
    "ComputedStackIndex",
    "ConicGradient",
    "Display",
    "FlexDirection",
    "FlexWrap",
    "FocusPolicy",
    "GlobalZIndex",
    "Gradient",
    "GridAutoFlow",
    "GridPlacement",
    "GridTrack",
    "ImageNode",
    "InlineDirection",
    "Interaction",
    "InteractionDisabled",
    "InterpolationColorSpace",
    "IsDefaultUiCamera",
    "JustifyContent",
    "JustifyItems",
    "JustifySelf",
    "Label",
    "LinearGradient",
    "Node",
    "NodeImageMode",
    "Outline",
    "Overflow",
    "OverflowAxis",
    "OverflowClipMargin",
    "PositionType",
    "Pressed",
    "RadialGradient",
    "RadialGradientShape",
    "RelativeCursorPosition",
    "RepeatedGridTrack",
    "ScrollPosition",
    "ShadowStyle",
    "Text",
    "TextShadow",
    "UiImage",
    "UiPosition",
    "UiRect",
    "UiScale",
    "UiTargetCamera",
    "UiTransform",
    "Val",
    "Val2",
    "VisualBox",
    "ZIndex",
]
