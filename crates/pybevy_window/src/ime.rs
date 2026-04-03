use bevy::window::Ime;
use pybevy_core::{PyEntity, PyMessage};
use pybevy_macros::pymessage;
use pyo3::prelude::*;

#[derive(Debug, Clone, PartialEq)]
pub enum ImeData {
    Preedit {
        window: PyEntity,
        value: String,
        cursor: Option<(usize, usize)>,
    },
    Commit {
        window: PyEntity,
        value: String,
    },
    Enabled {
        window: PyEntity,
    },
    Disabled {
        window: PyEntity,
    },
}

#[pymessage(Ime)]
#[pyclass(name = "Ime", extends = PyMessage, eq)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyIme {
    data: ImeData,
}

impl PyIme {
    pub fn from_bevy(event: &Ime) -> (Self, PyMessage) {
        (Self::from(event), PyMessage)
    }
}

impl From<&Ime> for PyIme {
    fn from(event: &Ime) -> Self {
        let data = match event {
            Ime::Preedit {
                window,
                value,
                cursor,
            } => ImeData::Preedit {
                window: (*window).into(),
                value: value.clone(),
                cursor: *cursor,
            },
            Ime::Commit { window, value } => ImeData::Commit {
                window: (*window).into(),
                value: value.clone(),
            },
            Ime::Enabled { window } => ImeData::Enabled {
                window: (*window).into(),
            },
            Ime::Disabled { window } => ImeData::Disabled {
                window: (*window).into(),
            },
        };
        PyIme { data }
    }
}

#[pymethods]
impl PyIme {
    pub fn is_preedit(&self) -> bool {
        matches!(self.data, ImeData::Preedit { .. })
    }

    pub fn is_commit(&self) -> bool {
        matches!(self.data, ImeData::Commit { .. })
    }

    pub fn is_enabled(&self) -> bool {
        matches!(self.data, ImeData::Enabled { .. })
    }

    pub fn is_disabled(&self) -> bool {
        matches!(self.data, ImeData::Disabled { .. })
    }

    #[getter]
    pub fn window(&self) -> PyEntity {
        match &self.data {
            ImeData::Preedit { window, .. } => *window,
            ImeData::Commit { window, .. } => *window,
            ImeData::Enabled { window } => *window,
            ImeData::Disabled { window } => *window,
        }
    }

    #[getter]
    pub fn value(&self) -> Option<String> {
        match &self.data {
            ImeData::Preedit { value, .. } => Some(value.clone()),
            ImeData::Commit { value, .. } => Some(value.clone()),
            _ => None,
        }
    }

    #[getter]
    pub fn cursor(&self) -> Option<(usize, usize)> {
        match &self.data {
            ImeData::Preedit { cursor, .. } => *cursor,
            _ => None,
        }
    }

    pub fn __repr__(&self) -> String {
        match &self.data {
            ImeData::Preedit {
                window,
                value,
                cursor,
            } => {
                format!("Ime.Preedit(window={window:?}, value={value:?}, cursor={cursor:?})")
            }
            ImeData::Commit { window, value } => {
                format!("Ime.Commit(window={window:?}, value={value:?})")
            }
            ImeData::Enabled { window } => format!("Ime.Enabled(window={window:?})"),
            ImeData::Disabled { window } => format!("Ime.Disabled(window={window:?})"),
        }
    }
}
