"""Input handling for PyBevy - keyboard, mouse, and gamepad input."""

from typing import ClassVar, Final, Generic, Literal, TypeVar

from pybevy.app import App, Plugin
from pybevy.ecs import Component, Entity, Message, Resource, SystemSet
from pybevy.math import Vec2

ButtonT = TypeVar("ButtonT", "KeyCode", "MouseButton")

InputSystems: Final[SystemSet]

class InputPlugin(Plugin):
    """Plugin that provides input handling (keyboard, mouse, gamepad, touch).

    This plugin initializes input resources like AccumulatedMouseMotion.
    """
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class NativeKeyCode:
    """Platform-specific physical key identifier."""

    class Unidentified(NativeKeyCode):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Android(NativeKeyCode):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    class MacOS(NativeKeyCode):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    class Windows(NativeKeyCode):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    class Xkb(NativeKeyCode):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    def __hash__(self) -> int: ...

class NativeKey:
    """Platform-specific logical key identifier."""

    class Unidentified(NativeKey):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Android(NativeKey):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    class MacOS(NativeKey):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    class Windows(NativeKey):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    class Xkb(NativeKey):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    class Web(NativeKey):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    def __hash__(self) -> int: ...

class Key:
    """Logical meaning of a keyboard input."""

    class Character(Key):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str
        def __init__(self, value: str) -> None: ...

    class Unidentified(Key):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: NativeKey
        def __init__(self, value: NativeKey) -> None: ...

    class Dead(Key):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: str | None
        def __init__(self, value: str | None) -> None: ...

    class Alt(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AltGraph(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CapsLock(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Control(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Fn(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FnLock(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class NumLock(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ScrollLock(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Shift(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Symbol(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SymbolLock(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Meta(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Hyper(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Super(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Enter(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Tab(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Space(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ArrowDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ArrowLeft(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ArrowRight(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ArrowUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class End(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Home(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PageDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PageUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Backspace(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Clear(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Copy(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CrSel(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Cut(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Delete(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class EraseEof(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ExSel(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Insert(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Paste(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Redo(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Undo(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Accept(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Again(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Attn(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Cancel(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ContextMenu(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Escape(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Execute(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Find(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Help(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Pause(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Play(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Props(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Select(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ZoomIn(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ZoomOut(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BrightnessDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BrightnessUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Eject(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LogOff(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Power(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PowerOff(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PrintScreen(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Hibernate(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Standby(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class WakeUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AllCandidates(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Alphanumeric(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CodeInput(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Compose(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Convert(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FinalMode(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class GroupFirst(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class GroupLast(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class GroupNext(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class GroupPrevious(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ModeChange(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class NextCandidate(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class NonConvert(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PreviousCandidate(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Process(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SingleCandidate(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class HangulMode(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class HanjaMode(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class JunjaMode(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Eisu(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Hankaku(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Hiragana(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class HiraganaKatakana(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class KanaMode(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class KanjiMode(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Katakana(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Romaji(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Zenkaku(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ZenkakuHankaku(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Soft1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Soft2(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Soft3(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Soft4(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ChannelDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ChannelUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Close(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MailForward(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MailReply(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MailSend(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaClose(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaFastForward(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaPause(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaPlay(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaPlayPause(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaRecord(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaRewind(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaStop(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaTrackNext(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaTrackPrevious(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class New(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Open(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Print(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Save(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SpellCheck(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Key11(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Key12(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioBalanceLeft(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioBalanceRight(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioBassBoostDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioBassBoostToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioBassBoostUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioFaderFront(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioFaderRear(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioSurroundModeNext(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioTrebleDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioTrebleUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioVolumeDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioVolumeUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AudioVolumeMute(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MicrophoneToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MicrophoneVolumeDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MicrophoneVolumeUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MicrophoneVolumeMute(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SpeechCorrectionList(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SpeechInputToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchApplication1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchApplication2(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchCalendar(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchContacts(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchMail(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchMediaPlayer(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchMusicPlayer(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchPhone(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchScreenSaver(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchSpreadsheet(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchWebBrowser(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchWebCam(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LaunchWordProcessor(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BrowserBack(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BrowserFavorites(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BrowserForward(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BrowserHome(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BrowserRefresh(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BrowserSearch(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class BrowserStop(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AppSwitch(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Call(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Camera(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class CameraFocus(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class EndCall(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class GoBack(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class GoHome(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class HeadsetHook(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LastNumberRedial(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Notification(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MannerMode(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class VoiceDial(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TV(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TV3DMode(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVAntennaCable(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVAudioDescription(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVAudioDescriptionMixDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVAudioDescriptionMixUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVContentsMenu(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVDataService(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInput(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInputComponent1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInputComponent2(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInputComposite1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInputComposite2(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInputHDMI1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInputHDMI2(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInputHDMI3(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInputHDMI4(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVInputVGA1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVMediaContext(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVNetwork(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVNumberEntry(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVPower(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVRadioService(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVSatellite(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVSatelliteBS(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVSatelliteCS(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVSatelliteToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVTerrestrialAnalog(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVTerrestrialDigital(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class TVTimer(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AVRInput(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class AVRPower(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ColorF0Red(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ColorF1Green(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ColorF2Yellow(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ColorF3Blue(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ColorF4Grey(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ColorF5Brown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ClosedCaptionToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Dimmer(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class DisplaySwap(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class DVR(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Exit(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteClear0(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteClear1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteClear2(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteClear3(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteRecall0(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteRecall1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteRecall2(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteRecall3(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteStore0(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteStore1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteStore2(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class FavoriteStore3(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Guide(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class GuideNextDay(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class GuidePreviousDay(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Info(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class InstantReplay(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Link(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ListProgram(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LiveContent(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Lock(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaApps(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaAudioTrack(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaLast(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaSkipBackward(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaSkipForward(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaStepBackward(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaStepForward(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MediaTopMenu(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class NavigateIn(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class NavigateNext(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class NavigateOut(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class NavigatePrevious(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class NextFavoriteChannel(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class NextUserProfile(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class OnDemand(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Pairing(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PinPDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PinPMove(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PinPToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PinPUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PlaySpeedDown(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PlaySpeedReset(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class PlaySpeedUp(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RandomToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RcLowBattery(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RecordSpeedNext(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RfBypass(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ScanChannelsToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ScreenModeNext(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Settings(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class SplitScreenToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class STBInput(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class STBPower(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Subtitle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Teletext(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class VideoModeNext(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Wink(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ZoomToggle(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F1(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F2(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F3(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F4(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F5(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F6(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F7(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F8(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F9(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F10(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F11(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F12(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F13(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F14(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F15(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F16(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F17(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F18(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F19(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F20(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F21(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F22(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F23(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F24(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F25(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F26(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F27(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F28(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F29(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F30(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F31(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F32(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F33(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F34(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class F35(Key):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    def __hash__(self) -> int: ...

class KeyCode:
    """Keyboard key codes for input detection."""

    class Unidentified(KeyCode):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: NativeKeyCode
        def __init__(self, value: NativeKeyCode) -> None: ...

    # Function keys
    Backquote: ClassVar[KeyCode]
    Backslash: ClassVar[KeyCode]
    BracketLeft: ClassVar[KeyCode]
    BracketRight: ClassVar[KeyCode]
    Comma: ClassVar[KeyCode]
    Digit0: ClassVar[KeyCode]
    Digit1: ClassVar[KeyCode]
    Digit2: ClassVar[KeyCode]
    Digit3: ClassVar[KeyCode]
    Digit4: ClassVar[KeyCode]
    Digit5: ClassVar[KeyCode]
    Digit6: ClassVar[KeyCode]
    Digit7: ClassVar[KeyCode]
    Digit8: ClassVar[KeyCode]
    Digit9: ClassVar[KeyCode]
    Equal: ClassVar[KeyCode]
    IntlBackslash: ClassVar[KeyCode]
    IntlRo: ClassVar[KeyCode]
    IntlYen: ClassVar[KeyCode]
    KeyA: ClassVar[KeyCode]
    KeyB: ClassVar[KeyCode]
    KeyC: ClassVar[KeyCode]
    KeyD: ClassVar[KeyCode]
    KeyE: ClassVar[KeyCode]
    KeyF: ClassVar[KeyCode]
    KeyG: ClassVar[KeyCode]
    KeyH: ClassVar[KeyCode]
    KeyI: ClassVar[KeyCode]
    KeyJ: ClassVar[KeyCode]
    KeyK: ClassVar[KeyCode]
    KeyL: ClassVar[KeyCode]
    KeyM: ClassVar[KeyCode]
    KeyN: ClassVar[KeyCode]
    KeyO: ClassVar[KeyCode]
    KeyP: ClassVar[KeyCode]
    KeyQ: ClassVar[KeyCode]
    KeyR: ClassVar[KeyCode]
    KeyS: ClassVar[KeyCode]
    KeyT: ClassVar[KeyCode]
    KeyU: ClassVar[KeyCode]
    KeyV: ClassVar[KeyCode]
    KeyW: ClassVar[KeyCode]
    KeyX: ClassVar[KeyCode]
    KeyY: ClassVar[KeyCode]
    KeyZ: ClassVar[KeyCode]
    Minus: ClassVar[KeyCode]
    Period: ClassVar[KeyCode]
    Quote: ClassVar[KeyCode]
    Semicolon: ClassVar[KeyCode]
    Slash: ClassVar[KeyCode]
    AltLeft: ClassVar[KeyCode]
    AltRight: ClassVar[KeyCode]
    Backspace: ClassVar[KeyCode]
    CapsLock: ClassVar[KeyCode]
    ContextMenu: ClassVar[KeyCode]
    ControlLeft: ClassVar[KeyCode]
    ControlRight: ClassVar[KeyCode]
    Enter: ClassVar[KeyCode]
    SuperLeft: ClassVar[KeyCode]
    SuperRight: ClassVar[KeyCode]
    ShiftLeft: ClassVar[KeyCode]
    ShiftRight: ClassVar[KeyCode]
    Space: ClassVar[KeyCode]
    Tab: ClassVar[KeyCode]
    Convert: ClassVar[KeyCode]
    KanaMode: ClassVar[KeyCode]
    Lang1: ClassVar[KeyCode]
    Lang2: ClassVar[KeyCode]
    Lang3: ClassVar[KeyCode]
    Lang4: ClassVar[KeyCode]
    Lang5: ClassVar[KeyCode]
    NonConvert: ClassVar[KeyCode]
    Delete: ClassVar[KeyCode]
    End: ClassVar[KeyCode]
    Help: ClassVar[KeyCode]
    Home: ClassVar[KeyCode]
    Insert: ClassVar[KeyCode]
    PageDown: ClassVar[KeyCode]
    PageUp: ClassVar[KeyCode]
    ArrowDown: ClassVar[KeyCode]
    ArrowLeft: ClassVar[KeyCode]
    ArrowRight: ClassVar[KeyCode]
    ArrowUp: ClassVar[KeyCode]
    NumLock: ClassVar[KeyCode]
    Numpad0: ClassVar[KeyCode]
    Numpad1: ClassVar[KeyCode]
    Numpad2: ClassVar[KeyCode]
    Numpad3: ClassVar[KeyCode]
    Numpad4: ClassVar[KeyCode]
    Numpad5: ClassVar[KeyCode]
    Numpad6: ClassVar[KeyCode]
    Numpad7: ClassVar[KeyCode]
    Numpad8: ClassVar[KeyCode]
    Numpad9: ClassVar[KeyCode]
    NumpadAdd: ClassVar[KeyCode]
    NumpadBackspace: ClassVar[KeyCode]
    NumpadClear: ClassVar[KeyCode]
    NumpadClearEntry: ClassVar[KeyCode]
    NumpadComma: ClassVar[KeyCode]
    NumpadDecimal: ClassVar[KeyCode]
    NumpadDivide: ClassVar[KeyCode]
    NumpadEnter: ClassVar[KeyCode]
    NumpadEqual: ClassVar[KeyCode]
    NumpadHash: ClassVar[KeyCode]
    NumpadMemoryAdd: ClassVar[KeyCode]
    NumpadMemoryClear: ClassVar[KeyCode]
    NumpadMemoryRecall: ClassVar[KeyCode]
    NumpadMemoryStore: ClassVar[KeyCode]
    NumpadMemorySubtract: ClassVar[KeyCode]
    NumpadMultiply: ClassVar[KeyCode]
    NumpadParenLeft: ClassVar[KeyCode]
    NumpadParenRight: ClassVar[KeyCode]
    NumpadStar: ClassVar[KeyCode]
    NumpadSubtract: ClassVar[KeyCode]
    Escape: ClassVar[KeyCode]
    Fn: ClassVar[KeyCode]
    FnLock: ClassVar[KeyCode]
    PrintScreen: ClassVar[KeyCode]
    ScrollLock: ClassVar[KeyCode]
    Pause: ClassVar[KeyCode]
    BrowserBack: ClassVar[KeyCode]
    BrowserFavorites: ClassVar[KeyCode]
    BrowserForward: ClassVar[KeyCode]
    BrowserHome: ClassVar[KeyCode]
    BrowserRefresh: ClassVar[KeyCode]
    BrowserSearch: ClassVar[KeyCode]
    BrowserStop: ClassVar[KeyCode]
    Eject: ClassVar[KeyCode]
    LaunchApp1: ClassVar[KeyCode]
    LaunchApp2: ClassVar[KeyCode]
    LaunchMail: ClassVar[KeyCode]
    MediaPlayPause: ClassVar[KeyCode]
    MediaSelect: ClassVar[KeyCode]
    MediaStop: ClassVar[KeyCode]
    MediaTrackNext: ClassVar[KeyCode]
    MediaTrackPrevious: ClassVar[KeyCode]
    Power: ClassVar[KeyCode]
    Sleep: ClassVar[KeyCode]
    AudioVolumeDown: ClassVar[KeyCode]
    AudioVolumeMute: ClassVar[KeyCode]
    AudioVolumeUp: ClassVar[KeyCode]
    WakeUp: ClassVar[KeyCode]
    Meta: ClassVar[KeyCode]
    Hyper: ClassVar[KeyCode]
    Turbo: ClassVar[KeyCode]
    Abort: ClassVar[KeyCode]
    Resume: ClassVar[KeyCode]
    Suspend: ClassVar[KeyCode]
    Again: ClassVar[KeyCode]
    Copy: ClassVar[KeyCode]
    Cut: ClassVar[KeyCode]
    Find: ClassVar[KeyCode]
    Open: ClassVar[KeyCode]
    Paste: ClassVar[KeyCode]
    Props: ClassVar[KeyCode]
    Select: ClassVar[KeyCode]
    Undo: ClassVar[KeyCode]
    Hiragana: ClassVar[KeyCode]
    Katakana: ClassVar[KeyCode]
    F1: ClassVar[KeyCode]
    F2: ClassVar[KeyCode]
    F3: ClassVar[KeyCode]
    F4: ClassVar[KeyCode]
    F5: ClassVar[KeyCode]
    F6: ClassVar[KeyCode]
    F7: ClassVar[KeyCode]
    F8: ClassVar[KeyCode]
    F9: ClassVar[KeyCode]
    F10: ClassVar[KeyCode]
    F11: ClassVar[KeyCode]
    F12: ClassVar[KeyCode]
    F13: ClassVar[KeyCode]
    F14: ClassVar[KeyCode]
    F15: ClassVar[KeyCode]
    F16: ClassVar[KeyCode]
    F17: ClassVar[KeyCode]
    F18: ClassVar[KeyCode]
    F19: ClassVar[KeyCode]
    F20: ClassVar[KeyCode]
    F21: ClassVar[KeyCode]
    F22: ClassVar[KeyCode]
    F23: ClassVar[KeyCode]
    F24: ClassVar[KeyCode]
    F25: ClassVar[KeyCode]
    F26: ClassVar[KeyCode]
    F27: ClassVar[KeyCode]
    F28: ClassVar[KeyCode]
    F29: ClassVar[KeyCode]
    F30: ClassVar[KeyCode]
    F31: ClassVar[KeyCode]
    F32: ClassVar[KeyCode]
    F33: ClassVar[KeyCode]
    F34: ClassVar[KeyCode]
    F35: ClassVar[KeyCode]

    def __hash__(self) -> int: ...

class ButtonInput(Resource, Generic[ButtonT]):
    """
    Tracks button state - whether buttons are pressed, just pressed, or just released.

    Subscript to pick the button type, matching Bevy's generic `ButtonInput<T>`:
    `ButtonInput[KeyCode]` is the keyboard resource, `ButtonInput[MouseButton]`
    the mouse one. A bare `ButtonInput` means the keyboard.

    This resource is automatically provided as a system parameter and should not be
    instantiated directly.

    Example:
        ```python
        def handle_input_system(input: Res[ButtonInput[KeyCode]]) -> None:
            if input.just_pressed(KeyCode.Space):
                print("Space bar was just pressed!")

            if input.pressed(KeyCode.ShiftLeft):
                print("Left shift is being held")

            if input.just_released(KeyCode.Escape):
                print("Escape was just released")
        ```
    """

    def __init__(self) -> None:
        """Create a new ButtonInput instance (typically done internally)."""

    def just_pressed(self, input: ButtonT) -> bool:
        """
        Returns true if the key was just pressed this frame.

        Args:
            input: The key code to check

        Returns:
            True if the key was just pressed this frame, False otherwise
        """

    def just_released(self, input: ButtonT) -> bool:
        """
        Returns true if the key was just released this frame.

        Args:
            input: The key code to check

        Returns:
            True if the key was just released this frame, False otherwise
        """

    def pressed(self, input: ButtonT) -> bool:
        """
        Returns true if the key is currently held down.

        Args:
            input: The key code to check

        Returns:
            True if the key is currently pressed, False otherwise
        """

    def any_just_pressed(self, inputs: list[ButtonT]) -> bool:
        """
        Returns true if any of the keys were just pressed this frame.

        Args:
            inputs: List of key codes to check

        Returns:
            True if any key in the list was just pressed
        """

    def any_pressed(self, inputs: list[ButtonT]) -> bool:
        """
        Returns true if any of the keys are currently pressed.

        Args:
            inputs: List of key codes to check

        Returns:
            True if any key in the list is currently pressed
        """

    def all_pressed(self, inputs: list[ButtonT]) -> bool:
        """
        Returns true if all of the keys are currently pressed.

        Args:
            inputs: List of key codes to check

        Returns:
            True if all keys in the list are currently pressed
        """

    def get_just_pressed(self) -> list[ButtonT]:
        """
        Get all keys that were just pressed this frame.

        Returns:
            List of KeyCodes that were just pressed
        """

    def get_pressed(self) -> list[ButtonT]:
        """
        Get all keys that are currently pressed.

        Returns:
            List of KeyCodes that are currently pressed
        """

    def get_just_released(self) -> list[ButtonT]:
        """
        Get all keys that were just released this frame.

        Returns:
            List of KeyCodes that were just released
        """

    def any_just_released(self, inputs: list[ButtonT]) -> bool:
        """
        Returns true if any of the keys were just released this frame.

        Args:
            inputs: List of key codes to check

        Returns:
            True if any key in the list was just released
        """

    def all_just_pressed(self, inputs: list[ButtonT]) -> bool:
        """
        Returns true if all of the keys were just pressed this frame.

        Args:
            inputs: List of key codes to check

        Returns:
            True if all keys in the list were just pressed
        """

    def all_just_released(self, inputs: list[ButtonT]) -> bool:
        """
        Returns true if all of the keys were just released this frame.

        Args:
            inputs: List of key codes to check

        Returns:
            True if all keys in the list were just released
        """

class MouseButton:
    """Mouse button codes for input detection."""

    class Left(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Right(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Middle(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Back(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Forward(MouseButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Other(MouseButton):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    def __hash__(self) -> int: ...

class MouseInput(Resource):
    """
    Tracks the state of mouse buttons - whether they're pressed, just pressed, or just released.

    `ButtonInput[MouseButton]` is the canonical spelling and resolves to this
    class; the name remains valid.

    This resource is automatically provided as a system parameter and should not be
    instantiated directly.

    Example:
        ```python
        def handle_mouse_system(mouse: MouseInput) -> None:
            if mouse.just_pressed(MouseButton.Left()):
                print("Left mouse button was just pressed!")

            if mouse.pressed(MouseButton.Right()):
                print("Right mouse button is being held")

            if mouse.just_released(MouseButton.Middle()):
                print("Middle mouse button was just released")
        ```
    """

    def __init__(self) -> None:
        """Create a new MouseInput instance (typically done internally)."""

    def just_pressed(self, button: MouseButton) -> bool:
        """
        Returns true if the button was just pressed this frame.

        Args:
            button: The mouse button to check

        Returns:
            True if the button was just pressed this frame, False otherwise
        """

    def just_released(self, button: MouseButton) -> bool:
        """
        Returns true if the button was just released this frame.

        Args:
            button: The mouse button to check

        Returns:
            True if the button was just released this frame, False otherwise
        """

    def pressed(self, button: MouseButton) -> bool:
        """
        Returns true if the button is currently held down.

        Args:
            button: The mouse button to check

        Returns:
            True if the button is currently pressed, False otherwise
        """

    def any_just_pressed(self, buttons: list[MouseButton]) -> bool:
        """
        Returns true if any of the buttons were just pressed this frame.

        Args:
            buttons: List of mouse buttons to check

        Returns:
            True if any button in the list was just pressed
        """

    def any_pressed(self, buttons: list[MouseButton]) -> bool:
        """
        Returns true if any of the buttons are currently pressed.

        Args:
            buttons: List of mouse buttons to check

        Returns:
            True if any button in the list is currently pressed
        """

    def all_pressed(self, buttons: list[MouseButton]) -> bool:
        """
        Returns true if all of the buttons are currently pressed.

        Args:
            buttons: List of mouse buttons to check

        Returns:
            True if all buttons in the list are currently pressed
        """

    def get_just_pressed(self) -> list[MouseButton]:
        """
        Get all buttons that were just pressed this frame.

        Returns:
            List of MouseButtons that were just pressed
        """

    def get_pressed(self) -> list[MouseButton]:
        """
        Get all buttons that are currently pressed.

        Returns:
            List of MouseButtons that are currently pressed
        """

    def get_just_released(self) -> list[MouseButton]:
        """
        Get all buttons that were just released this frame.

        Returns:
            List of MouseButtons that were just released
        """

    def any_just_released(self, buttons: list[MouseButton]) -> bool:
        """Returns true if any of the given buttons were just released."""

    def all_just_pressed(self, buttons: list[MouseButton]) -> bool:
        """Returns true if all of the given buttons were just pressed."""

    def all_just_released(self, buttons: list[MouseButton]) -> bool:
        """Returns true if all of the given buttons were just released."""

class ButtonState:
    """State of a button (pressed or released)."""

    @staticmethod
    def Pressed() -> ButtonState: ...
    @staticmethod
    def Released() -> ButtonState: ...
    def is_pressed(self) -> bool:
        """Returns true if this state is Pressed."""

    def __hash__(self) -> int: ...

class KeyboardInput(Message):
    """
    Keyboard input event message.

    Contains information about a key press or release event.
    Use with MessageReader to receive keyboard events.

    Example:
        ```python
        def handle_keys(reader: MessageReader[KeyboardInput]) -> None:
            for event in reader:
                if event.state == ButtonState.Pressed():
                    print(f"Key pressed: {event.key_code}")
        ```
    """

    def __init__(
        self,
        key_code: KeyCode,
        logical_key: Key,
        state: ButtonState,
        text: str | None = None,
        repeat: bool = False,
        window: Entity = ...,
    ) -> None: ...
    @property
    def key_code(self) -> KeyCode:
        """The key that was pressed or released."""

    @property
    def logical_key(self) -> Key:
        """The logical meaning of the key."""

    @property
    def state(self) -> ButtonState:
        """Whether the key was pressed or released."""

    @property
    def text(self) -> str | None:
        """
        The text produced by this keypress.

        Returns None if this keypress cannot be interpreted as text.
        """

    @property
    def repeat(self) -> bool:
        """Whether this is a repeated key event (key held down)."""

    @property
    def window(self) -> Entity:
        """The window entity this event was received on."""

class MouseButtonInput(Message):
    """
    Mouse button input event message.

    Contains information about a mouse button press or release event.
    Use with MessageReader to receive mouse button events.

    Example:
        ```python
        def handle_clicks(reader: MessageReader[MouseButtonInput]) -> None:
            for event in reader:
                if event.state == ButtonState.Pressed():
                    print(f"Mouse button pressed: {event.button}")
        ```
    """

    def __init__(
        self, button: MouseButton, state: ButtonState, window: Entity = ...
    ) -> None: ...
    @property
    def button(self) -> MouseButton:
        """The mouse button that was pressed or released."""

    @property
    def state(self) -> ButtonState:
        """Whether the button was pressed or released."""

    @property
    def window(self) -> Entity:
        """The window entity this event was received on."""

class MouseMotion(Message):
    """
    Mouse motion event message.

    Contains information about mouse cursor movement.
    Use with MessageReader to receive mouse motion events.

    Example:
        ```python
        def handle_mouse_move(reader: MessageReader[MouseMotion]) -> None:
            for event in reader:
                print(f"Mouse moved: ({event.delta.x}, {event.delta.y})")
        ```
    """

    def __init__(self, delta: Vec2) -> None: ...
    @property
    def delta(self) -> Vec2:
        """Mouse movement delta as a Vec2."""

class MouseScrollUnit:
    """
    The scroll unit for a mouse wheel event.

    Describes how a value of a MouseWheel event has to be interpreted.
    The value can either be interpreted as the amount of lines or the amount of pixels to scroll.
    """

    Line: MouseScrollUnit
    """The line scroll unit - delta corresponds to lines/rows to scroll."""
    Pixel: MouseScrollUnit
    """The pixel scroll unit - delta corresponds to pixels to scroll."""

    def __hash__(self) -> int: ...

class MouseWheel(Message):
    """
    Mouse wheel scroll event message.

    Contains information about mouse wheel scrolling.
    Use with MessageReader to receive scroll events.

    Example:
        ```python
        def handle_scroll(reader: MessageReader[MouseWheel]) -> None:
            for event in reader:
                print(f"Scroll: ({event.x}, {event.y}) unit: {event.unit}")
        ```
    """

    def __init__(
        self, x: float, y: float, unit: MouseScrollUnit = ..., window: Entity = ...
    ) -> None: ...
    @property
    def x(self) -> float:
        """Horizontal scroll amount."""

    @property
    def y(self) -> float:
        """Vertical scroll amount."""

    @property
    def unit(self) -> MouseScrollUnit:
        """The scroll unit (Line or Pixel)."""

    @property
    def window(self) -> Entity:
        """The window entity this event was received on."""

class GamepadButton:
    """
    Gamepad button codes for input detection.

    Uses cardinal directions for face buttons (matching Bevy):
    - South: A button on Xbox, Cross on PlayStation
    - East: B button on Xbox, Circle on PlayStation
    - North: Y button on Xbox, Triangle on PlayStation
    - West: X button on Xbox, Square on PlayStation
    """

    class South(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class East(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class North(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class West(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class C(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Z(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LeftTrigger(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LeftTrigger2(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RightTrigger(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RightTrigger2(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Select(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Start(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Mode(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LeftThumb(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RightThumb(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class DPadUp(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class DPadDown(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class DPadLeft(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class DPadRight(GamepadButton):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Other(GamepadButton):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    @staticmethod
    def all() -> list[GamepadButton]:
        """Returns a list of all standard gamepad buttons (excluding Other)."""

    def __hash__(self) -> int: ...

class GamepadAxis:
    """Gamepad axis codes for analog input detection."""

    class LeftStickX(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LeftStickY(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class LeftZ(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RightStickX(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RightStickY(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class RightZ(GamepadAxis):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Other(GamepadAxis):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: int
        def __init__(self, value: int) -> None: ...

    @staticmethod
    def all() -> list[GamepadAxis]:
        """Returns a list of all standard gamepad axes (excluding Other)."""

    def __hash__(self) -> int: ...

class GamepadInput:
    """
    Represents a gamepad input which can be either an axis or a button.

    Used for generic gamepad input operations where the input type may vary.
    """

    class Axis(GamepadInput):
        __match_args__: ClassVar[tuple[Literal["axis"]]]
        axis: GamepadAxis
        def __init__(self, axis: GamepadAxis) -> None: ...

    class Button(GamepadInput):
        __match_args__: ClassVar[tuple[Literal["button"]]]
        button: GamepadButton
        def __init__(self, button: GamepadButton) -> None: ...

class Gamepad(Component):
    """
    Gamepad component that tracks button and axis state for a connected gamepad.

    Query for entities with this component to access gamepad input state.

    Example:
        ```python
        def handle_gamepad(query: Query[Gamepad]) -> None:
            for gamepad in query:
                if gamepad.just_pressed(GamepadButton.South()):
                    print("A/Cross button pressed!")

                left_x = gamepad.get_axis(GamepadAxis.LeftStickX())
                if left_x is not None:
                    print(f"Left stick X: {left_x}")
        ```
    """

    def just_pressed(self, button_type: GamepadButton) -> bool:
        """Returns true if the button was just pressed this frame."""

    def just_released(self, button_type: GamepadButton) -> bool:
        """Returns true if the button was just released this frame."""

    def pressed(self, button_type: GamepadButton) -> bool:
        """Returns true if the button is currently held down."""

    def get_button(self, button: GamepadButton) -> float | None:
        """Get the analog value of a button (0.0 to 1.0), or None if not available."""

    def get_axis(self, axis: GamepadAxis) -> float | None:
        """Get the value of an axis (-1.0 to 1.0), or None if not available."""

    def get_button_unclamped(self, button: GamepadButton) -> float | None:
        """Get the unclamped analog value of a button (may be outside -1.0 to 1.0)."""

    def get_axis_unclamped(self, axis: GamepadAxis) -> float | None:
        """Get the unclamped value of an axis (may be outside -1.0 to 1.0)."""

    def get(self, input: GamepadInput) -> float | None:
        """Get the analog value of a GamepadInput (axis or button), clamped to [-1.0, 1.0]."""

    def get_unclamped(self, input: GamepadInput) -> float | None:
        """Get the unclamped analog value of a GamepadInput (axis or button)."""

    def get_analog_axes(self) -> list[GamepadInput]:
        """Get all analog inputs (axes and buttons) that have values."""

    def get_pressed(self) -> list[GamepadButton]:
        """Get all buttons that are currently pressed."""

    def get_just_pressed(self) -> list[GamepadButton]:
        """Get all buttons that were just pressed this frame."""

    def get_just_released(self) -> list[GamepadButton]:
        """Get all buttons that were just released this frame."""

    def left_stick(self) -> Vec2:
        """Returns the left analog stick as a Vec2 (x, y)."""

    def right_stick(self) -> Vec2:
        """Returns the right analog stick as a Vec2 (x, y)."""

    def dpad(self) -> Vec2:
        """Returns the directional pad as a Vec2 (x: left/right, y: up/down)."""

    def any_pressed(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if any of the buttons are currently pressed."""

    def all_pressed(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if all of the buttons are currently pressed."""

    def any_just_pressed(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if any of the buttons were just pressed this frame."""

    def all_just_pressed(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if all of the buttons were just pressed this frame."""

    def any_just_released(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if any of the buttons were just released this frame."""

    def all_just_released(self, button_inputs: list[GamepadButton]) -> bool:
        """Returns true if all of the buttons were just released this frame."""

    def vendor_id(self) -> int | None:
        """Returns the USB vendor ID as assigned by the USB-IF, if available."""

    def product_id(self) -> int | None:
        """Returns the USB product ID as assigned by the vendor, if available."""

class GamepadButtonChangedEvent(Message):
    """
    Gamepad button event message.

    Contains information about gamepad button changes with analog value.
    Use with MessageReader to receive gamepad button events.

    Example:
        ```python
        def handle_gamepad(reader: MessageReader[GamepadButtonChangedEvent]) -> None:
            for event in reader:
                print(f"Button {event.button} value: {event.value}")
        ```
    """

    def __init__(
        self,
        button: GamepadButton,
        value: float,
        *,
        state: ButtonState = ...,
        entity: Entity | None = None,
    ) -> None: ...
    @property
    def entity(self) -> Entity:
        """The gamepad this button belongs to."""

    @property
    def button(self) -> GamepadButton:
        """The gamepad button that changed."""

    @property
    def state(self) -> ButtonState:
        """Whether the button is pressed or released."""

    @property
    def value(self) -> float:
        """Analog value of the button (0.0 to 1.0)."""

class GamepadAxisChangedEvent(Message):
    """
    Gamepad axis event message.

    Contains information about gamepad axis changes.
    Use with MessageReader to receive gamepad axis events.

    Example:
        ```python
        def handle_gamepad(reader: MessageReader[GamepadAxisChangedEvent]) -> None:
            for event in reader:
                print(f"Axis {event.axis} value: {event.value}")
        ```
    """

    def __init__(
        self, axis: GamepadAxis, value: float, *, entity: Entity | None = None
    ) -> None: ...
    @property
    def entity(self) -> Entity:
        """The gamepad this axis belongs to."""

    @property
    def axis(self) -> GamepadAxis:
        """The gamepad axis that changed."""

    @property
    def value(self) -> float:
        """Axis value (-1.0 to 1.0)."""

class GamepadConnection:
    """
    Whether a gamepad connected or disconnected, and its device metadata.

    This is the payload of `GamepadConnectionEvent.connection`. Match on it to
    handle the two cases:

    Example:
        ```python
        def handle_gamepad(reader: MessageReader[GamepadConnectionEvent]) -> None:
            for event in reader:
                match event.connection:
                    case GamepadConnection.Connected(name, vendor_id, product_id):
                        print(f"Gamepad connected: {name} ({vendor_id}:{product_id})")
                    case GamepadConnection.Disconnected():
                        print("Gamepad disconnected")
        ```
    """

    class Connected(GamepadConnection):
        __match_args__: ClassVar[
            tuple[Literal["name"], Literal["vendor_id"], Literal["product_id"]]
        ]
        name: str
        vendor_id: int | None
        product_id: int | None

        def __init__(
            self, name: str, vendor_id: int | None, product_id: int | None
        ) -> None: ...

    class Disconnected(GamepadConnection):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

class GamepadConnectionEvent(Message):
    """
    Gamepad connection event message.

    Use with MessageReader to receive gamepad connection and disconnection
    events. The device metadata lives on the `connection` payload.

    Example:
        ```python
        def handle_gamepad(reader: MessageReader[GamepadConnectionEvent]) -> None:
            for event in reader:
                if event.connected():
                    print(f"Gamepad {event.gamepad} connected")
        ```
    """

    def __init__(self, gamepad: Entity, connection: GamepadConnection) -> None: ...
    @property
    def gamepad(self) -> Entity:
        """The gamepad entity that connected or disconnected."""

    @property
    def connection(self) -> GamepadConnection:
        """The change in the gamepad's connection."""

    def connected(self) -> bool:
        """Whether the gamepad is connected."""

    def disconnected(self) -> bool:
        """Whether the gamepad is disconnected."""

class TouchPhase:
    """
    Touch phase enum - describes the current state of a touch.

    Variants:
        Started: A finger started to touch the touchscreen
        Moved: A finger moved over the touchscreen
        Ended: A finger stopped touching the touchscreen
        Canceled: The system canceled tracking (window lost focus, etc.)
    """

    Started: TouchPhase
    Moved: TouchPhase
    Ended: TouchPhase
    Canceled: TouchPhase

    def __hash__(self) -> int: ...

class TouchInput(Message):
    """
    Touch input event message.

    Contains information about touch screen interactions.
    Use with MessageReader to receive touch events.

    Example:
        ```python
        def handle_touch(reader: MessageReader[TouchInput]) -> None:
            for event in reader:
                if event.phase == TouchPhase.Started:
                    print(f"Touch started at {event.position}")
                elif event.phase == TouchPhase.Moved:
                    print(f"Touch moved to {event.position}")
                elif event.phase == TouchPhase.Ended:
                    print(f"Touch ended at {event.position}")
        ```
    """

    def __init__(
        self,
        phase: TouchPhase,
        position: Vec2,
        id: int,
        force: float | None = None,
        window: Entity = ...,
    ) -> None: ...
    @property
    def phase(self) -> TouchPhase:
        """The phase of the touch event."""

    @property
    def position(self) -> Vec2:
        """The position of the touch in window coordinates."""

    @property
    def id(self) -> int:
        """
        Unique identifier for this touch/finger.

        Different fingers will have different IDs, allowing multi-touch tracking.
        """

    @property
    def force(self) -> float | None:
        """
        Optional pressure data for pressure-sensitive touchscreens.

        Returns a value between 0.0 and 1.0, or None if not supported.
        """

    @property
    def window(self) -> Entity:
        """The window entity this event was received on."""

class AccumulatedMouseMotion(Resource):
    """Resource that accumulates mouse motion delta per frame.

    This resource tracks the total mouse movement that occurred during the current
    frame, providing a convenient way to access accumulated motion for camera controls,
    drag operations, and other mouse-based interactions.

    The delta values are reset each frame, so they represent only the current frame's motion.
    """

    def __init__(self) -> None: ...
    @property
    def delta(self) -> Vec2:
        """Accumulated mouse movement this frame as a Vec2."""

class AccumulatedMouseScroll(Resource):
    """Resource that accumulates mouse scroll delta per frame."""

    def __init__(self, unit: MouseScrollUnit = ...) -> None: ...
    @property
    def delta(self) -> Vec2:
        """Accumulated scroll this frame as a Vec2."""

    @property
    def unit(self) -> MouseScrollUnit:
        """The scroll unit (Line or Pixel)."""

class GamepadRumbleIntensity:
    """Gamepad rumble/haptic intensity settings."""

    MAX: GamepadRumbleIntensity
    WEAK_MAX: GamepadRumbleIntensity
    STRONG_MAX: GamepadRumbleIntensity

    def __init__(
        self,
        strong_motor: float = 1.0,
        weak_motor: float = 1.0,
    ) -> None: ...
    @property
    def strong_motor(self) -> float:
        """Intensity of the strong (low-frequency) motor (0.0-1.0)."""

    @property
    def weak_motor(self) -> float:
        """Intensity of the weak (high-frequency) motor (0.0-1.0)."""

class PinchGesture(Message):
    """Two-finger pinch gesture event (macOS/iOS only)."""

    def __init__(self, value: float) -> None: ...
    @property
    def value(self) -> float:
        """The pinch delta. Positive = magnify, negative = shrink."""

class RotationGesture(Message):
    """Two-finger rotation gesture event (macOS/iOS only)."""

    def __init__(self, value: float) -> None: ...
    @property
    def value(self) -> float:
        """The rotation delta in radians. Positive = counterclockwise."""

class DoubleTapGesture(Message):
    """Double tap gesture event (macOS/iOS only)."""

    def __init__(self) -> None: ...

class PanGesture(Message):
    """Pan gesture event."""

    def __init__(self, x: float, y: float) -> None: ...
    @property
    def x(self) -> float:
        """Horizontal pan delta."""

    @property
    def y(self) -> float:
        """Vertical pan delta."""

    @property
    def delta(self) -> Vec2:
        """Pan delta as a Vec2."""

class GamepadButtonStateChangedEvent(Message):
    """Gamepad button state change event."""

    def __init__(
        self, button: GamepadButton, state: ButtonState, *, entity: Entity | None = None
    ) -> None: ...
    @property
    def entity(self) -> Entity:
        """The gamepad this button belongs to."""

    @property
    def button(self) -> GamepadButton:
        """The gamepad button that changed state."""

    @property
    def state(self) -> ButtonState:
        """The new state of the button."""

class GamepadEvent:
    """Unified gamepad event (connection, button, or axis).

    This is a PyO3 complex enum with Connection, Button, and Axis variants.
    Use pattern matching to handle different event types.
    """

    class Connection(GamepadEvent):
        __match_args__: ClassVar[
            tuple[
                Literal["connected"],
                Literal["name"],
                Literal["vendor_id"],
                Literal["product_id"],
            ]
        ]
        connected: bool
        name: str | None
        vendor_id: int | None
        product_id: int | None

        def __init__(
            self,
            connected: bool,
            name: str | None = None,
            vendor_id: int | None = None,
            product_id: int | None = None,
        ) -> None: ...

    class Button(GamepadEvent):
        __match_args__: ClassVar[tuple[Literal["button"], Literal["value"]]]
        button: GamepadButton
        value: float
        def __init__(self, button: GamepadButton, value: float) -> None: ...

    class Axis(GamepadEvent):
        __match_args__: ClassVar[tuple[Literal["axis"], Literal["value"]]]
        axis: GamepadAxis
        value: float
        def __init__(self, axis: GamepadAxis, value: float) -> None: ...

class KeyboardFocusLost(Message):
    """
    Keyboard focus lost event message.

    Triggered when the window loses keyboard focus. This is useful for pausing
    input processing or releasing currently pressed keys to avoid stuck key states.

    Example:
        ```python
        def handle_focus_lost(reader: MessageReader[KeyboardFocusLost]) -> None:
            for event in reader:
                print("Keyboard focus lost - pausing input")
                # Release all pressed keys or pause game
        ```
    """

    def __init__(self) -> None: ...

class GamepadRumbleRequest(Message):
    """
    Gamepad rumble/haptic feedback request message.

    Send this message to request haptic feedback on connected gamepads.
    Gamepads have two motors: strong (low-frequency) and weak (high-frequency).

    Example:
        ```python
        def trigger_rumble(writer: MessageWriter[GamepadRumbleRequest]) -> None:
            # Strong rumble on both motors for 0.5 seconds
            writer.write(GamepadRumbleRequest(duration_secs=0.5))

            # Custom motor intensities
            writer.write(GamepadRumbleRequest(
                duration_secs=0.3,
                strong_motor=1.0,
                weak_motor=0.5
            ))
        ```
    """

    def __init__(
        self,
        duration_secs: float,
        strong_motor: float = 1.0,
        weak_motor: float = 1.0,
        gamepad: Entity = ...,
    ) -> None: ...
    @property
    def duration_secs(self) -> float:
        """Duration of the rumble effect in seconds."""

    @property
    def strong_motor(self) -> float:
        """Intensity of the strong (low-frequency) motor (0.0-1.0)."""

    @property
    def weak_motor(self) -> float:
        """Intensity of the weak (high-frequency) motor (0.0-1.0)."""

    def gamepad(self) -> Entity:
        """Get the Entity associated with this request."""

class Touch:
    """
    A single touch input with position, force, and movement tracking.

    Tracks a finger's position and movement across the touchscreen, including
    starting position, previous position, and optional pressure data.
    """

    def __init__(self, id: int, position: Vec2) -> None: ...
    @property
    def id(self) -> int:
        """Unique identifier for this touch/finger."""

    @property
    def position(self) -> Vec2:
        """Current position of the touch."""

    @property
    def start_position(self) -> Vec2:
        """Position where the touch first made contact."""

    @property
    def previous_position(self) -> Vec2:
        """Position of the touch in the previous frame."""

    @property
    def force(self) -> float | None:
        """
        Current pressure/force of the touch, if supported.

        Normalized to 0.0-1.0 range. None if pressure sensing not available.
        """

    @property
    def start_force(self) -> float | None:
        """Pressure/force when the touch first made contact."""

    @property
    def previous_force(self) -> float | None:
        """Pressure/force in the previous frame."""

    def delta(self) -> Vec2:
        """Get the movement delta between current and previous position."""

    def distance(self) -> Vec2:
        """Get the total distance moved from start position to current position."""

class Touches(Resource):
    """
    Multi-touch input state tracking resource.

    Manages all active touches and provides queries for touch state changes.
    Automatically updated by the InputPlugin from touch screen events.

    Example:
        ```python
        def handle_touches(touches: Res[Touches]) -> None:
            # Check for new touches
            if touches.any_just_pressed():
                for touch in touches.iter_just_pressed():
                    print(f"New touch {touch.id} at {touch.position}")

            # Process active touches
            for touch in touches.iter():
                delta = touch.delta()
                print(f"Touch {touch.id} moved {delta.x}, {delta.y}")

            # Check for released touches
            for touch in touches.iter_just_released():
                distance = touch.distance()
                print(f"Touch {touch.id} released after {distance.length()} pixels")
        ```
    """

    def __init__(self) -> None: ...
    def any_just_pressed(self) -> bool:
        """Returns true if any touch was just started this frame."""

    def any_just_released(self) -> bool:
        """Returns true if any touch was just released this frame."""

    def any_just_canceled(self) -> bool:
        """Returns true if any touch was just canceled this frame."""

    def just_pressed(self, id: int) -> bool:
        """Returns true if the touch with given ID was just started."""

    def just_released(self, id: int) -> bool:
        """Returns true if the touch with given ID was just released."""

    def just_canceled(self, id: int) -> bool:
        """Returns true if the touch with given ID was just canceled."""

    def get_pressed(self, id: int) -> Touch | None:
        """Get touch data for a currently pressed touch by ID."""

    def get_released(self, id: int) -> Touch | None:
        """Get touch data for a just-released touch by ID."""

    def iter(self) -> list[Touch]:
        """Get all currently pressed touches."""

    def iter_just_pressed(self) -> list[Touch]:
        """Get all touches that were just started this frame."""

    def iter_just_released(self) -> list[Touch]:
        """Get all touches that were just released this frame."""

    def iter_just_canceled(self) -> list[Touch]:
        """Get all touches that were just canceled this frame."""

    def first_pressed_position(self) -> Vec2 | None:
        """Get the position of the first currently pressed touch, if any."""

    def clear(self) -> None:
        """Clears the just_pressed, just_released, and just_canceled data."""

    def clear_just_pressed(self, id: int) -> bool:
        """Clears the just_pressed state for a touch and returns True if it was just pressed."""

    def clear_just_released(self, id: int) -> bool:
        """Clears the just_released state for a touch and returns True if it was just released."""

    def clear_just_canceled(self, id: int) -> bool:
        """Clears the just_canceled state for a touch and returns True if it was just canceled."""

    def release(self, id: int) -> None:
        """Register a release for a given touch input."""

    def release_all(self) -> None:
        """Registers a release for all currently pressed touch inputs."""

    def reset_all(self) -> None:
        """Clears all touch data: pressed, just_pressed, just_released, and just_canceled."""

class ButtonSettings:
    """Button press/release threshold settings.

    Controls when a button is considered pressed or released based on analog values.
    """

    def __init__(
        self, press_threshold: float = 0.75, release_threshold: float = 0.65
    ) -> None:
        """Create button settings.

        Args:
            press_threshold: Value above which button is pressed (0.0-1.0)
            release_threshold: Value below which button is released (0.0-1.0)

        Raises:
            ValueError: If thresholds are out of range or release > press
        """

    @property
    def press_threshold(self) -> float:
        """The threshold above which a button is considered pressed."""

    @property
    def release_threshold(self) -> float:
        """The threshold below which a button is considered released."""

    def is_pressed(self, value: float) -> bool:
        """Returns True if the button is considered pressed at the given value."""

    def is_released(self, value: float) -> bool:
        """Returns True if the button is considered released at the given value."""

class AxisSettings:
    """Axis deadzone and livezone settings.

    Controls how axis values are processed, including deadzones and livezones.
    """

    def __init__(
        self,
        livezone_lowerbound: float = -1.0,
        deadzone_lowerbound: float = -0.05,
        deadzone_upperbound: float = 0.05,
        livezone_upperbound: float = 1.0,
        threshold: float = 0.01,
    ) -> None:
        """Create axis settings.

        Args:
            livezone_lowerbound: Value below which inputs round to -1.0
            deadzone_lowerbound: Value above which negative inputs round to 0.0
            deadzone_upperbound: Value below which positive inputs round to 0.0
            livezone_upperbound: Value above which inputs round to 1.0
            threshold: Minimum change required to register input

        Raises:
            ValueError: If bounds are invalid
        """

    @property
    def livezone_upperbound(self) -> float:
        """Value above which inputs are rounded to 1.0."""

    @property
    def deadzone_upperbound(self) -> float:
        """Value below which positive inputs are rounded to 0.0."""

    @property
    def deadzone_lowerbound(self) -> float:
        """Value above which negative inputs are rounded to 0.0."""

    @property
    def livezone_lowerbound(self) -> float:
        """Value below which inputs are rounded to -1.0."""

    @property
    def threshold(self) -> float:
        """Minimum change required to register input."""

    def clamp(self, value: float) -> float:
        """Clamp a raw axis value according to settings."""

class ButtonAxisSettings:
    """Button axis settings for analog button values.

    Controls how analog button values are rounded.
    """

    def __init__(
        self, high: float = 0.95, low: float = 0.05, threshold: float = 0.01
    ) -> None:
        """Create button axis settings.

        Args:
            high: Value at which to round to 1.0
            low: Value at which to round to 0.0
            threshold: Threshold for change detection
        """

    @property
    def high(self) -> float:
        """The high value at which to round to 1.0."""

    @property
    def low(self) -> float:
        """The low value at which to round to 0.0."""

    @property
    def threshold(self) -> float:
        """The threshold for change detection."""

class GamepadSettings(Component):
    """Gamepad input settings component.

    Controls deadzone, livezone, and threshold settings for gamepad inputs.
    Attached to gamepad entities to customize their input behavior.
    """

    def __init__(
        self,
        default_button_settings: ButtonSettings | None = None,
        default_axis_settings: AxisSettings | None = None,
        default_button_axis_settings: ButtonAxisSettings | None = None,
        button_settings: dict[GamepadButton, ButtonSettings] | None = None,
        axis_settings: dict[GamepadAxis, AxisSettings] | None = None,
        button_axis_settings: dict[GamepadButton, ButtonAxisSettings] | None = None,
    ) -> None: ...
    @property
    def default_button_settings(self) -> ButtonSettings:
        """Get the default button settings."""

    @default_button_settings.setter
    def default_button_settings(self, value: ButtonSettings) -> None: ...
    @property
    def default_axis_settings(self) -> AxisSettings:
        """Get the default axis settings."""

    @default_axis_settings.setter
    def default_axis_settings(self, value: AxisSettings) -> None: ...
    @property
    def default_button_axis_settings(self) -> ButtonAxisSettings:
        """Get the default button axis settings."""

    @default_button_axis_settings.setter
    def default_button_axis_settings(self, value: ButtonAxisSettings) -> None: ...
    @property
    def button_settings(self) -> dict[GamepadButton, ButtonSettings]:
        """Per-button overrides of `default_button_settings`.

        Reading returns a copy, so assign the whole mapping back to change it:

            settings.button_settings = {GamepadButton.South(): ButtonSettings(0.9, 0.1)}

        For the value bevy would actually apply, fall back to the default:

            settings.button_settings.get(button, settings.default_button_settings)
        """

    @button_settings.setter
    def button_settings(self, value: dict[GamepadButton, ButtonSettings]) -> None: ...
    @property
    def axis_settings(self) -> dict[GamepadAxis, AxisSettings]:
        """Per-axis overrides of `default_axis_settings`."""

    @axis_settings.setter
    def axis_settings(self, value: dict[GamepadAxis, AxisSettings]) -> None: ...
    @property
    def button_axis_settings(self) -> dict[GamepadButton, ButtonAxisSettings]:
        """Per-button overrides of `default_button_axis_settings`."""

    @button_axis_settings.setter
    def button_axis_settings(
        self, value: dict[GamepadButton, ButtonAxisSettings]
    ) -> None: ...

# Type aliases for Bevy's Event suffix naming convention
