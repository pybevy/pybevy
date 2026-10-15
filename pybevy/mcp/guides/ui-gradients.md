# UI Gradients Guide

CSS-style gradients for UI backgrounds and borders: linear, radial, and conic.

## Gradient Types

| Type | Best for | Example use |
|------|----------|-------------|
| `LinearGradient` | Bars, headers, buttons | Top-to-bottom color fade |
| `RadialGradient` | Spotlights, badges, orbs | Center-to-edge circular fade |
| `ConicGradient` | Pie charts, spinners, color wheels | Angle-based sweep |

## LinearGradient

```python
from pybevy.color import Color
from pybevy.ui import (
    LinearGradient, RadialGradient, ConicGradient,
    ColorStop, BackgroundGradient, BorderGradient, Gradient,
    UiPosition, RadialGradientShape, AngularColorStop,
    Node, UiRect, Val,
)

# Top-to-bottom gradient
bg = LinearGradient.to_bottom([
    ColorStop.auto(Color.srgb(0.2, 0.4, 0.8)),
    ColorStop.auto(Color.srgb(0.1, 0.1, 0.3)),
])

# Diagonal with explicit angle (radians)
bg = LinearGradient(0.785, [  # 45 degrees
    ColorStop.auto(Color.srgb(1.0, 0.5, 0.0)),
    ColorStop.auto(Color.srgb(0.8, 0.0, 0.4)),
])

# Using degrees helper
bg = LinearGradient.degrees(135.0, [
    ColorStop.auto(Color.srgb(0.0, 0.8, 0.6)),
    ColorStop.auto(Color.srgb(0.0, 0.3, 0.5)),
])
```

**Direction helpers:** `.to_top()`, `.to_bottom()`, `.to_left()`, `.to_right()`, `.to_top_left()`, `.to_top_right()`, `.to_bottom_left()`, `.to_bottom_right()`

## RadialGradient

`UiPosition.CENTER`, `TOP_LEFT`, and the other Bevy position constants have
zero-pixel offsets. Position constructors such as `UiPosition.center(x, y)`
require both offsets.

```python
# Center-outward circular gradient
bg = RadialGradient(
    UiPosition.CENTER,
    RadialGradientShape.FarthestCorner(),
    [
        ColorStop.auto(Color.srgb(1.0, 1.0, 1.0)),
        ColorStop.auto(Color.srgb(0.2, 0.2, 0.3)),
    ],
)

# Fixed-size circle
bg = RadialGradient(
    UiPosition.CENTER,
    RadialGradientShape.Circle(Val.Px(50.0)),
    [
        ColorStop.auto(Color.srgb(1.0, 0.8, 0.0)),
        ColorStop.auto(Color.srgba(1.0, 0.8, 0.0, 0.0)),
    ],
)
```

**Shapes:** `RadialGradientShape.ClosestSide()`,
`RadialGradientShape.FarthestSide()`,
`RadialGradientShape.ClosestCorner()`,
`RadialGradientShape.FarthestCorner()`,
`RadialGradientShape.Circle(value)`, and
`RadialGradientShape.Ellipse(width, height)`.
Circle and ellipse payloads are available as `.value` and `.width`/`.height`.

## ConicGradient

```python
# Color wheel
bg = ConicGradient(
    UiPosition.CENTER,
    [
        AngularColorStop.auto(Color.srgb(1.0, 0.0, 0.0)),
        AngularColorStop.auto(Color.srgb(1.0, 1.0, 0.0)),
        AngularColorStop.auto(Color.srgb(0.0, 1.0, 0.0)),
        AngularColorStop.auto(Color.srgb(0.0, 0.0, 1.0)),
    ],
)
```

## Applying Gradients to UI Nodes

Wrap gradients in `Gradient.Linear(value)`, `Gradient.Radial(value)`, or
`Gradient.Conic(value)` and use `BackgroundGradient` or `BorderGradient`.
Each variant exposes its payload through `.value` and supports `match`:

