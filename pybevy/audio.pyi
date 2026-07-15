from datetime import timedelta
from typing import ClassVar, Literal

import numpy as np

from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.ecs import Batchable, Component, Resource
from pybevy.math import Vec3
from pybevy.transform import Transform

class Volume:
    """Audio volume representation.

    Can be specified as linear (0.0 to 1.0+) or decibels.

    Examples:
        Volume.Linear(0.5)      # 50% volume
        Volume.Decibels(-6.0)   # -6 dB
        Volume.SILENT           # Silent (0 volume)
    """

    class Linear(Volume):
        """Volume represented as a linear amplitude."""
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float, /) -> None: ...

    class Decibels(Volume):
        """Volume represented in decibels."""
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: float
        def __init__(self, value: float, /) -> None: ...

    SILENT: ClassVar[Volume]
    """Create a silent (zero) volume."""

    def to_linear(self) -> float:
        """Get volume as linear value."""

    def to_decibels(self) -> float:
        """Get volume as decibels value."""

    def increase_by_percentage(self, percentage: float) -> Volume:
        """Increase volume by percentage (e.g., 0.1 for 10% increase)."""

    def decrease_by_percentage(self, percentage: float) -> Volume:
        """Decrease volume by percentage (e.g., 0.1 for 10% decrease)."""

    def fade_towards(self, target: Volume, factor: float) -> Volume:
        """Fade towards target volume by factor (0.0 to 1.0)."""

    def scale_to_factor(self, factor: float) -> Volume:
        """Scale volume by factor."""

    def __mul__(self, other: Volume) -> Volume: ...
    def __truediv__(self, other: Volume) -> Volume: ...
    def __eq__(self, other: object) -> bool: ...

class PlaybackMode:
    """Controls what happens when audio finishes playing."""

    Once: PlaybackMode
    """Play once and stop."""

    Loop: PlaybackMode
    """Loop forever."""

    Despawn: PlaybackMode
    """Despawn the entity when audio finishes."""

    Remove: PlaybackMode
    """Remove AudioPlayer component when audio finishes."""

class PlaybackSettings(Component):
    """Initial settings to be used when audio starts playing.

    If you would like to control the audio while it is playing, query for
    AudioSink or SpatialAudioSink components. Changes to this component
    will NOT be applied to already-playing audio.
    """

    ONCE: ClassVar[PlaybackSettings]
    """Play once and stop."""

    LOOP: ClassVar[PlaybackSettings]
    """Play in a loop forever."""

    DESPAWN: ClassVar[PlaybackSettings]
    """Play once and despawn the entity."""

    REMOVE: ClassVar[PlaybackSettings]
    """Play once and remove the audio components."""

    def __init__(
        self,
        *,
        mode: PlaybackMode | None = None,
        volume: Volume | None = None,
        speed: float = 1.0,
        paused: bool = False,
        muted: bool = False,
        spatial: bool = False,
        start_position: timedelta | None = None,
        duration: timedelta | None = None,
        spatial_scale: SpatialScale | None = None,
    ) -> None:
        """Create playback settings with optional customization.

        Args:
            mode: Playback behavior (default: Once)
            volume: Volume level
            speed: Playback speed multiplier
            paused: Start paused
            muted: Start muted
            spatial: Enable spatial audio
            start_position: Position to start playback from
            duration: Limit playback duration
            spatial_scale: Custom spatial scale
        """

    @property
    def mode(self) -> PlaybackMode:
        """The desired playback behavior."""

    @mode.setter
    def mode(self, value: PlaybackMode) -> None: ...

    @property
    def volume(self) -> Volume:
        """Volume to play at."""

    @volume.setter
    def volume(self, value: Volume) -> None: ...

    @property
    def speed(self) -> float:
        """Speed to play at (1.0 is normal speed)."""

    @speed.setter
    def speed(self, value: float) -> None: ...

    @property
    def paused(self) -> bool:
        """Create the sink in paused state.

        Useful for deferred playback, if you want to prepare
        the entity but hear the sound later.
        """

    @paused.setter
    def paused(self, value: bool) -> None: ...

    @property
    def muted(self) -> bool:
        """Whether to create the sink in muted state or not.

        This is useful for audio that should be initially muted. You can still
        set the initial volume and it is applied when the audio is unmuted.
        """

    @muted.setter
    def muted(self, value: bool) -> None: ...

    @property
    def spatial(self) -> bool:
        """Enables spatial audio for this source.

        Note: Bevy does not currently support HRTF or any other high-quality
        3D sound rendering features. Spatial audio is implemented via simple
        left-right stereo panning.
        """

    @spatial.setter
    def spatial(self, value: bool) -> None: ...

    @property
    def start_position(self) -> timedelta | None:
        """The point in time in the audio clip where playback should start.

        If set to None, it will play from the beginning of the clip.
        If the playback mode is set to Loop, each loop will start from this position.
        """

    @start_position.setter
    def start_position(self, value: timedelta | None) -> None: ...

    @property
    def duration(self) -> timedelta | None:
        """How long the audio should play before stopping.

        If set, the clip will play for at most the specified duration.
        If set to None, it will play for as long as it can.
        If the playback mode is set to Loop, each loop will last for this duration.
        """

    @duration.setter
    def duration(self, value: timedelta | None) -> None: ...

    def with_volume(self, volume: Volume) -> PlaybackSettings:
        """Helper to set the volume from start of playback."""

    def with_speed(self, speed: float) -> PlaybackSettings:
        """Helper to set the speed from start of playback."""

    def with_spatial(self, spatial: bool) -> PlaybackSettings:
        """Helper to enable or disable spatial audio."""

    def with_start_position(self, start_position: timedelta) -> PlaybackSettings:
        """Helper to use a custom playback start position."""

    def with_duration(self, duration: timedelta) -> PlaybackSettings:
        """Helper to use a custom playback duration."""

    def with_spatial_scale(self, spatial_scale: SpatialScale) -> PlaybackSettings:
        """Helper to use a custom spatial scale."""

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        speed: np.typing.ArrayLike | None = None,
        paused: np.typing.ArrayLike | None = None,
        muted: np.typing.ArrayLike | None = None,
        spatial: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

    @property
    def spatial_scale(self) -> SpatialScale | None:
        """Optional scale factor for spatial positioning.

        Overrides the default value configured on AudioPlugin::default_spatial_scale.
        """

    @spatial_scale.setter
    def spatial_scale(self, value: SpatialScale | None) -> None: ...

