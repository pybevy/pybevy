use bevy::{
    math::Vec2,
    text::{PreeditCursor, TextEdit},
};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pybevy_macros::{pyenum, pyvalue};
use pybevy_math::snapshot::Snapshot;
use pyo3::{prelude::*, types::PyString};

#[pyvalue]
#[pyclass(
    name = "PreeditCursor",
    module = "pybevy.text",
    eq,
    skip_from_py_object
)]
#[derive(Debug, Clone, PartialEq)]
pub struct PyPreeditCursor {
    pub(crate) storage: ValueStorage<PreeditCursor>,
}

impl From<PreeditCursor> for PyPreeditCursor {
    fn from(value: PreeditCursor) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyPreeditCursor> for PreeditCursor {
    type Error = PyErr;

    fn try_from(value: PyPreeditCursor) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyPreeditCursor> for PreeditCursor {
    type Error = PyErr;

    fn try_from(value: &PyPreeditCursor) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl PyPreeditCursor {
    fn snapshot(value: PreeditCursor) -> Self {
        Self::from_borrowed(ValueStorage::read_only_snapshot(value))
    }
}

// By-value extraction only feeds the frozen TextEdit payload, so it stores a read-only copy.
impl<'a, 'py> FromPyObject<'a, 'py> for PyPreeditCursor {
    type Error = PyErr;

    fn extract(obj: Borrowed<'a, 'py, PyAny>) -> PyResult<Self> {
        let cursor = obj.cast::<Self>()?.borrow().to_bevy()?;
        Ok(Self::snapshot(cursor))
    }
}

#[pymethods]
impl PyPreeditCursor {
    #[new]
    #[pyo3(signature = (*, anchor, focus))]
    pub fn new(anchor: usize, focus: usize) -> Self {
        Self::from_owned(PreeditCursor { anchor, focus })
    }

    #[getter]
    pub fn anchor(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.anchor)
    }

    #[setter]
    pub fn set_anchor(&mut self, value: usize) -> PyResult<()> {
        self.storage.as_mut()?.anchor = value;
        Ok(())
    }

    #[getter]
    pub fn focus(&self) -> PyResult<usize> {
        Ok(self.storage.as_ref()?.focus)
    }

    #[setter]
    pub fn set_focus(&mut self, value: usize) -> PyResult<()> {
        self.storage.as_mut()?.focus = value;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let cursor = self.to_bevy()?;
        Ok(format!(
            "PreeditCursor(anchor={}, focus={})",
            cursor.anchor, cursor.focus
        ))
    }
}

#[pyenum(TextEdit, manual)]
#[pyclass(name = "TextEdit", module = "pybevy.text", eq, frozen, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub enum PyTextEdit {
    Copy(),
    Cut(),
    Paste(),
    Insert {
        value: String,
    },
    Backspace(),
    BackspaceWord(),
    Delete(),
    DeleteWord(),
    Left {
        value: bool,
    },
    Right {
        value: bool,
    },
    WordLeft {
        value: bool,
    },
    WordRight {
        value: bool,
    },
    Up {
        value: bool,
    },
    Down {
        value: bool,
    },
    TextStart {
        value: bool,
    },
    TextEnd {
        value: bool,
    },
    HardLineStart {
        value: bool,
    },
    HardLineEnd {
        value: bool,
    },
    LineStart {
        value: bool,
    },
    LineEnd {
        value: bool,
    },
    CollapseSelection(),
    SelectAll(),
    SelectAllIfCollapsed(),
    MoveToPoint {
        value: Snapshot<Vec2>,
    },
    SelectWordAtPoint {
        value: Snapshot<Vec2>,
    },
    SelectLineAtPoint {
        value: Snapshot<Vec2>,
    },
    SelectedHardLineAtPoint {
        value: Snapshot<Vec2>,
    },
    ExtendSelectionToPoint {
        value: Snapshot<Vec2>,
    },
    ShiftClickExtension {
        value: Snapshot<Vec2>,
    },
    #[pyo3(constructor = (*, value, cursor))]
    ImeSetCompose {
        value: String,
        cursor: Option<PyPreeditCursor>,
    },
    #[pyo3(constructor = (*, value))]
    ImeCommit {
        value: String,
    },
}

fn from_smol(value: smol_str::SmolStr) -> String {
    value.to_string()
}

fn to_smol(value: String) -> smol_str::SmolStr {
    smol_str::SmolStr::from(value)
}

fn point_repr(py: Python<'_>, value: &Snapshot<Vec2>) -> PyResult<String> {
    Ok(Bound::new(py, value.into_wrapper())?
        .repr()?
        .to_str()?
        .to_owned())
}

fn str_repr(py: Python<'_>, value: &str) -> PyResult<String> {
    Ok(PyString::new(py, value).repr()?.to_str()?.to_owned())
}

fn bool_variant_repr(name: &str, value: bool) -> String {
    let value = if value { "True" } else { "False" };
    format!("TextEdit.{name}({value})")
}

impl From<&TextEdit> for PyTextEdit {
    fn from(edit: &TextEdit) -> Self {
        match edit {
            TextEdit::Copy => Self::Copy(),
            TextEdit::Cut => Self::Cut(),
            TextEdit::Paste => Self::Paste(),
            TextEdit::Insert(value) => Self::Insert {
                value: from_smol(value.clone()),
            },
            TextEdit::Backspace => Self::Backspace(),
            TextEdit::BackspaceWord => Self::BackspaceWord(),
            TextEdit::Delete => Self::Delete(),
            TextEdit::DeleteWord => Self::DeleteWord(),
            TextEdit::Left(value) => Self::Left { value: *value },
            TextEdit::Right(value) => Self::Right { value: *value },
            TextEdit::WordLeft(value) => Self::WordLeft { value: *value },
            TextEdit::WordRight(value) => Self::WordRight { value: *value },
            TextEdit::Up(value) => Self::Up { value: *value },
            TextEdit::Down(value) => Self::Down { value: *value },
            TextEdit::TextStart(value) => Self::TextStart { value: *value },
            TextEdit::TextEnd(value) => Self::TextEnd { value: *value },
            TextEdit::HardLineStart(value) => Self::HardLineStart { value: *value },
            TextEdit::HardLineEnd(value) => Self::HardLineEnd { value: *value },
            TextEdit::LineStart(value) => Self::LineStart { value: *value },
            TextEdit::LineEnd(value) => Self::LineEnd { value: *value },
            TextEdit::CollapseSelection => Self::CollapseSelection(),
            TextEdit::SelectAll => Self::SelectAll(),
            TextEdit::SelectAllIfCollapsed => Self::SelectAllIfCollapsed(),
            TextEdit::MoveToPoint(value) => Self::MoveToPoint {
                value: Snapshot(*value),
            },
            TextEdit::SelectWordAtPoint(value) => Self::SelectWordAtPoint {
                value: Snapshot(*value),
            },
            TextEdit::SelectLineAtPoint(value) => Self::SelectLineAtPoint {
                value: Snapshot(*value),
            },
            TextEdit::SelectedHardLineAtPoint(value) => Self::SelectedHardLineAtPoint {
                value: Snapshot(*value),
            },
            TextEdit::ExtendSelectionToPoint(value) => Self::ExtendSelectionToPoint {
                value: Snapshot(*value),
            },
            TextEdit::ShiftClickExtension(value) => Self::ShiftClickExtension {
                value: Snapshot(*value),
            },
            TextEdit::ImeSetCompose { value, cursor } => Self::ImeSetCompose {
                value: from_smol(value.clone()),
                cursor: cursor.map(PyPreeditCursor::snapshot),
            },
            TextEdit::ImeCommit { value } => Self::ImeCommit {
                value: from_smol(value.clone()),
            },
        }
    }
}

impl TryFrom<PyTextEdit> for TextEdit {
    type Error = PyErr;

