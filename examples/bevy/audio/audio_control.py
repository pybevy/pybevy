"""Audio playback control example.

Demonstrates how to control audio playback including:
- Volume control (up/down arrow keys)
- Pause/resume (space key)
- Mute/unmute (M key)
- Dynamic speed adjustment (automatic sine wave modulation)
- Playback position tracking
"""

import math

from pybevy.prelude import *


@component
class MyMusic(Component):
    """Marker component for the music entity."""



@component
class ProgressText(Component):
    """Marker component for the progress text entity."""



def setup(commands: Commands, asset_server: Res[AssetServer]) -> None:
    """Set up the audio player and UI."""
    # Load and play music
    audio_source = asset_server.load_audio("bevy/sounds/Windless Slopes.ogg")
    commands.spawn(AudioPlayer(audio_source), MyMusic())

    # Progress text (top-left)
    commands.spawn(
        Text2d("Progress: 0.0s"),
        TextFont.from_font_size(24.0),
        TextColor.WHITE,
        Transform.from_xyz(-300.0, 200.0, 0.0),
        ProgressText(),
    )

    # Instructions text (bottom-left)
    commands.spawn(
        Text2d("Arrow Up/Down: Volume\nSpace: Toggle Playback\nM: Toggle Mute"),
        TextFont.from_font_size(20.0),
        TextColor(Color.srgb(0.8, 0.8, 0.8)),
        Transform.from_xyz(-300.0, -200.0, 0.0),
    )

    # Camera
    commands.spawn(Camera2d())


def update_progress_text(
    music_controller: Single[AudioSink, With[MyMusic]],
    progress_text: Query[Mut[Text2d], With[ProgressText]],
) -> None:
    """Update the progress text with current playback position."""
    for sink in music_controller:
        position = sink.position().total_seconds()
        for text in progress_text:
            text.text = f"Progress: {position:.1f}s"


def update_speed(
    music_controller: Query[AudioSink, With[MyMusic]], time: Res[Time]
) -> None:
    """Modulate playback speed with a sine wave (0.1x to 1.9x)."""
    for sink in music_controller:
        if not sink.is_paused():  # type: ignore[union-attr]
            # Sine wave from -1 to 1, shifted to 0 to 2, clamped to min 0.1
            speed = max(0.1, math.sin(time.elapsed_secs() / 5.0) + 1.0)
            sink.set_speed(speed)  # type: ignore[union-attr]


def pause(
    keyboard_input: Res[ButtonInput],
    music_controller: Query[AudioSink, With[MyMusic]],
) -> None:
    """Toggle playback with spacebar."""
    if keyboard_input.just_pressed(KeyCode.Space):
        for sink in music_controller:
            if sink.is_paused():  # type: ignore[union-attr]
                sink.play()  # type: ignore[union-attr]
            else:
                sink.pause()  # type: ignore[union-attr]


def mute(
    keyboard_input: Res[ButtonInput],
    music_controller: Query[AudioSink, With[MyMusic]],
) -> None:
    """Toggle mute with M key."""
    if keyboard_input.just_pressed(KeyCode.KeyM):
        for sink in music_controller:
            if sink.is_muted():  # type: ignore[union-attr]
                sink.unmute()  # type: ignore[union-attr]
            else:
                sink.mute()  # type: ignore[union-attr]


def volume(
    keyboard_input: Res[ButtonInput],
    music_controller: Query[AudioSink, With[MyMusic]],
) -> None:
    """Adjust volume with arrow up/down keys."""
    for sink in music_controller:
        if keyboard_input.just_pressed(KeyCode.ArrowUp):
            current_volume = sink.volume()  # type: ignore[union-attr]
            sink.set_volume(current_volume.increase_by_percentage(0.1))  # type: ignore[union-attr]
        elif keyboard_input.just_pressed(KeyCode.ArrowDown):
            current_volume = sink.volume()  # type: ignore[union-attr]
            sink.set_volume(current_volume.decrease_by_percentage(0.1))  # type: ignore[union-attr]


@entrypoint
def main(app: App) -> App:
    return (
        app.add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, (update_progress_text, update_speed, pause, mute, volume))
    )


if __name__ == "__main__":
    main().run()
