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
from pybevy.ui import (
    LinearGradient, RadialGradient, ConicGradient,
    ColorStop, BackgroundGradient, BorderGradient, Gradient,
    UiPosition, RadialGradientShape, AngularColorStop,
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

```python
# Center-outward circular gradient
bg = RadialGradient(
    UiPosition.center(),
    RadialGradientShape.farthest_corner(),
    [
        ColorStop.auto(Color.srgb(1.0, 1.0, 1.0)),
        ColorStop.auto(Color.srgb(0.2, 0.2, 0.3)),
    ],
)

# Fixed-size circle
bg = RadialGradient(
    UiPosition.center(),
    RadialGradientShape.circle(Val.px(50.0)),
    [
        ColorStop.auto(Color.srgb(1.0, 0.8, 0.0)),
        ColorStop.auto(Color.srgba(1.0, 0.8, 0.0, 0.0)),
    ],
)
```

**Shapes:** `.closest_side()`, `.farthest_side()`, `.closest_corner()`, `.farthest_corner()`, `.circle(radius)`, `.ellipse(width, height)`

## ConicGradient

```python
# Color wheel
bg = ConicGradient(
    UiPosition.center(),
    [
        AngularColorStop.auto(Color.srgb(1.0, 0.0, 0.0)),
        AngularColorStop.auto(Color.srgb(1.0, 1.0, 0.0)),
        AngularColorStop.auto(Color.srgb(0.0, 1.0, 0.0)),
        AngularColorStop.auto(Color.srgb(0.0, 0.0, 1.0)),
    ],
)
```

## Applying Gradients to UI Nodes

Wrap gradients in `Gradient.linear()`/`.radial()`/`.conic()` and use `BackgroundGradient` or `BorderGradient`:

```python
# Background gradient on a node
commands.spawn(
    Node(),
    BackgroundGradient([Gradient.linear(
        LinearGradient.to_bottom([
            ColorStop.auto(Color.srgb(0.2, 0.5, 0.9)),
            ColorStop.auto(Color.srgb(0.1, 0.2, 0.4)),
        ])
    )]),
)

# Border gradient
commands.spawn(
    Node(),
    BorderGradient([Gradient.linear(
        LinearGradient.to_right([
            ColorStop.auto(Color.srgb(1.0, 0.0, 0.5)),
            ColorStop.auto(Color.srgb(0.5, 0.0, 1.0)),
        ])
    )]),
)
```

## Color Stops

Control where colors are placed:

```python
ColorStop.auto(color)              # Evenly distributed
ColorStop.px(color, 20.0)          # At 20 pixels
ColorStop.percent(color, 0.75)     # At 75%

# With interpolation hint (midpoint between this and next stop)
ColorStop.auto(Color.RED).with_hint(0.3)  # Color shifts earlier
```

## Color Space

Gradients default to sRGB interpolation. For perceptually smoother transitions:

```python
smooth = LinearGradient.to_right([
    ColorStop.auto(Color.srgb(1.0, 0.0, 0.0)),
    ColorStop.auto(Color.srgb(0.0, 0.0, 1.0)),
]).in_oklaba()  # Perceptually uniform — no muddy middle
```

Options: `.in_oklaba()`, `.in_srgb()`, `.in_linear_rgb()`

## TextShadow

Drop shadow on text — added as a component on text entities:

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
    commands.spawn(
        Button(),
        Node(),
        BackgroundGradient([Gradient.linear(
            LinearGradient.to_bottom([
                ColorStop.auto(Color.srgb(0.3, 0.6, 1.0)),
                ColorStop.auto(Color.srgb(0.15, 0.3, 0.7)),
            ]).in_oklaba()
        )]),
        BorderGradient([Gradient.linear(
            LinearGradient.to_bottom([
                ColorStop.auto(Color.srgba(1.0, 1.0, 1.0, 0.3)),
                ColorStop.auto(Color.srgba(1.0, 1.0, 1.0, 0.05)),
            ])
        )]),
    ).with_children(|cb| {
        cb.spawn(
            Text("Click Me"),
            TextShadow(offset=Vec2(1.0, 1.0), color=Color.srgba(0, 0, 0, 0.4)),
        )
    })
```

**For all parameters:** `get_type_definition('LinearGradient')`, `get_type_definition('BackgroundGradient')`
