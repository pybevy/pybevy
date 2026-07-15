# Sprites Guide

Loading images, sprite sheets, frame animation, tiling, and procedural sprite generation.

## Basic Sprite

Load an image and display it:

```python
def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    commands.spawn(Camera2d())
    image = asset_server.load_image("bevy/branding/bevy_bird_dark.png")
    commands.spawn(Sprite.from_image(image))
```

Solid color rectangle (no image file needed):

```python
commands.spawn(
    Sprite.from_color(Color.srgb(1.0, 0.0, 0.0), Vec2(100.0, 50.0)),
    Transform.from_xyz(0.0, 0.0, 0.0),
)
```

## Sprite Properties

```python
sprite = Sprite(
    image=image_handle,
    flip_x=True,          # Mirror horizontally
    flip_y=False,         # Mirror vertically
    custom_size=(64, 64), # Override image dimensions (width, height)
    color=Color.srgb(1.0, 0.5, 0.5),  # Tint color
)
```

## Sprite Sheet Animation

The full pipeline: load image -> define grid layout -> create atlas -> spawn sprite -> animate index.

### 1. Setup: Layout + Atlas + Sprite

```python
from pybevy.image import TextureAtlas, TextureAtlasLayout

def setup(
    commands: Commands,
    asset_server: Res[AssetServer],
    layouts: ResMut[Assets[TextureAtlasLayout]],
) -> None:
    commands.spawn(Camera2d())

    # Load the sprite sheet image
    texture = asset_server.load_image("textures/rpg/chars/gabe/gabe-idle-run.png")

    # Define the grid: 7 frames in a row, each 24x24 pixels
    layout = TextureAtlasLayout.from_grid(
        tile_size=UVec2(24, 24),
        columns=7,
        rows=1,
        padding=None,   # optional: UVec2 spacing between tiles
        offset=None,    # optional: UVec2 offset from top-left
    )
    layout_handle = layouts.add(layout)

    # Create atlas pointing at frame 1
    atlas = TextureAtlas(layout=layout_handle, index=1)

    # Spawn the sprite (scaled 6x for visibility)
    commands.spawn(
        Sprite.from_atlas_image(texture, atlas),
        Transform.from_scale(Vec3.splat(6.0)),
        AnimationIndices(first=1, last=6),
        AnimationTimer(timer=Timer(0.1, TimerMode.REPEATING)),
    )
```

### 2. Animation Components

```python
@component
@dataclass
class AnimationIndices(Component):
    first: int
    last: int

@component(storage="python")
@dataclass
class AnimationTimer(Component):
    timer: Timer
```

**Note:** `Timer` is not a primitive type, so the component needs `storage="python"`.

### 3. Animation System

```python
def animate_sprite(
    time: Res[Time],
    query: Query[tuple[AnimationIndices, Mut[AnimationTimer], Mut[Sprite]]],
) -> None:
    for indices, timer_comp, sprite in query:
        timer_comp.timer.tick(time.delta_secs())

        if timer_comp.timer.just_finished():
            atlas = sprite.texture_atlas
            if atlas is not None:
                if atlas.index == indices.last:
                    atlas.index = indices.first
                else:
                    atlas.index += 1
```

Register it:

```python
app.add_systems(Update, animate_sprite)
```

### Minimal Alternative (no Timer)

If you don't need `Timer` features (pause, speed, just_finished), a simpler float-based approach avoids `storage="python"`:

```python
@component
@dataclass
class SpriteAnimation(Component):
    frame_count: int = 1
    fps: float = 8.0
    timer: float = 0.0

def animate_sprites(
    query: Query[tuple[Mut[Sprite], Mut[SpriteAnimation]]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    for sprite, anim in query:
        anim.timer += dt
        if anim.timer >= 1.0 / anim.fps:
            anim.timer -= 1.0 / anim.fps
            atlas = sprite.texture_atlas
            if atlas is not None:
                atlas.index = (atlas.index + 1) % anim.frame_count
```

## Tiled Sprites

Repeat a texture across a resizable area:

```python
from pybevy.sprite import SpriteImageMode

sprite = Sprite(image=image_handle)
sprite.image_mode = SpriteImageMode.tiled(
    tile_x=True,
    tile_y=True,
    stretch_value=0.5,  # tile every 128px at default image size
)
commands.spawn(sprite)

# Resize dynamically in an Update system:
sprite.custom_size = (new_width, new_height)
```

## Procedural Sprite Sheets

Generate sprite sheets from code using the `Image` API - no image files needed.

```python
from pybevy.image import TextureAtlas, TextureAtlasLayout
from pybevy.wgpu import Extent3d, ImageSampler

def make_sprite_sheet(images: ResMut[Assets[Image]]) -> int:
    frames = 4
    size = 16
    img = Image(Extent3d(size * frames, size, 1))

    # Clear to transparent
    for y in range(size):
        for x in range(size * frames):
            img.set_color_at(x, y, Color.srgba(0.0, 0.0, 0.0, 0.0))

    # Draw each frame
    for f in range(frames):
        ox = f * size  # x offset for this frame
        for y in range(size):
            for x in range(size):
                # Your pixel art logic here
                img.set_color_at(ox + x, y, some_color)

    # Nearest-neighbor filtering for crisp pixel art
    img.sampler = ImageSampler.nearest()
    return images.add(img)
```

Then use it like any other sprite sheet:

```python
def setup(
    commands: Commands,
    images: ResMut[Assets[Image]],
    layouts: ResMut[Assets[TextureAtlasLayout]],
) -> None:
    sheet = make_sprite_sheet(images)
    layout = TextureAtlasLayout.from_grid(UVec2(16, 16), columns=4, rows=1)
    layout_handle = layouts.add(layout)
    atlas = TextureAtlas(layout_handle, 0)

    commands.spawn(
        Sprite.from_atlas_image(sheet, atlas),
        Transform.from_scale(Vec3.splat(4.0)),  # scale up pixel art
        SpriteAnimation(frame_count=4, fps=8.0),
    )
```

## Imports

Most sprite types are in the prelude. The key ones that aren't:

```python
from pybevy.image import TextureAtlas, TextureAtlasLayout
from pybevy.wgpu import Extent3d, ImageSampler  # for procedural images
from pybevy.sprite import SpriteImageMode       # for tiling
```

**Common gotcha:** `pybevy.image` has its own `ImageSampler` class, but `Image.sampler` setter expects the one from `pybevy.wgpu`. Always import `ImageSampler` from `pybevy.wgpu`.
