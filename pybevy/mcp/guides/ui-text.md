# UI & Text Guide

Rendering text overlays, HUDs, and UI elements using Bevy's flexbox-based UI system.

## Text2d camera setup

`Text2d` renders through `Camera2d`. For an overlay above a base camera with
order 0, copy its `RenderTarget` component and match its viewport:

```python
from copy import copy

from pybevy.camera import Camera, Camera2d, ClearColorConfig

commands.spawn(
    Camera2d(),
    copy(base_render_target),
    Camera(order=1, clear_color=ClearColorConfig.None_(),
           viewport=base_camera.viewport),
)
```

For HUDs use UI `Text` + `Node`; world-anchored labels require projecting
world positions into the viewport.

## UI Text vs World Text

PyBevy has two text systems:

| | UI Text | World Text |
|---|---------|------------|
| Component | `Text` (from `pybevy.ui`) | `Text2d` (from `pybevy.text`) |
| Positioning | Screen-space via `Node` | World-space via `Transform` |
| Use case | HUDs, subtitles, menus | 2D game labels |
| Layout | `Node` | `Transform` |

## UI Text (Screen Overlay)

### Basic Usage

```python
from pybevy.ui import BackgroundColor, Node, PositionType, Text
from pybevy.text import TextFont, TextColor, TextLayout, Justify

# Simple text at default position (top-left)
commands.spawn(
    Text("Hello, world!"),
    Node(),
    TextFont.from_font_size(24.0),
    TextColor(Color.WHITE),
)
```

### Font Sizes and Sources

`font_size` accepts a float (pixels) or a `FontSize` unit. For portable scenes,
load a font asset and pass its handle. The official PyBevy build follows Bevy's
default feature set and does not enable system-font discovery, so family names,
generic `FontSource` values, `weight`, and `style` do not select installed system
fonts there. Those selectors are useful only in a custom build with Bevy's
`system_font_discovery` feature enabled.

```python
from pybevy.text import FontSize

TextFont(font_size=FontSize.Rem(1.5))          # relative to root font size (Px/Vw/Vh/VMin/VMax/Rem)
TextFont(font=asset_server.load_font("fonts/FiraMono-Medium.ttf"), font_size=24.0)
```

### Letter Spacing

`LetterSpacing` is its own component on the text entity (mirroring bevy), not a `TextFont` field. `Px` is absolute; `Rem` scales with the root font size:

```python
from pybevy.text import LetterSpacing

commands.spawn(
    Text("W I D E   T I T L E"),
    Node(),
    TextFont.from_font_size(32.0),
    LetterSpacing.Px(4.0),   # or LetterSpacing.Rem(0.25)
)
```

### Absolute Positioning

Use `PositionType.Absolute` to place text anywhere on screen:

```python
commands.spawn(
    Text("Bottom text"),
    Node(
        position_type=PositionType.Absolute,
        bottom=50.0,     # 50px from bottom
        left=0.0,        # Flush left
    ),
    TextFont.from_font_size(28.0),
    TextColor(Color.WHITE),
)
```

**Node position properties:** `top`, `bottom`, `left`, `right`. Set as `float` for pixel values (auto-converted to `Val.Px`), or use `Val.Px(50.0)` / `Val.Percent(50.0)` explicitly.

### Centered Subtitles

For centered text like movie subtitles, set a wide width and use `Justify.Center`:

```python
commands.spawn(
    Text("A long time ago, in a galaxy far, far away..."),
    Node(
        position_type=PositionType.Absolute,
        bottom=50.0,
        left=0.0,
        width=1920.0,  # Wide enough for any screen
    ),
    TextFont.from_font_size(28.0),
    TextColor(Color.srgba(1.0, 0.95, 0.8, 1.0)),  # Warm white
    TextLayout(justify=Justify.Center),
    BackgroundColor(Color.srgba(0.0, 0.0, 0.0, 0.45)),  # Semi-transparent backdrop
)
```

### Text with Background

`BackgroundColor` adds a colored rectangle behind the text node:

```python
commands.spawn(
    Text("Score: 0"),
    Node(),
    TextFont.from_font_size(20.0),
    TextColor.WHITE,
    BackgroundColor(Color.srgba(0.0, 0.0, 0.0, 0.5)),  # 50% black backdrop
)
```

## Updating Text at Runtime

The `Text` component has a `.text` property you can mutate in systems, just like
`Text2d` and `TextSpan`. Construct it with `Text("Hello")` or `Text(text="Hello")`.

For example:

```python
@component
class ScoreDisplay(Component):
    pass

# In setup:
commands.spawn(
    Text("Score: 0"),
    Node(),
    TextFont.from_font_size(24.0),
    TextColor.WHITE,
    ScoreDisplay(),
)

# In update system:
def update_score_display(
    query: Query[Mut[Text], With[ScoreDisplay]],
    score: Res[GameScore],
):
    for text in query:
        text.text = f"Score: {score.value}"
```

