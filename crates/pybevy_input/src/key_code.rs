use bevy::input::keyboard::KeyCode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::native_key_code::PyNativeKeyCode;

#[pyenum(KeyCode, manual)]
#[pyclass(
    name = "KeyCode",
    module = "pybevy.input",
    eq,
    frozen,
    subclass,
    from_py_object,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PyKeyCode(pub(crate) KeyCode);

impl PyKeyCode {
    pub fn from_bevy(key_code: KeyCode) -> Self {
        Self(key_code)
    }

    pub fn to_bevy(&self) -> KeyCode {
        self.0
    }
}

#[pymethods]
impl PyKeyCode {
    fn __repr__(&self) -> String {
        match self.0 {
            KeyCode::Unidentified(value) => {
                format!(
                    "KeyCode.Unidentified({})",
                    PyNativeKeyCode::from(value).repr()
                )
            }
            _ => format!(
                "KeyCode.{}",
                self.portable_name().expect("portable key code")
            ),
        }
    }

    fn __str__(&self) -> String {
        match self.0 {
            KeyCode::Unidentified(value) => {
                format!("Unidentified({})", PyNativeKeyCode::from(value).repr())
            }
            _ => self.portable_name().expect("portable key code").to_string(),
        }
    }

    fn __copy__(&self) -> Self {
        *self
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        *self
    }
}

macro_rules! define_key_code_variants {
    ($($variant:ident),* $(,)?) => {
        impl PyKeyCode {
            fn portable_name(&self) -> Option<&'static str> {
                match self.0 {
                    KeyCode::Unidentified(_) => None,
                    $(KeyCode::$variant => Some(stringify!($variant)),)*
                }
            }
        }

        #[pymethods]
        #[allow(non_snake_case)]
        impl PyKeyCode {
            $(
                #[classattr]
                fn $variant() -> Self {
                    Self(KeyCode::$variant)
                }
            )*
        }
    };
}

define_key_code_variants!(
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Equal,
    IntlBackslash,
    IntlRo,
    IntlYen,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Minus,
    Period,
    Quote,
    Semicolon,
    Slash,
    AltLeft,
    AltRight,
    Backspace,
    CapsLock,
    ContextMenu,
    ControlLeft,
    ControlRight,
    Enter,
    SuperLeft,
    SuperRight,
    ShiftLeft,
    ShiftRight,
    Space,
    Tab,
    Convert,
    KanaMode,
    Lang1,
    Lang2,
    Lang3,
    Lang4,
    Lang5,
    NonConvert,
    Delete,
    End,
    Help,
    Home,
    Insert,
    PageDown,
    PageUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    NumLock,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadBackspace,
    NumpadClear,
    NumpadClearEntry,
    NumpadComma,
    NumpadDecimal,
    NumpadDivide,
    NumpadEnter,
    NumpadEqual,
    NumpadHash,
    NumpadMemoryAdd,
    NumpadMemoryClear,
    NumpadMemoryRecall,
    NumpadMemoryStore,
    NumpadMemorySubtract,
    NumpadMultiply,
    NumpadParenLeft,
    NumpadParenRight,
    NumpadStar,
    NumpadSubtract,
    Escape,
    Fn,
    FnLock,
    PrintScreen,
    ScrollLock,
    Pause,
    BrowserBack,
    BrowserFavorites,
    BrowserForward,
    BrowserHome,
    BrowserRefresh,
    BrowserSearch,
    BrowserStop,
    Eject,
    LaunchApp1,
    LaunchApp2,
    LaunchMail,
    MediaPlayPause,
    MediaSelect,
    MediaStop,
    MediaTrackNext,
    MediaTrackPrevious,
    Power,
    Sleep,
    AudioVolumeDown,
    AudioVolumeMute,
    AudioVolumeUp,
    WakeUp,
    Meta,
    Hyper,
    Turbo,
    Abort,
    Resume,
    Suspend,
    Again,
    Copy,
    Cut,
    Find,
    Open,
    Paste,
    Props,
    Select,
    Undo,
    Hiragana,
    Katakana,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,
);

#[pyclass(
    name = "Unidentified",
    module = "pybevy.input",
    extends = PyKeyCode,
    frozen
)]
pub struct PyKeyCodeUnidentified;

#[pymethods]
#[allow(non_upper_case_globals)]
impl PyKeyCodeUnidentified {
    #[classattr]
    const __qualname__: &'static str = "KeyCode.Unidentified";

    #[classattr]
    fn __match_args__() -> (&'static str,) {
        ("value",)
    }

    #[new]
    pub fn new(value: PyNativeKeyCode) -> PyClassInitializer<Self> {
        PyClassInitializer::from(PyKeyCode(KeyCode::Unidentified(value.into()))).add_subclass(Self)
    }

    #[getter]
    pub fn value(slf: PyRef<'_, Self>) -> PyNativeKeyCode {
        let base = slf.into_super();
        match base.0 {
            KeyCode::Unidentified(value) => value.into(),
            _ => unreachable!("KeyCode.Unidentified changed discriminant"),
        }
    }
}

pub fn materialize_key_code(py: Python<'_>, key_code: KeyCode) -> PyResult<Py<PyAny>> {
    match key_code {
        KeyCode::Unidentified(_) => Ok(Py::new(
            py,
            PyClassInitializer::from(PyKeyCode(key_code)).add_subclass(PyKeyCodeUnidentified),
        )?
        .into_any()),
        _ => Ok(Py::new(py, PyKeyCode(key_code))?.into_any()),
    }
}

pub fn register_key_code_variants(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    module
        .getattr("KeyCode")?
        .setattr("Unidentified", py.get_type::<PyKeyCodeUnidentified>())
}
