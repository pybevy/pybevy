"""Displays a single Sprite tiled in a grid, with a scaling animation.

Demonstrates:
- SpriteImageMode.Tiled for repeating textures
- Sprite custom_size for dynamic sizing
- Resource-based animation state
- Time-based oscillating animation
"""

from pybevy.prelude import *


@resource
class AnimationState(Resource):
    """Resource storing animation parameters."""
    def __init__(self):
        self.min = 128.0
        self.max = 512.0
        self.current = 128.0
        self.speed = 50.0


def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    """Set up camera and tiled sprite."""
    commands.spawn(Camera2d())

    image_handle = asset_server.load_image("icon.png")

    # Create sprite with tiled image mode
    sprite = Sprite(image=image_handle)
    sprite.image_mode = SpriteImageMode.Tiled(
        tile_x=True,
        tile_y=True,
        stretch_value=0.5,  # The image will tile every 128px
    )

    commands.spawn(sprite)


def animate(
    sprites: Query[Mut[Sprite]],
    state: ResMut[AnimationState],
    time: Res[Time],
) -> None:
    """Animate sprite size with oscillating motion."""
    # Reverse direction at boundaries
    if state.current >= state.max or state.current <= state.min:
        state.speed = -state.speed

    state.current += state.speed * time.delta_secs()

    # Update all sprites
    for sprite in sprites:
        sprite.custom_size = Vec2(state.current, state.current)


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .insert_resource(AnimationState())
        .add_systems(Startup, setup)
        .add_systems(Update, animate)
    )


if __name__ == "__main__":
    main().run()
