use bevy::input::keyboard::NativeKey;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(NativeKey, manual, empty_tuple)]
#[pyclass(
    name = "NativeKey",
    module = "pybevy.input",
    eq,
    frozen,
    from_py_object,
    hash
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PyNativeKey {
    Unidentified(),
    Android { value: u32 },
    MacOS { value: u16 },
    Windows { value: u16 },
    Xkb { value: u32 },
    Web { value: String },
}

impl From<&NativeKey> for PyNativeKey {
    fn from(value: &NativeKey) -> Self {
        match value {
            NativeKey::Unidentified => Self::Unidentified(),
            NativeKey::Android(value) => Self::Android { value: *value },
            NativeKey::MacOS(value) => Self::MacOS { value: *value },
            NativeKey::Windows(value) => Self::Windows { value: *value },
            NativeKey::Xkb(value) => Self::Xkb { value: *value },
            NativeKey::Web(value) => Self::Web {
                value: value.to_string(),
            },
        }
    }
}

impl From<PyNativeKey> for NativeKey {
    fn from(value: PyNativeKey) -> Self {
        match value {
            PyNativeKey::Unidentified() => Self::Unidentified,
            PyNativeKey::Android { value } => Self::Android(value),
            PyNativeKey::MacOS { value } => Self::MacOS(value),
            PyNativeKey::Windows { value } => Self::Windows(value),
            PyNativeKey::Xkb { value } => Self::Xkb(value),
            PyNativeKey::Web { value } => Self::Web(value.into()),
        }
    }
}

#[pymethods]
impl PyNativeKey {
    pub(crate) fn __repr__(&self) -> String {
        match self {
            Self::Unidentified() => "NativeKey.Unidentified".to_string(),
            Self::Android { value } => format!("NativeKey.Android({value})"),
            Self::MacOS { value } => format!("NativeKey.MacOS({value})"),
            Self::Windows { value } => format!("NativeKey.Windows({value})"),
            Self::Xkb { value } => format!("NativeKey.Xkb({value})"),
            Self::Web { value } => format!("NativeKey.Web({value:?})"),
        }
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }
}
