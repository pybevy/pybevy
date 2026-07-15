# Platformer Recipe - Gravity, Jumping & Collectibles

Side-scrolling platformer with keyboard movement, gravity/jump physics, camera follow, collectible pickups, and a HUD score counter. Good for action games, endless runners, or any scene with continuous physics-style movement.

## Core Pattern

A `PlayerState` component tracks vertical velocity and ground state. An Update system applies gravity, handles jumping, and clamps to the ground plane.

```python
import math
import random
from dataclasses import dataclass
from pybevy.prelude import *
from pybevy.input import ButtonInput, KeyCode
from pybevy.audio import PlaybackMode

PLAYER_SPEED = 6.0
JUMP_FORCE = 8.0
GRAVITY = -20.0
GROUND_Y = 0.0

# Components

@component
class Player(Component):
    pass

@component
@dataclass
class PlayerState(Component):
    vel_y: float = 0.0
    on_ground: float = 1.0       # 1.0 = grounded, 0.0 = airborne
    facing_right: float = 1.0

@component
class Collectible(Component):
    pass

@component
@dataclass
class CollectibleSpin(Component):
    offset: float = 0.0

@component
class MainCamera(Component):
    pass

@component
class ScoreDisplay(Component):
    pass

@resource
@dataclass
class GameScore(Resource):
    gems: int = 0
    total: int = 0
```

## Player Movement & Jump System

Reads keyboard input, applies horizontal movement, gravity, and jump impulse. Reference `guide://input` for full input API details.

```python
def move_player(
    query: Query[tuple[Mut[Transform], Mut[PlayerState]], With[Player]],
    keyboard: Res[ButtonInput],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    for transform, state in query:
        # Horizontal movement
        dx = 0.0
        if keyboard.pressed(KeyCode.ArrowLeft) or keyboard.pressed(KeyCode.KeyA):
            dx -= PLAYER_SPEED * dt
        if keyboard.pressed(KeyCode.ArrowRight) or keyboard.pressed(KeyCode.KeyD):
            dx += PLAYER_SPEED * dt

        # Track facing direction for sprite flip
        if dx > 0.001:
            state.facing_right = 1.0
        elif dx < -0.001:
            state.facing_right = -1.0

        # Jump (only when grounded)
        if keyboard.just_pressed(KeyCode.Space) and state.on_ground > 0.5:
            state.vel_y = JUMP_FORCE
            state.on_ground = 0.0

        # Gravity
        state.vel_y += GRAVITY * dt
        transform.translation.y += state.vel_y * dt

        # Ground collision (simple Y-floor)
        if transform.translation.y <= GROUND_Y:
            transform.translation.y = GROUND_Y
            state.vel_y = 0.0
            state.on_ground = 1.0

        transform.translation.x += dx

        # Flip character via scale
        transform.scale = Vec3(state.facing_right, 1.0, 1.0)
```

**Key points:**
- `just_pressed` for jump (one-shot), `pressed` for movement (continuous)
- Ground collision is a simple Y-floor check - for platforms at different heights, compare against each platform's Y + height
- `on_ground` uses float (1.0/0.0) for compatibility with View batch expressions, but `bool` fields also work with `@component`

## Camera Follow System

Smooth lerp camera that tracks the player horizontally with slight vertical offset for jumps:

```python
CAM_LERP = 3.0

def follow_camera(
    cam_q: Query[Mut[Transform], tuple[With[MainCamera], Without[Player]]],
    player_q: Query[Transform, tuple[With[Player], With[PlayerState]]],
    time: Res[Time],
) -> None:
    dt = time.delta_secs()
    for ptf in player_q:
        for cam_tf in cam_q:
            target_x = ptf.translation.x
            target_y = 3.0 + ptf.translation.y * 0.3  # slight Y follow
            lerp = min(1.0, CAM_LERP * dt)
            cam_tf.translation.x += (target_x - cam_tf.translation.x) * lerp
            cam_tf.translation.y += (target_y - cam_tf.translation.y) * lerp
```

**Note:** Uses `Without[Player]` filter to avoid query conflict with the player movement system that also reads `Transform`.

## Collectible Pickup (Distance-Based)

Spinning collectibles that despawn when the player gets close:

```python
def spin_collectibles(
    query: Query[tuple[Mut[Transform], CollectibleSpin], With[Collectible]],
    time: Res[Time],
) -> None:
    t = time.elapsed_secs()
    for transform, spin in query:
        transform.rotation = Quat.from_euler(
            EulerRot.XYZ, 0.0, t * 2.5 + spin.offset,
            math.sin(t * 1.5 + spin.offset) * 0.3,
        )
        transform.translation.y = 1.5 + math.sin(t * 2.0 + spin.offset) * 0.25


def collect_gems(
    commands: Commands,
    player_q: Query[Transform, tuple[With[Player], With[PlayerState]]],
    gem_q: Query[tuple[Entity, Transform], With[Collectible]],
    score: ResMut[GameScore],
) -> None:
    for ptf in player_q:
        px = ptf.translation.x
        py = ptf.translation.y + 1.0  # offset to player center
        for entity, gtf in gem_q:
            dist_sq = (px - gtf.translation.x) ** 2 + (py - gtf.translation.y) ** 2
            if dist_sq < 1.0:  # pickup radius
                commands.entity(entity).despawn()
                score.gems += 1
```

