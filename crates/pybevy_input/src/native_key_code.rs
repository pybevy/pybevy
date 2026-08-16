use bevy::input::keyboard::NativeKeyCode;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(NativeKeyCode, empty_tuple)]
#[pyclass(
    name = "NativeKeyCode",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object,
    hash
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyNativeKeyCode {
    Unidentified(),
    #[py_bevy(tuple)]
    Android {
        value: u32,
    },
    #[py_bevy(tuple)]
    MacOS {
        value: u16,
    },
    #[py_bevy(tuple)]
    Windows {
        value: u16,
    },
    #[py_bevy(tuple)]
    Xkb {
        value: u32,
    },
}

impl PyNativeKeyCode {
    pub(crate) fn repr(&self) -> String {
        match self {
            Self::Unidentified() => "NativeKeyCode.Unidentified()".to_string(),
            Self::Android { value } => format!("NativeKeyCode.Android({value})"),
            Self::MacOS { value } => format!("NativeKeyCode.MacOS({value})"),
            Self::Windows { value } => format!("NativeKeyCode.Windows({value})"),
            Self::Xkb { value } => format!("NativeKeyCode.Xkb({value})"),
        }
    }
}