```python
from pybevy.ui import BackgroundGradient, Gradient, LinearGradient

background_gradient = BackgroundGradient([Gradient.Linear(LinearGradient.to_bottom([]))])
for gradient in background_gradient.gradients:
    match gradient:
        case Gradient.Linear(linear):
            assert linear.stops == []
        case Gradient.Radial(radial):
            assert radial.shape is not None
        case Gradient.Conic(conic):
            assert conic.start == 0.0
```

```python
# Background gradient on a node
commands.spawn(
    Node(),
    BackgroundGradient([Gradient.Linear(
        LinearGradient.to_bottom([
            ColorStop.auto(Color.srgb(0.2, 0.5, 0.9)),
            ColorStop.auto(Color.srgb(0.1, 0.2, 0.4)),
        ])
    )]),
)

# Border gradient
border_node = Node()
border_node.border = UiRect.all(Val.Px(2.0))
commands.spawn(
    border_node,
    BorderGradient([Gradient.Linear(
        LinearGradient.to_right([
            ColorStop.auto(Color.srgb(1.0, 0.0, 0.5)),
            ColorStop.auto(Color.srgb(0.5, 0.0, 1.0)),
        ])
    )]),
)
```

## Color Stops

Control where colors are placed:

Standalone gradients and stops support whole-field assignment. Child values
returned by `stops`, `position`, `color`, and `anchor_value` are read-only
snapshots: replace the parent's field instead of mutating a returned child.
For example, assign `linear.stops = [ColorStop.auto(Color.WHITE)]` or
`radial.position = UiPosition.TOP_LEFT`. Replace a component's entire
`gradients` list through `Query[Mut[BackgroundGradient]]` or
`Query[Mut[BorderGradient]]`; a plain `Query` remains read-only.

```python
ColorStop.auto(color)              # Evenly distributed
ColorStop.px(color, 20.0)          # At 20 pixels
ColorStop.percent(color, 75.0)     # At 75%

# With interpolation hint (midpoint between this and next stop)
ColorStop.auto(Color.srgb(1.0, 0.0, 0.0)).with_hint(0.3)  # Color shifts earlier
```

## Color Space

Gradients default to Oklab. Use `.in_srgb()` or `.in_linear_rgb()` to select another space.

```python
# Select sRGB interpolation explicitly.
linear = LinearGradient.to_right([
    ColorStop.auto(Color.srgb(1.0, 0.0, 0.0)),
    ColorStop.auto(Color.srgb(0.0, 0.0, 1.0)),
]).in_srgb()
```

## TextShadow

Drop shadow on text - added as a component on text entities:

```python
from pybevy.ui import TextShadow

commands.spawn(
    Text("Title"),
    TextShadow(offset=Vec2(2.0, 2.0), color=Color.srgba(0.0, 0.0, 0.0, 0.5)),
)
```

## Recipe: Styled Button

```python
def spawn_button(commands: Commands) -> None:
    button_node = Node()
    button_node.border = UiRect.all(Val.Px(2.0))
    commands.spawn(
        Button(),
        button_node,
        BackgroundGradient([Gradient.Linear(
            LinearGradient.to_bottom([
                ColorStop.auto(Color.srgb(0.3, 0.6, 1.0)),
                ColorStop.auto(Color.srgb(0.15, 0.3, 0.7)),
            ]).in_oklaba()
        )]),
        BorderGradient([Gradient.Linear(
            LinearGradient.to_bottom([
                ColorStop.auto(Color.srgba(1.0, 1.0, 1.0, 0.3)),
                ColorStop.auto(Color.srgba(1.0, 1.0, 1.0, 0.05)),
            ])
        )]),
    ).with_children(lambda parent: parent.spawn(
        Text("Click Me"),
        TextShadow(offset=Vec2(1.0, 1.0), color=Color.srgba(0.0, 0.0, 0.0, 0.4)),
    ))
```

**For all parameters:** `get_type_definition('LinearGradient')`, `get_type_definition('BackgroundGradient')`