## Text Input (EditableText)

`EditableText` turns a UI node into a text input field. Typing, cursor movement,
selection, and clipboard are handled by the engine; click the field to focus it.
There is no built-in submit event: read `.value` from a system, then call
`.clear()` through a mutable query to reset the field.

Use `DefaultPlugins` for editable UI text. Adding `TextPlugin` alone installs
the queued-edit application system, but not the UI layout systems that shape the
editor before cursor, selection, and deletion commands run. PyBevy does not
expose Bevy's `UiPlugin` separately, so the supported public setup is
`DefaultPlugins` with `EditableText` on a UI `Node`.

```python
from pybevy.text import EditableText

# In setup:
commands.spawn(
    EditableText("type here", max_characters=64),
    Node(width=320.0, height=40.0),  # bare floats are pixels
    TextFont.from_font_size(24.0),
    BackgroundColor(Color.srgb(0.15, 0.15, 0.2)),
)

# In an update system:
def read_input(query: Query[EditableText]):
    for field in query:
        if field.value:
            ...
```

Options worth knowing:

- Single-line by default; `allow_newlines=True` with `visible_lines=4.0` makes a multiline box.
- `cursor_blink_period` accepts a `timedelta` or plain seconds (`0.5`); `cursor_width` is relative to the font size.
- `max_characters=None` (the default) means unlimited.
- Focus follows clicks. Tab-navigation and auto-focus components are not wrapped yet, so make each field's node large enough to click comfortably.

## Fading Text

Animate `TextColor` alpha for fade-in/fade-out effects:

```python
def fade_text(
    query: Query[tuple[Mut[TextColor], Mut[BackgroundColor]], With[MyText]],
    time: Res[Time],
):
    # Fade in over 2 seconds
    alpha = min(1.0, time.elapsed_secs() / 2.0)
    # Smoothstep for eased fade
    alpha = alpha * alpha * (3.0 - 2.0 * alpha)

    for text_color, bg_color in query:
        text_color.color = Color.srgba(1.0, 1.0, 1.0, alpha)
        bg_color.color = Color.srgba(0.0, 0.0, 0.0, alpha * 0.5)
```

## World-Space Text (Text2d)

```python
from pybevy.text import Text2d, TextBounds, TextFont, TextColor

commands.spawn(
    Text2d("Player Name"),
    TextFont.from_font_size(50.0),
    TextColor(Color.srgb(1.0, 1.0, 0.0)),
    Transform.from_xyz(0.0, 200.0, 0.0),
)
```

For long world-space strings, provide an explicit `TextBounds(width=...)` to
control wrapping. In Bevy 0.19, the camera target still clips rendered text and
`TextBounds.height` does not truncate laid-out lines. Use width, explicit line
breaks, or UI `Text` when content must remain fully visible within a screen
region.

## Node Layout Quick Reference

| Property | Type | Description |
|----------|------|-------------|
| `position_type` | `PositionType` | `.Relative` (flow) or `.Absolute` |
| `top`, `bottom`, `left`, `right` | `float` or `Val` | Offset (float = pixels, or `Val.Px(10.0)` / `Val.Percent(50.0)`) |
| `width`, `height` | `float` or `Val` | Size (float = pixels, or `Val.Px(10.0)` / `Val.Percent(50.0)`) |
| `flex_direction` | `FlexDirection` | `.Row`, `.Column`, `.RowReverse`, `.ColumnReverse` |
| `justify_content` | `JustifyContent` | `.Start`, `.Center`, `.End`, `.SpaceBetween` |
| `align_items` | `AlignItems` | `.Start`, `.Center`, `.End`, `.Stretch` |

For percentage-based values, use `Val.Percent(50.0)` on properties that accept `Val`.

## Complete HUD Example

```python
from pybevy.ui import BackgroundColor, Node, PositionType, Text
from pybevy.text import TextFont, TextColor

def setup_hud(commands: Commands):
    # Top-left score
    score_node = Node()
    score_node.position_type = PositionType.Absolute
    score_node.top = 10.0
    score_node.left = 10.0
    commands.spawn(
        Text("Score: 0"),
        score_node,
        TextFont.from_font_size(20.0),
        TextColor.WHITE,
        BackgroundColor(Color.srgba(0.0, 0.0, 0.0, 0.4)),
    )

    # Bottom-center narration
    narration_node = Node()
    narration_node.position_type = PositionType.Absolute
    narration_node.bottom = 50.0
    narration_node.left = 0.0
    narration_node.width = 1920.0
    commands.spawn(
        Text("Welcome to the adventure..."),
        narration_node,
        TextFont.from_font_size(28.0),
        TextColor(Color.srgba(1.0, 0.95, 0.8, 1.0)),
        TextLayout(justify=Justify.Center),
        BackgroundColor(Color.srgba(0.0, 0.0, 0.0, 0.45)),
    )
```
