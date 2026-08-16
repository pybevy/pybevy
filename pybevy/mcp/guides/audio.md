# Audio Guide

Playing sounds, controlling volume, and using spatial audio in PyBevy.

## Basic Audio Playback

Spawn an entity with `AudioPlayer` to play a sound. Bevy loads audio files via the asset server.

```python
from pybevy.prelude import *

def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    # Fire-and-forget: plays once and stops
    commands.spawn(
        AudioPlayer(asset_server.load_audio("bevy/sounds/impact.ogg")),
        PlaybackSettings.ONCE,
    )
```

**Supported formats:** wav, ogg, flac

## Playback Modes

Use `PlaybackSettings` presets or construct custom settings:

```python
# Presets
PlaybackSettings.ONCE      # Play once, then stop
PlaybackSettings.LOOP      # Loop forever
PlaybackSettings.DESPAWN   # Play once, despawn entity when done
PlaybackSettings.REMOVE    # Play once, remove audio components when done

# Custom settings (PlaybackMode requires explicit import)
# from pybevy.audio import PlaybackMode
PlaybackSettings(
    mode=PlaybackMode.Loop,
    volume=Volume.Linear(0.5),
    speed=1.5,
    paused=True,           # Start paused, resume later via AudioSink
)
```

**Builder pattern:**

```python
PlaybackSettings.LOOP.with_volume(Volume.Linear(0.3)).with_speed(0.8)
```

## Volume Control

### Per-Source Volume

```python
# Linear scale (0.0 = silent, 1.0 = full, >1.0 = amplified)
Volume.Linear(0.5)

# Decibel scale
Volume.Decibels(-6.0)    # Half perceived loudness

# Silent constant
Volume.SILENT
```

**Volume utilities:**

```python
vol = Volume.Linear(0.5)
vol.to_linear()          # 0.5
vol.to_decibels()        # ~-6.02
vol.increase_by_percentage(10.0)  # +10%
vol.decrease_by_percentage(10.0)  # -10%
vol.fade_towards(Volume.Linear(1.0), 0.1)  # Smooth fade
```

### Global Volume

Affects all audio output:

```python
@entrypoint
def main(app: App) -> App:
    return (
        app
        .add_plugins(DefaultPlugins)
        .insert_resource(GlobalVolume(Volume.Linear(0.8)))
        .add_systems(Startup, setup)
    )
```

Modify at runtime:

```python
def adjust_volume(global_vol: ResMut[GlobalVolume]) -> None:
    global_vol.volume = Volume.Linear(0.5)
```

## Runtime Audio Control (AudioSink)

Bevy automatically adds `AudioSink` to entities when audio starts playing. Query it to control playback:

```python
def control_audio(query: Query[AudioSink]) -> None:
    for sink in query:
        sink.pause()
        sink.play()
        sink.stop()
        sink.toggle_playback()

        # Volume
        sink.set_volume(Volume.Linear(0.3))
        current = sink.volume()

        # Speed
        sink.set_speed(2.0)

        # Mute (preserves volume setting)
        sink.mute()
        sink.unmute()
        sink.toggle_mute()

        # Seeking (see limitation note below)
        from datetime import timedelta
        sink.try_seek(timedelta(seconds=5.0))
        pos = sink.position()

        # State
        sink.is_paused()
        sink.is_muted()
        sink.empty()
```

**Important:** `AudioSink` is engine-managed. You can query it but cannot spawn it directly. It appears automatically after `AudioPlayer` starts playing.

**Seeking:** `try_seek()` needs a non-looping source whose decoder can seek:

| Source | `PlaybackSettings.ONCE` | `PlaybackSettings.LOOP` |
|--------|-------------------------|-------------------------|
| `.wav` | seeks | fails |
| `.ogg` | fails | fails |

For seekable background music, use `.wav` with `ONCE` and restart it yourself.

## Spatial Audio

For 3D positional audio, enable `spatial=True` on playback settings and add a `SpatialListener` to the camera.

### Listener Setup

```python
def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    # Camera with spatial listener
    commands.spawn(
        Camera3d(),
        Transform.from_translation(Vec3(0.0, 0.0, 5.0)),
        SpatialListener(gap=4.0),  # Distance between left/right ears
    )

    # Spatial audio source
    commands.spawn(
        AudioPlayer(asset_server.load_audio("bevy/sounds/engine.ogg")),
        PlaybackSettings(
            mode=PlaybackMode.Loop,
            spatial=True,
        ),
        Transform.from_translation(Vec3(10.0, 0.0, 0.0)),
    )
```

### SpatialAudioSink

For spatial sources, Bevy adds `SpatialAudioSink` instead of `AudioSink`. It has all the same controls plus spatial positioning:

```python
def update_spatial(query: Query[SpatialAudioSink]) -> None:
    for sink in query:
        sink.set_emitter_position(Vec3(5.0, 0.0, 0.0))
        sink.set_listener_position(
            Transform.from_translation(Vec3(0.0, 0.0, 0.0)),
            gap=4.0,
        )
```

### Custom Ear Positioning

```python
SpatialListener(
    left_ear_offset=Vec3(-2.0, 0.0, 0.0),
    right_ear_offset=Vec3(2.0, 0.0, 0.0),
)
```

### Default Spatial Scale

Control how distance affects volume falloff globally:

```python
app.insert_resource(DefaultSpatialScale(SpatialScale(0.5)))
```

Or per-source:

```python
PlaybackSettings(spatial=True, spatial_scale=SpatialScale(2.0))
```

## Procedural Audio (Pitch): not playable yet

`Pitch` constructs and can be added to `Assets[Pitch]`, but **cannot be played**:
`AudioPlayer` accepts only `Handle[AudioSource]`, and `AudioSource` has no Python
constructor. Generate tones offline and load them as a file instead:

```python
handle = asset_server.load_audio("tones/a4.ogg")
commands.spawn(AudioPlayer(handle), PlaybackSettings.ONCE)
```

## Start Position & Duration

```python
from datetime import timedelta

PlaybackSettings(
    start_position=timedelta(seconds=10.0),   # Skip first 10s
    duration=timedelta(seconds=30.0),          # Play for 30s max
)
```

## Asset Loading

Audio files are loaded from the `assets/` directory by default:

```
my_project/
├── assets/
│   └── sounds/
│       ├── music.ogg
│       └── sfx/
│           └── click.wav
├── main.py
```

```python
asset_server.load_audio("bevy/sounds/music.ogg")
asset_server.load_audio("bevy/sounds/sfx/click.wav")
```
