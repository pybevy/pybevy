"""Renders an animated sprite by loading all animation frames from a single image.

This example demonstrates:
- Sprite sheet animation using TextureAtlas
- Automatic frame advancement with Timer
- Custom components for animation state
- Mutable field access on Sprite.texture_atlas.index

The sprite sheet is divided into a grid, and the animation cycles through
a subset of frames at a fixed frame rate.

Note: PyBevy doesn't support DefaultPlugins.set() yet, so ImagePlugin
configuration (nearest-neighbor filtering) is not available. Sprites may
appear slightly blurred at certain scales.
"""

from dataclasses import dataclass

from pybevy.image import TextureAtlas, TextureAtlasLayout
from pybevy.math import UVec2
from pybevy.prelude import *


@component
@dataclass
class AnimationIndices(Component):
    """Defines which frames in the sprite sheet to use for animation.

    Attributes:
        first: Index of the first frame in the animation sequence
        last: Index of the last frame in the animation sequence
    """
    first: int
    last: int


@component(storage="python")
@dataclass
class AnimationTimer(Component):
    """Timer controlling animation frame advancement.

    Attributes:
        timer: Timer that ticks each frame
    """
    timer: Timer


def animate_sprite(
    time: Res[Time],
    query: Query[tuple[AnimationIndices, Mut[AnimationTimer], Mut[Sprite]]],
) -> None:
    """Advance sprite animation frames based on timer."""
    for indices, timer_component, sprite in query:
        timer_component.timer.tick(time.delta_secs())

        if timer_component.timer.just_finished():
            atlas = sprite.texture_atlas
            if atlas is not None:
                # Cycle to next frame, wrapping back to first after last
                if atlas.index == indices.last:
                    atlas.index = indices.first
                else:
                    atlas.index += 1


def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    texture_atlas_layouts: ResMut[Assets[TextureAtlasLayout]],
) -> None:
    """Set up camera and animated sprite."""
    # Load the sprite sheet texture
    texture = asset_server.load_image("bevy/textures/rpg/chars/gabe/gabe-idle-run.png")

    # Create a texture atlas layout from a sprite sheet grid
    # The sprite sheet has 7 frames in a single row, each 24x24 pixels
    layout = TextureAtlasLayout.from_grid(
        tile_size=UVec2.splat(24),
        columns=7,
        rows=1,
        padding=None,
        offset=None,
    )
    texture_atlas_layout = texture_atlas_layouts.add(layout)

    # Use only the subset of sprites that make up the run animation (frames 1-6)
    animation_indices = AnimationIndices(first=1, last=6)

    commands.spawn(Camera2d())

    commands.spawn(
        Sprite.from_atlas_image(
            texture,
            TextureAtlas(
                layout=texture_atlas_layout,
                index=animation_indices.first,
            ),
        ),
        Transform.from_scale(Vec3.splat(6.0)),
        animation_indices,
        AnimationTimer(timer=Timer(0.1, TimerMode.REPEATING)),
    )


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, animate_sprite)
    )


if __name__ == "__main__":
    main().run()