    fn try_from(edit: PyTextEdit) -> PyResult<Self> {
        Ok(match edit {
            PyTextEdit::Copy() => TextEdit::Copy,
            PyTextEdit::Cut() => TextEdit::Cut,
            PyTextEdit::Paste() => TextEdit::Paste,
            PyTextEdit::Insert { value } => TextEdit::Insert(to_smol(value)),
            PyTextEdit::Backspace() => TextEdit::Backspace,
            PyTextEdit::BackspaceWord() => TextEdit::BackspaceWord,
            PyTextEdit::Delete() => TextEdit::Delete,
            PyTextEdit::DeleteWord() => TextEdit::DeleteWord,
            PyTextEdit::Left { value } => TextEdit::Left(value),
            PyTextEdit::Right { value } => TextEdit::Right(value),
            PyTextEdit::WordLeft { value } => TextEdit::WordLeft(value),
            PyTextEdit::WordRight { value } => TextEdit::WordRight(value),
            PyTextEdit::Up { value } => TextEdit::Up(value),
            PyTextEdit::Down { value } => TextEdit::Down(value),
            PyTextEdit::TextStart { value } => TextEdit::TextStart(value),
            PyTextEdit::TextEnd { value } => TextEdit::TextEnd(value),
            PyTextEdit::HardLineStart { value } => TextEdit::HardLineStart(value),
            PyTextEdit::HardLineEnd { value } => TextEdit::HardLineEnd(value),
            PyTextEdit::LineStart { value } => TextEdit::LineStart(value),
            PyTextEdit::LineEnd { value } => TextEdit::LineEnd(value),
            PyTextEdit::CollapseSelection() => TextEdit::CollapseSelection,
            PyTextEdit::SelectAll() => TextEdit::SelectAll,
            PyTextEdit::SelectAllIfCollapsed() => TextEdit::SelectAllIfCollapsed,
            PyTextEdit::MoveToPoint { value } => TextEdit::MoveToPoint(value.0),
            PyTextEdit::SelectWordAtPoint { value } => TextEdit::SelectWordAtPoint(value.0),
            PyTextEdit::SelectLineAtPoint { value } => TextEdit::SelectLineAtPoint(value.0),
            PyTextEdit::SelectedHardLineAtPoint { value } => {
                TextEdit::SelectedHardLineAtPoint(value.0)
            }
            PyTextEdit::ExtendSelectionToPoint { value } => {
                TextEdit::ExtendSelectionToPoint(value.0)
            }
            PyTextEdit::ShiftClickExtension { value } => TextEdit::ShiftClickExtension(value.0),
            PyTextEdit::ImeSetCompose { value, cursor } => TextEdit::ImeSetCompose {
                value: to_smol(value),
                cursor: cursor.map(PreeditCursor::try_from).transpose()?,
            },
            PyTextEdit::ImeCommit { value } => TextEdit::ImeCommit {
                value: to_smol(value),
            },
        })
    }
}

#[pymethods]
impl PyTextEdit {
    pub fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        Ok(match self {
            PyTextEdit::Copy() => "TextEdit.Copy".into(),
            PyTextEdit::Cut() => "TextEdit.Cut".into(),
            PyTextEdit::Paste() => "TextEdit.Paste".into(),
            PyTextEdit::Insert { value } => format!("TextEdit.Insert({})", str_repr(py, value)?),
            PyTextEdit::Backspace() => "TextEdit.Backspace".into(),
            PyTextEdit::BackspaceWord() => "TextEdit.BackspaceWord".into(),
            PyTextEdit::Delete() => "TextEdit.Delete".into(),
            PyTextEdit::DeleteWord() => "TextEdit.DeleteWord".into(),
            PyTextEdit::Left { value } => bool_variant_repr("Left", *value),
            PyTextEdit::Right { value } => bool_variant_repr("Right", *value),
            PyTextEdit::WordLeft { value } => bool_variant_repr("WordLeft", *value),
            PyTextEdit::WordRight { value } => bool_variant_repr("WordRight", *value),
            PyTextEdit::Up { value } => bool_variant_repr("Up", *value),
            PyTextEdit::Down { value } => bool_variant_repr("Down", *value),
            PyTextEdit::TextStart { value } => bool_variant_repr("TextStart", *value),
            PyTextEdit::TextEnd { value } => bool_variant_repr("TextEnd", *value),
            PyTextEdit::HardLineStart { value } => bool_variant_repr("HardLineStart", *value),
            PyTextEdit::HardLineEnd { value } => bool_variant_repr("HardLineEnd", *value),
            PyTextEdit::LineStart { value } => bool_variant_repr("LineStart", *value),
            PyTextEdit::LineEnd { value } => bool_variant_repr("LineEnd", *value),
            PyTextEdit::CollapseSelection() => "TextEdit.CollapseSelection".into(),
            PyTextEdit::SelectAll() => "TextEdit.SelectAll".into(),
            PyTextEdit::SelectAllIfCollapsed() => "TextEdit.SelectAllIfCollapsed".into(),
            PyTextEdit::MoveToPoint { value } => {
                format!("TextEdit.MoveToPoint({})", point_repr(py, value)?)
            }
            PyTextEdit::SelectWordAtPoint { value } => {
                format!("TextEdit.SelectWordAtPoint({})", point_repr(py, value)?)
            }
            PyTextEdit::SelectLineAtPoint { value } => {
                format!("TextEdit.SelectLineAtPoint({})", point_repr(py, value)?)
            }
            PyTextEdit::SelectedHardLineAtPoint { value } => {
                format!(
                    "TextEdit.SelectedHardLineAtPoint({})",
                    point_repr(py, value)?
                )
            }
            PyTextEdit::ExtendSelectionToPoint { value } => {
                format!(
                    "TextEdit.ExtendSelectionToPoint({})",
                    point_repr(py, value)?
                )
            }
            PyTextEdit::ShiftClickExtension { value } => {
                format!("TextEdit.ShiftClickExtension({})", point_repr(py, value)?)
            }
            PyTextEdit::ImeSetCompose { value, cursor } => {
                let cursor = match cursor {
                    Some(cursor) => cursor.__repr__()?,
                    None => "None".into(),
                };
                format!(
                    "TextEdit.ImeSetCompose(value={}, cursor={cursor})",
                    str_repr(py, value)?
                )
            }
            PyTextEdit::ImeCommit { value } => {
                format!("TextEdit.ImeCommit(value={})", str_repr(py, value)?)
            }
        })
    }
}
