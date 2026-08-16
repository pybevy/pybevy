# UI Layout and Decoration

Use `Node` for layout, then add visual components to the same entity. UI text
and gradients have dedicated `ui-text` and `ui-gradients` guides.

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
        width=Val.px(320.0),
        height=Val.px(180.0),
        border=UiRect.all(Val.px(2.0)),
    )

    commands.spawn(
        panel,
        BackgroundColor(Color.srgb(0.08, 0.10, 0.15)),
        BorderColor.all(Color.srgb(0.25, 0.55, 1.0)),
        Outline(
            Val.px(1.0),
            Val.px(3.0),
            Color.srgba(0.4, 0.7, 1.0, 0.7),
        ),
        BoxShadow.single(
            Color.srgba(0.0, 0.0, 0.0, 0.45),
            Val.px(6.0),
            Val.px(8.0),
            Val.px(0.0),
            Val.px(12.0),
        ),
    )
```

Use `BorderColor(top=..., right=..., bottom=..., left=...)` when sides need
different colors. Use `BoxShadow([ShadowStyle(...), ...])` for layered shadows.

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