class SpatialScale:
    def __init__(self, scale: float | Vec3 = 1.0) -> None: ...

    @staticmethod
    def new_2d(scale: float) -> SpatialScale: ...

    def to_vec3(self) -> Vec3: ...

class SpatialListener(Component):
    """Marks an entity as a spatial audio listener.

    Spatial audio is implemented via simple left-right stereo panning.
    Each listener has a left and right "ear" offset from the entity's position.
    """

    def __init__(
        self,
        gap: float = 4.0,
        *,
        left_ear_offset: Vec3 | None = None,
        right_ear_offset: Vec3 | None = None,
    ) -> None:
        """Create a new SpatialListener.

        Args:
            gap: Distance between ears (creates default offsets at +/- gap/2 on X axis)
            left_ear_offset: Override left ear position
            right_ear_offset: Override right ear position
        """

    @property
    def left_ear_offset(self) -> Vec3:
        """Position offset of the left ear from the entity's transform."""

    @left_ear_offset.setter
    def left_ear_offset(self, value: Vec3) -> None: ...

    @property
    def right_ear_offset(self) -> Vec3:
        """Position offset of the right ear from the entity's transform."""

    @right_ear_offset.setter
    def right_ear_offset(self, value: Vec3) -> None: ...

class AudioSource(Asset):
    """Audio data asset loaded from file.

    Load via asset_server.load_audio("path/to/audio.ogg").
    Supports wav, ogg, flac, and mp3 formats.
    """

    @property
    def bytes(self) -> bytes:
        """Raw audio data bytes."""

class Pitch(Asset):
    """Procedural audio asset that generates a sine wave at a specific frequency.

    This asset can be used to generate simple tones programmatically.
    """

    def __init__(self, frequency: float, duration: float) -> None:
        """Create a new Pitch asset.

        Args:
            frequency: Frequency in Hz (e.g., 440.0 for A4 note)
            duration: Duration in seconds
        """

    @property
    def frequency(self) -> float:
        """Get the frequency in Hz."""

    @frequency.setter
    def frequency(self, value: float) -> None:
        """Set the frequency in Hz."""

    @property
    def duration(self) -> float:
        """Get the duration in seconds."""

    @duration.setter
    def duration(self, value: float) -> None:
        """Set the duration in seconds."""

class AudioPlayer(Component):
    """Component that plays audio from a source handle.

    Add this to an entity with a Handle<AudioSource> to play audio.
    """

    def __init__(self, source: Handle) -> None: ...

    @property
    def source(self) -> Handle:
        """Get the audio source handle."""

    @source.setter
    def source(self, value: Handle) -> None:
        """Set the audio source handle."""