## HUD Score Counter

Uses UI `Text` (not `Text2d` - `Text2d` requires `Camera2d`). See `guide://ui-text` for full UI text details.

```python
from pybevy.ui import Node, Text, BackgroundColor
from pybevy.text import TextFont, TextColor

def setup_hud(commands: Commands, score: Res[GameScore]) -> None:
    node = Node()
    node.position_type = 1  # Absolute
    node.top = 12.0
    node.right = 16.0
    commands.spawn(
        Text(f"GEMS: 0 / {score.total}"),
        node,
        TextFont.from_font_size(22.0),
        TextColor(Color.srgb(0.1, 1.0, 0.8)),
        BackgroundColor(Color.srgba(0.0, 0.0, 0.0, 0.5)),
        ScoreDisplay(),
        Name("score_hud"),
    )


def update_hud(
    query: Query[Mut[Text], With[ScoreDisplay]],
    score: Res[GameScore],
) -> None:
    for text in query:
        text.content = f"GEMS: {score.gems} / {score.total}"
```

## Setup & Registration

```python
PLAYER_Z = 6.5  # Z position for side-scrolling plane

def setup(
    commands: Commands,
    meshes: ResMut[Assets[Mesh]],
    materials: ResMut[Assets[StandardMaterial]],
) -> None:
    # Camera
    commands.spawn(
        Camera3d(),
        Transform.from_xyz(0.0, 3.0, 18.0).looking_at(Vec3(0.0, 4.0, 0.0), Vec3.Y),
        Bloom(intensity=0.2),
        MainCamera(),
        Name("camera"),
    )

    # Ground
    ground_mesh = meshes.add(Cuboid(40.0, 0.2, 30.0))
    ground_mat = materials.add(StandardMaterial(base_color=Color.srgb(0.1, 0.1, 0.15)))
    commands.spawn(Mesh3d(ground_mesh), MeshMaterial3d(ground_mat), Transform.from_xyz(0.0, -0.1, 0.0))

    # Player (simple box character)
    body_mesh = meshes.add(Cuboid(0.5, 0.7, 0.3))
    body_mat = materials.add(StandardMaterial(base_color=Color.srgb(0.2, 0.5, 0.9)))
    commands.spawn(
        Mesh3d(body_mesh), MeshMaterial3d(body_mat),
        Transform.from_xyz(0.0, 0.0, PLAYER_Z),
        Player(), PlayerState(),
        Name("player"),
    )

    # Collectibles
    gem_mesh = meshes.add(Cuboid(0.3, 0.3, 0.3))
    gem_mat = materials.add(StandardMaterial(
        base_color=Color.srgb(1.0, 0.8, 0.1),
        emissive=LinearRgba.rgb(8.0, 6.0, 0.5), unlit=True,
    ))
    gem_positions = [-8.0, -4.0, 0.0, 4.0, 8.0]
    for i, gx in enumerate(gem_positions):
        commands.spawn(
            Mesh3d(gem_mesh), MeshMaterial3d(gem_mat),
            Transform.from_xyz(gx, 1.5, PLAYER_Z),
            Collectible(), CollectibleSpin(offset=float(i) * 1.2),
            Name(f"gem_{i}"),
        )

    commands.insert_resource(GameScore(gems=0, total=len(gem_positions)))


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup, setup_hud))
        .add_systems(Update, (
            move_player,
            follow_camera,
            spin_collectibles,
            collect_gems,
            update_hud,
        ))
    )

if __name__ == "__main__":
    main().run()
```

## Adding Sound Effects

For audio on jump/collect events, see `guide://audio`. The pattern for fire-and-forget SFX:

```python
# In a system that detects the event:
commands.spawn(
    AudioPlayer(sfx_handle),
    PlaybackSettings(mode=PlaybackMode.Despawn, volume=Volume.Linear(0.3)),
)
```

`PlaybackMode.Despawn` auto-removes the entity after playback. Import with `from pybevy.audio import PlaybackMode`.

## Extending

- **Multiple platforms:** Store platform Y-heights in a list or resource; check player position against each in the ground collision section
- **Enemies:** Add an `Enemy` component with patrol logic (see `guide://recipes/game-logic` for AI patterns)
- **Procedural audio:** Generate WAV files at startup with Python `wave`/`struct` modules (see `guide://audio`)
- **Particle effects:** Spawn small emissive cubes with velocity/lifetime components on collect events
