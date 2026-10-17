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
Node construction and property assignments, including margin, padding,
border, flex growth and shrink, scrollbar width and live rect-side writes,
reject NaN and infinity before they reach Bevy's layout, naming the field:
`Node.width must be finite (got inf)`. Padding and border sides must also be
non-negative (`Node.padding.left must be finite and non-negative (got -1.0)`).
Node batch columns and MCP writes check the same rules; View arithmetic follows
NumPy float semantics. Standalone `Val` values can still carry non-finite data;
`UiRect` rejects it in its constructors, `with_*` builders and setters.
Grid-track numeric factories require finite values at construction.

Scalar multiplication, division and negation preserve the unit. `try_add` and
`try_sub` return a value for matching numeric units and raise `ValueError` for
incompatible units, including two `Auto` values. Zero values from different
units compare equal in Bevy but still have incompatible arithmetic units.
Val arithmetic follows Bevy float behavior, including infinities and NaNs;
zero division produces float infinity/NaN, and scalar arithmetic preserves
`Auto`. A non-finite result cannot be assigned to a Node layout field.

## Borders, Outlines, and Shadows

`Node.border` reserves layout space. `BorderColor` colors that border.
`Outline` draws outside the border box without affecting layout, and
`BoxShadow` draws one or more shadows behind the node.

```python
from pybevy.color import Color
from pybevy.ecs import Commands, system
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


@system
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
        BoxShadow.new(
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

## Resolved Geometry

After UI layout has run, query `ComputedNode` for the resolved size and
`UiGlobalTransform` for the absolute screen-space transform. The translation is
reported in physical pixels:

```python
from pybevy.ecs import Query, system
from pybevy.ui import ComputedNode, UiGlobalTransform


@system
def inspect_layout(query: Query[tuple[ComputedNode, UiGlobalTransform]]) -> None:
    for computed, transform in query:
        print("size", computed.size, "position", transform.translation)
```

`UiGlobalTransform` is engine-computed and cannot be inserted directly. Change
`UiTransform` when you need to translate, rotate, or scale a UI node.
Nested `Node.border_radius`, `Node.overflow_clip_margin`, and
`UiTransform.translation` values write through when reached through `Query[Mut[T]]`.
Their fields reject writes from read-only queries and expire with the system.

Bevy permits explicit `ComputedNode` adjustments after layout, for example
for scrollbar geometry. Use `Query[Mut[ComputedNode]]` to assign its fields
or change a vector child such as `computed.size.x`. These children are live
borrows, require mutable access, and expire when the system ends. A later
layout pass can overwrite your adjustment. `ComputedStackIndex.value` is
also writable, but the UI stacking system normally recomputes it.

## CSS Grid

`grid_template_rows` and `grid_template_columns` accept both `GridTrack` and
`RepeatedGridTrack`; a bare track means one repetition. `grid_auto_rows` and
`grid_auto_columns` accept only `GridTrack`.

```python
from pybevy.ui import Display, GridTrack, Node, RepeatedGridTrack

grid = Node(
    display=Display.Grid,
    grid_template_columns=[GridTrack.px(90.0), RepeatedGridTrack.fr(2, 1.0)],
    grid_auto_rows=[GridTrack.auto()],
)
```

## Layering

`ZIndex(value)` orders siblings and descendants within a UI stacking context;
higher values render on top. Use `GlobalZIndex(value)` only when an element
must compare across stacking contexts.

```python
commands.spawn(Node(), ZIndex(10))
commands.spawn(Node(), GlobalZIndex(100))
```

## Images and Intrinsic Size

`ImageNode(handle)` defaults to `NodeImageMode.Auto()`: an unsized `Node` takes
the image's intrinsic size, while explicit dimensions preserve its aspect ratio.
Use `NodeImageMode.Stretch()` to fill the node instead:

```python
from pybevy.ui import ImageNode, Node, NodeImageMode, Val

image = ImageNode(handle)
image.image_mode = NodeImageMode.Stretch()
commands.spawn(Node(width=Val.Px(200.0), height=Val.Px(80.0)), image)
```

`ImageNode.solid_color(color)` uses a 1-by-1 texture and also stays at
`NodeImageMode.Auto`, so an explicitly sized node shows a centered square of
its shorter side (80-by-80 on a 200-by-80 node), not a filled area. Assign
`NodeImageMode.Stretch()` to cover the node. For a plain colored panel,
`BackgroundColor` is usually simpler.

## Global UI Scale

`UiScale` is a resource. Insert it at app construction or through `Commands`,
or mutate the existing value with `ResMut[UiScale]`:

```python
from pybevy.ecs import ResMut, system
from pybevy.ui import UiScale

app.insert_resource(UiScale(1.5))

@system
def zoom_ui(scale: ResMut[UiScale]) -> None:
    scale.scale = 2.0
```

The scale must be finite and greater than zero. Invalid constructor values,
resource writes, and scene-editing resource patches raise an error before UI
layout runs.