class AudioSink(Component):
    """Component for controlling audio playback.

    This component is automatically added by Bevy when AudioPlayer
    starts playing. Use it to control playback (pause, volume, etc.).
    """

    def play(self) -> None:
        """Resume playback."""

    def pause(self) -> None:
        """Pause playback."""

    def stop(self) -> None:
        """Stop playback and remove sink."""

    def volume(self) -> Volume:
        """Get current volume."""

    def set_volume(self, volume: Volume) -> None:
        """Set playback volume."""

    def speed(self) -> float:
        """Get playback speed (1.0 is normal)."""

    def set_speed(self, speed: float) -> None:
        """Set playback speed (1.0 is normal, 2.0 is double speed, etc.)."""

    def mute(self) -> None:
        """Mute audio (preserves volume setting)."""

    def unmute(self) -> None:
        """Unmute audio."""

    def is_muted(self) -> bool:
        """Check if audio is muted."""

    def is_paused(self) -> bool:
        """Check if audio is paused."""

    def empty(self) -> bool:
        """Check if playback queue is empty."""

    def position(self) -> timedelta:
        """Get current playback position."""

    def try_seek(self, pos: timedelta) -> None:
        """Seek to position in the audio stream.

        Note: Seeking is not supported on file-based audio loaded via
        ``asset_server.load_audio()`` due to a rodio limitation (the decoder
        does not support ``Seekable``). This method will raise an error in that
        case. Seeking works with procedural sources such as ``Pitch``.
        """

    def toggle_playback(self) -> None:
        """Toggle between play and pause state.

        If paused, resumes playback. If playing, pauses playback.
        """

    def toggle_mute(self) -> None:
        """Toggle the mute state.

        If muted, unmutes and restores volume. If not muted, mutes.
        """

class SpatialAudioSink(Component):
    """Component for controlling spatial audio playback.

    This component is automatically added by Bevy when AudioPlayer with spatial=true
    starts playing. Use it to control playback and set spatial positions.
    """

    def play(self) -> None:
        """Resume playback."""

    def pause(self) -> None:
        """Pause playback."""

    def stop(self) -> None:
        """Stop playback and remove sink."""

    def volume(self) -> Volume:
        """Get current volume."""

    def set_volume(self, volume: Volume) -> None:
        """Set playback volume."""

    def speed(self) -> float:
        """Get playback speed (1.0 is normal)."""

    def set_speed(self, speed: float) -> None:
        """Set playback speed (1.0 is normal, 2.0 is double speed, etc.)."""

    def mute(self) -> None:
        """Mute audio (preserves volume setting)."""

    def unmute(self) -> None:
        """Unmute audio."""

    def is_muted(self) -> bool:
        """Check if audio is muted."""

    def is_paused(self) -> bool:
        """Check if audio is paused."""

    def empty(self) -> bool:
        """Check if playback queue is empty."""

    def position(self) -> timedelta:
        """Get current playback position."""

    def try_seek(self, pos: timedelta) -> None:
        """Seek to position in the audio stream.

        Note: Seeking is not supported on file-based audio loaded via
        ``asset_server.load_audio()`` due to a rodio limitation (the decoder
        does not support ``Seekable``). This method will raise an error in that
        case. Seeking works with procedural sources such as ``Pitch``.
        """

    def toggle_playback(self) -> None:
        """Toggle between play and pause state.

        If paused, resumes playback. If playing, pauses playback.
        """

    def toggle_mute(self) -> None:
        """Toggle the mute state.

        If muted, unmutes and restores volume. If not muted, mutes.
        """

    def set_ears_position(self, left_position: Vec3, right_position: Vec3) -> None:
        """Set the two ears position for spatial audio."""

    def set_listener_position(self, position: Transform, gap: float) -> None:
        """Set the listener position, with an ear on each side separated by gap."""

    def set_emitter_position(self, position: Vec3) -> None:
        """Set the emitter (audio source) position."""

class GlobalVolume(Resource):
    """Global volume control for all audio.

    This resource affects all audio playback in the app.
    """

    def __init__(self, volume: Volume | None = None) -> None: ...

    @property
    def volume(self) -> Volume:
        """Get global volume."""

    @volume.setter
    def volume(self, value: Volume) -> None:
        """Set global volume."""

class DefaultSpatialScale(Resource):
    """Default spatial scale applied to all audio sources.

    This can be overridden per-source via PlaybackSettings.spatial_scale.
    """

    def __init__(self, scale: SpatialScale | None = None) -> None: ...

    @property
    def scale(self) -> SpatialScale:
        """Get the default spatial scale."""

    @scale.setter
    def scale(self, value: SpatialScale) -> None:
        """Set the default spatial scale."""

class AudioPlugin(Plugin):
    """Plugin that adds audio support to the app.

    This is included in DefaultPlugins.
    """

    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...
