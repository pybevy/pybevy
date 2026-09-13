# UI Layout and Decoration

Use `Node` for layout, then add visual components to the same entity. UI text
and gradients have dedicated `ui-text` and `ui-gradients` guides.

## Units

Use `px(value)`, `percent(value)`, `vw(value)`, `vh(value)`, `vmin(value)` and
`vmax(value)` from `pybevy.ui`, or construct exact variants such as
`Val.Px(12.0)` and `Val.Auto()`. Payloads are read-only; replace the whole Node
field to change its value. Read numeric payloads with `.value` after matching
the variant; use `isinstance(value, Val.Auto)` or `value == Val.Auto()` for Auto.
Bare numeric Node fields mean pixels.

Scalar multiplication, division and negation preserve the unit. `try_add` and
`try_sub` return a value for matching numeric units and raise `ValueError` for
incompatible units, including two `Auto` values. Zero values from different
units compare equal in Bevy but still have incompatible arithmetic units.
Unit inputs and arithmetic follow Bevy float behavior, including infinities and
NaNs; zero division produces float infinity/NaN, and scalar arithmetic preserves
`Auto`.

## Borders, Outlines, and Shadows

`Node.border` reserves layout space. `BorderColor` colors that border.
`Outline` draws outside the border box without affecting layout, and
`BoxShadow` draws one or more shadows behind the node.

```python
from pybevy.color import Color
from pybevy.ecs import Commands
from pybevy.ui import (
    BackgroundColor,
    BorderColor,
    BoxShadow,
    GlobalZIndex,
    Node,
    Outline,
    UiRect,
    Val,
    ZIndex,
)


def setup(commands: Commands) -> None:
    panel = Node(
        width=Val.Px(320.0),
        height=Val.Px(180.0),
        border=UiRect.all(Val.Px(2.0)),
    )

    commands.spawn(
        panel,
        BackgroundColor(Color.srgb(0.08, 0.10, 0.15)),
        BorderColor.all(Color.srgb(0.25, 0.55, 1.0)),
        Outline(
            Val.Px(1.0),
            Val.Px(3.0),
            Color.srgba(0.4, 0.7, 1.0, 0.7),
        ),
        BoxShadow.single(
            Color.srgba(0.0, 0.0, 0.0, 0.45),
            Val.Px(6.0),
            Val.Px(8.0),
            Val.Px(0.0),
            Val.Px(12.0),
        ),
    )
```

Use `BorderColor(top=..., right=..., bottom=..., left=...)` when sides need
different colors. `border.set_all(color)` mutates all sides and returns that same
border; a queried border still requires `Mut[BorderColor]` and expires with its system.
Use `BoxShadow([ShadowStyle(color=color), ...])` for layered shadows.

## Layering

`ZIndex(value)` orders siblings and descendants within a UI stacking context;
higher values render on top. Use `GlobalZIndex(value)` only when an element
must compare across stacking contexts.

```python
commands.spawn(Node(), ZIndex(10))
commands.spawn(Node(), GlobalZIndex(100))
```

## Images and Intrinsic Size

An unsized `Node` containing `ImageNode(handle)` takes its intrinsic size from
the image. Set `Node.width` and `Node.height` to stretch it to a known area.
`ImageNode.solid_color(color)` uses a 1-by-1 texture, so it also needs an
explicitly sized node to fill more than one pixel. For a plain colored panel,
`BackgroundColor` is usually simpler.

## Global UI Scale

`UiScale` is a resource. Insert it at app construction or through `Commands`,
or mutate the existing value with `ResMut[UiScale]`:

```python
from pybevy.ecs import ResMut
from pybevy.ui import UiScale

app.insert_resource(UiScale(1.5))

def zoom_ui(scale: ResMut[UiScale]) -> None:
    scale.scale = 2.0
```
