"""Keep an entity's identity across frames without keeping component references.

Components returned by a query are borrowed for one system invocation. Store the
entity's generational ``Entity`` ID in a resource, then use ``Query.get()`` to
reacquire its component on each frame. Click and hold the square to drag it.
"""

from dataclasses import dataclass

from pybevy.input import ButtonInput, MouseButton
from pybevy.prelude import *
from pybevy.window import PrimaryWindow, Window


@resource
@dataclass
class DragState(Resource):
    entity: Entity | None = None


def setup(commands: Commands) -> None:
    commands.spawn(Camera2d())
    commands.spawn(
        Sprite.from_color(Color.srgb(0.2, 0.4, 0.9), Vec2(100.0, 100.0)),
        Transform(),
    )


def drag(
    mouse: Res[ButtonInput[MouseButton]],
    state: ResMut[DragState],
    windows: Query[Window, With[PrimaryWindow]],
    objects: Query[tuple[Entity, Mut[Transform]], With[Sprite]],
) -> None:
    window = windows.single()
    cursor = window.cursor_position()
    if cursor is None:
        return

    position = Vec2(
        cursor.x - window.width() / 2.0,
        window.height() / 2.0 - cursor.y,
    )
    left = MouseButton.Left()

    if mouse.just_pressed(left):
        for entity, transform in objects:
            if (
                abs(position.x - transform.translation.x) <= 50.0
                and abs(position.y - transform.translation.y) <= 50.0
            ):
                state.entity = entity

    if mouse.pressed(left) and state.entity is not None:
        result = objects.get(state.entity)
        if result is None:
            state.entity = None
        else:
            _, transform = result
            transform.translation = Vec3(position.x, position.y, 0.0)

    if mouse.just_released(left):
        state.entity = None


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(DragState())
        .add_systems(Startup, setup)
        .add_systems(Update, drag)
    )


if __name__ == "__main__":
    main().run()
