use bevy::{
    math::Vec2,
    text::{PreeditCursor, TextEdit},
};
use pybevy_macros::pyenum;
use pybevy_math::vec2::PyVec2;
use pyo3::prelude::*;

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
        value: PyVec2,
    },
    SelectWordAtPoint {
        value: PyVec2,
    },
    SelectLineAtPoint {
        value: PyVec2,
    },
    SelectedHardLineAtPoint {
        value: PyVec2,
    },
    ExtendSelectionToPoint {
        value: PyVec2,
    },
    ShiftClickExtension {
        value: PyVec2,
    },
    #[pyo3(constructor = (*, value, cursor))]
    ImeSetCompose {
        value: String,
        cursor: Option<(usize, usize)>,
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

fn point_repr(value: &PyVec2) -> PyResult<String> {
    let value: Vec2 = value.try_into()?;
    Ok(format!("Vec2({}, {})", value.x, value.y))
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
                value: PyVec2::from_vec2(*value),
            },
            TextEdit::SelectWordAtPoint(value) => Self::SelectWordAtPoint {
                value: PyVec2::from_vec2(*value),
            },
            TextEdit::SelectLineAtPoint(value) => Self::SelectLineAtPoint {
                value: PyVec2::from_vec2(*value),
            },
            TextEdit::SelectedHardLineAtPoint(value) => Self::SelectedHardLineAtPoint {
                value: PyVec2::from_vec2(*value),
            },
            TextEdit::ExtendSelectionToPoint(value) => Self::ExtendSelectionToPoint {
                value: PyVec2::from_vec2(*value),
            },
            TextEdit::ShiftClickExtension(value) => Self::ShiftClickExtension {
                value: PyVec2::from_vec2(*value),
            },
            TextEdit::ImeSetCompose { value, cursor } => Self::ImeSetCompose {
                value: from_smol(value.clone()),
                cursor: cursor.map(|c| (c.anchor, c.focus)),
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
            PyTextEdit::MoveToPoint { value } => TextEdit::MoveToPoint(value.try_into()?),
            PyTextEdit::SelectWordAtPoint { value } => {
                TextEdit::SelectWordAtPoint(value.try_into()?)
            }
            PyTextEdit::SelectLineAtPoint { value } => {
                TextEdit::SelectLineAtPoint(value.try_into()?)
            }
            PyTextEdit::SelectedHardLineAtPoint { value } => {
                TextEdit::SelectedHardLineAtPoint(value.try_into()?)
            }
            PyTextEdit::ExtendSelectionToPoint { value } => {
                TextEdit::ExtendSelectionToPoint(value.try_into()?)
            }
            PyTextEdit::ShiftClickExtension { value } => {
                TextEdit::ShiftClickExtension(value.try_into()?)
            }
            PyTextEdit::ImeSetCompose { value, cursor } => TextEdit::ImeSetCompose {
                value: to_smol(value),
                cursor: cursor.map(|(anchor, focus)| PreeditCursor { anchor, focus }),
            },
            PyTextEdit::ImeCommit { value } => TextEdit::ImeCommit {
                value: to_smol(value),
            },
        })
    }
}

#[pymethods]
impl PyTextEdit {
    pub fn __repr__(&self) -> PyResult<String> {
        Ok(match self {
            PyTextEdit::Copy() => "TextEdit.Copy".into(),
            PyTextEdit::Cut() => "TextEdit.Cut".into(),
            PyTextEdit::Paste() => "TextEdit.Paste".into(),
            PyTextEdit::Insert { value } => format!("TextEdit.Insert({value:?})"),
            PyTextEdit::Backspace() => "TextEdit.Backspace".into(),
            PyTextEdit::BackspaceWord() => "TextEdit.BackspaceWord".into(),
            PyTextEdit::Delete() => "TextEdit.Delete".into(),
            PyTextEdit::DeleteWord() => "TextEdit.DeleteWord".into(),
            PyTextEdit::Left { value } => format!("TextEdit.Left({value})"),
            PyTextEdit::Right { value } => format!("TextEdit.Right({value})"),
            PyTextEdit::WordLeft { value } => format!("TextEdit.WordLeft({value})"),
            PyTextEdit::WordRight { value } => format!("TextEdit.WordRight({value})"),
            PyTextEdit::Up { value } => format!("TextEdit.Up({value})"),
            PyTextEdit::Down { value } => format!("TextEdit.Down({value})"),
            PyTextEdit::TextStart { value } => format!("TextEdit.TextStart({value})"),
            PyTextEdit::TextEnd { value } => format!("TextEdit.TextEnd({value})"),
            PyTextEdit::HardLineStart { value } => format!("TextEdit.HardLineStart({value})"),
            PyTextEdit::HardLineEnd { value } => format!("TextEdit.HardLineEnd({value})"),
            PyTextEdit::LineStart { value } => format!("TextEdit.LineStart({value})"),
            PyTextEdit::LineEnd { value } => format!("TextEdit.LineEnd({value})"),
            PyTextEdit::CollapseSelection() => "TextEdit.CollapseSelection".into(),
            PyTextEdit::SelectAll() => "TextEdit.SelectAll".into(),
            PyTextEdit::SelectAllIfCollapsed() => "TextEdit.SelectAllIfCollapsed".into(),
            PyTextEdit::MoveToPoint { value } => {
                format!("TextEdit.MoveToPoint({})", point_repr(value)?)
            }
            PyTextEdit::SelectWordAtPoint { value } => {
                format!("TextEdit.SelectWordAtPoint({})", point_repr(value)?)
            }
            PyTextEdit::SelectLineAtPoint { value } => {
                format!("TextEdit.SelectLineAtPoint({})", point_repr(value)?)
            }
            PyTextEdit::SelectedHardLineAtPoint { value } => {
                format!("TextEdit.SelectedHardLineAtPoint({})", point_repr(value)?)
            }
            PyTextEdit::ExtendSelectionToPoint { value } => {
                format!("TextEdit.ExtendSelectionToPoint({})", point_repr(value)?)
            }
            PyTextEdit::ShiftClickExtension { value } => {
                format!("TextEdit.ShiftClickExtension({})", point_repr(value)?)
            }
            PyTextEdit::ImeSetCompose { value, cursor } => {
                format!("TextEdit.ImeSetCompose(value={value:?}, cursor={cursor:?})")
            }
            PyTextEdit::ImeCommit { value } => format!("TextEdit.ImeCommit(value={value:?})"),
        })
    }
}
