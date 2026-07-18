"""Animates sprites in response to keyboard input.

This example demonstrates:
- Sprite sheet animation using TextureAtlas
- Input-based conditional system execution with run_if()
- Custom component-based animation configuration
- Timer-based frame advancement
- Single<T> query for unique entities

Controls:
- Left Arrow: Animate left sprite
- Right Arrow: Animate right sprite

Note: This is a conversion of Bevy's sprite_animation.rs example.
UI elements (Text/Node) are omitted as PyBevy doesn't have UI support yet.
"""

from dataclasses import dataclass

from pybevy.prelude import *


@component(storage="python")
@dataclass
class AnimationConfig(Component):
    """Configuration for sprite sheet animation.

    Attributes:
        first_sprite_index: Index of the first frame in the animation
        last_sprite_index: Index of the last frame in the animation
        fps: Frames per second for the animation
        frame_timer: Timer to control frame advancement
    """
    first_sprite_index: int
    last_sprite_index: int
    fps: int
    frame_timer: Timer

    @staticmethod
    def new(first: int, last: int, fps: int) -> "AnimationConfig":
        """Create a new animation configuration.

        Args:
            first: Index of the first frame
            last: Index of the last frame
            fps: Animation speed in frames per second

        Returns:
            New AnimationConfig instance
        """
        return AnimationConfig(
            first_sprite_index=first,
            last_sprite_index=last,
            fps=fps,
            frame_timer=AnimationConfig.timer_from_fps(fps)
        )

    @staticmethod
    def timer_from_fps(fps: int) -> Timer:
        """Create a timer based on FPS.

        Args:
            fps: Frames per second

        Returns:
            Timer that fires at the specified FPS
        """
        return Timer(1.0 / float(fps), TimerMode.Once)


@component
@dataclass
class LeftSprite(Component):
    """Marker component for the left sprite."""


@component
@dataclass
class RightSprite(Component):
    """Marker component for the right sprite."""


def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    texture_atlas_layouts: ResMut[Assets[TextureAtlasLayout]],
) -> None:
    """Set up the scene with camera and animated sprites."""
    # Spawn camera
    commands.spawn(Camera2d())

    # Load the sprite sheet texture
    texture: Handle[Image] = asset_server.load_image("bevy/textures/rpg/chars/gabe/gabe-idle-run.png")

    # Create texture atlas layout for the sprite sheet
    # The sprite sheet has 7 sprites arranged in a row, each 24px x 24px
    layout = TextureAtlasLayout.from_grid(
        tile_size=UVec2(24, 24),
        columns=7,
        rows=1,
        padding=None,
        offset=None,
    )
    texture_atlas_layout: Handle[TextureAtlasLayout] = texture_atlas_layouts.add(layout)

    # Create the first (left-hand) sprite running at 10 FPS
    animation_config_1 = AnimationConfig.new(1, 6, 10)
    commands.spawn(
        Sprite(
            image=texture,
            texture_atlas=TextureAtlas(
                layout=texture_atlas_layout,
                index=animation_config_1.first_sprite_index,
            ),
        ),
        Transform.from_scale(Vec3(6.0, 6.0, 6.0)).with_translation(Vec3(-70.0, 0.0, 0.0)),
        LeftSprite(),
        animation_config_1,
    )

    # Create the second (right-hand) sprite running at 20 FPS
    animation_config_2 = AnimationConfig.new(1, 6, 20)
    commands.spawn(
        Sprite(
            image=texture,
            texture_atlas=TextureAtlas(
                layout=texture_atlas_layout,
                index=animation_config_2.first_sprite_index,
            ),
        ),
        Transform.from_scale(Vec3(6.0, 6.0, 6.0)).with_translation(Vec3(70.0, 0.0, 0.0)),
        RightSprite(),
        animation_config_2,
    )


def execute_animations(
    time: Res[Time],
    query: Query[tuple[Mut[AnimationConfig], Mut[Sprite]]],
) -> None:
    """Advance sprite animations based on configured FPS.

    This system loops through all sprites with AnimationConfig,
    advancing frames when the frame timer completes.
    """
    for config, sprite in query:
        # Track how long the current sprite has been displayed
        config.frame_timer.tick(time.delta_secs())

        # If frame timer finished and sprite has a texture atlas
        if config.frame_timer.just_finished() and sprite.texture_atlas is not None:
            atlas = sprite.texture_atlas

            if atlas.index == config.last_sprite_index:
                # Last frame - loop back to first frame
                atlas.index = config.first_sprite_index
            else:
                # Not last frame - advance to next frame
                atlas.index += 1

            # Reset timer for next frame (needed for both cases)
            config.frame_timer = AnimationConfig.timer_from_fps(config.fps)


def trigger_animation_left(animation: Single[Mut[AnimationConfig], With[LeftSprite]]) -> None:
    """Trigger animation for the left sprite (called when left arrow is pressed)."""
    # Single is iterable - unpack to access the component
    for config in animation:
        config.frame_timer = AnimationConfig.timer_from_fps(config.fps)


def trigger_animation_right(animation: Single[Mut[AnimationConfig], With[RightSprite]]) -> None:
    """Trigger animation for the right sprite (called when right arrow is pressed)."""
    # Single is iterable - unpack to access the component
    for config in animation:
        config.frame_timer = AnimationConfig.timer_from_fps(config.fps)


# Condition helper functions (module-level for proper type annotations)
def check_left_arrow(input_state: Res[ButtonInput]) -> bool:
    """Check if left arrow key was just pressed."""
    return input_state.just_pressed(KeyCode.ArrowLeft)


def check_right_arrow(input_state: Res[ButtonInput]) -> bool:
    """Check if right arrow key was just pressed."""
    return input_state.just_pressed(KeyCode.ArrowRight)


@entrypoint
def main(app: App) -> App:
    """Entry point for the sprite animation example."""
    return (
        app
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, execute_animations)
        # Press left arrow key to animate the left sprite
        .add_systems(Update, run_if(trigger_animation_left, check_left_arrow))
        # Press right arrow key to animate the right sprite
        .add_systems(Update, run_if(trigger_animation_right, check_right_arrow))
    )


if __name__ == "__main__":
    main().run()
