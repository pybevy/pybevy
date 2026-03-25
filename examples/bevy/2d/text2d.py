"""2D text rendering with animations.

Demonstrates:
- Text2d component for world-space text
- Animated translation, rotation, and scaling
- Text styling (font, color, size)
- Text layout and justification
- Font smoothing options

Note: This demonstrates Text2d for rendering text in world space.
For UI text rendering (screen space), see the UI examples.
"""

import math

from pybevy.prelude import *


@component
class AnimateTranslation(Component):
    """Marker for text with translation animation."""


@component
class AnimateRotation(Component):
    """Marker for text with rotation animation."""


@component
class AnimateScale(Component):
    """Marker for text with scale animation."""


def setup(commands: Commands) -> None:
    """Set up 2D text examples with animations."""
    # Camera
    commands.spawn(Camera2d())

    # Note: Font loading not yet implemented in PyBevy,
    # so we use the default font with custom size
    text_font = TextFont.from_font_size(50.0)

    # Demonstrate changing translation
    commands.spawn(
        Text2d("translation"),
        text_font,
        TextLayout(justify=Justify.Center),
        TextColor.WHITE,
        Transform.from_xyz(-400.0, 100.0, 0.0),
        AnimateTranslation(),
    )

    # Demonstrate changing rotation
    commands.spawn(
        Text2d("rotation"),
        text_font,
        TextLayout(justify=Justify.Center),
        TextColor(Color.srgb(1.0, 0.8, 0.2)),  # Yellow
        Transform.from_xyz(0.0, 100.0, 0.0),
        AnimateRotation(),
    )

    # Demonstrate changing scale
    commands.spawn(
        Text2d("scale"),
        text_font,
        TextLayout(justify=Justify.Center),
        TextColor(Color.srgb(0.5, 1.0, 0.5)),  # Green
        Transform.from_xyz(400.0, 100.0, 0.0),
        AnimateScale(),
    )

    # Demonstrate font smoothing off (pixel art style)
    # Note: FontSmoothing enum might not be fully exposed, using default for now
    smaller_font = TextFont.from_font_size(35.0)

    commands.spawn(
        Text2d("This text has\nFontSmoothing.NoSmoothing\nAnd Justify.Center"),
        smaller_font,
        TextLayout(justify=Justify.Center),
        TextColor(Color.srgb(1.0, 0.5, 1.0)),  # Magenta
        Transform.from_xyz(-400.0, -150.0, 0.0),
    )

    # Demonstrate multi-line text with left justification
    commands.spawn(
        Text2d("Multi-line text\nLeft justified\nWith custom color"),
        TextFont.from_font_size(30.0),
        TextLayout(justify=Justify.Left),
        TextColor(Color.srgb(0.3, 0.8, 1.0)),  # Cyan
        Transform.from_xyz(200.0, -150.0, 0.0),
    )


def animate_translation(
    time: Res[Time],
    transforms: Query[Mut[Transform], With[tuple[Text2d, AnimateTranslation]]],
) -> None:
    """Animate text position in a circular pattern."""
    t = time.elapsed_secs()
    for transform in transforms:
        transform.translation.x = 100.0 * math.sin(t) - 400.0
        transform.translation.y = 100.0 * math.cos(t) + 100.0


def animate_rotation(
    time: Res[Time],
    transforms: Query[Mut[Transform], With[tuple[Text2d, AnimateRotation]]],
) -> None:
    """Animate text rotation."""
    t = time.elapsed_secs()
    for transform in transforms:
        transform.rotation = Quat.from_rotation_z(math.cos(t))


def animate_scale(
    time: Res[Time],
    transforms: Query[Mut[Transform], With[tuple[Text2d, AnimateScale]]],
) -> None:
    """Animate text scale.

    Note: Scaling a Text2d will scale the rendered quad, which can result
    in a pixellated look. Consider changing font-size instead for better quality.
    """
    t = time.elapsed_secs()
    for transform in transforms:
        scale = (math.sin(t) + 1.1) * 2.0
        transform.scale = Vec3.splat(scale)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (animate_translation, animate_rotation, animate_scale))
    )


if __name__ == "__main__":
    main().run()
