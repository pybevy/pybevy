use bevy::window::{CursorGrabMode, SystemCursorIcon};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(CursorGrabMode)]
#[pyclass(name = "CursorGrabMode", eq, from_py_object, frozen, hash)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyCursorGrabMode {
    #[pyo3(name = "None_")]
    None,
    Confined,
    Locked,
}

#[pyenum(SystemCursorIcon)]
#[pyclass(name = "SystemCursorIcon", eq, from_py_object, frozen, hash)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PySystemCursorIcon {
    Default,
    ContextMenu,
    Help,
    Pointer,
    Progress,
    Wait,
    Cell,
    Crosshair,
    Text,
    VerticalText,
    Alias,
    Copy,
    Move,
    NoDrop,
    NotAllowed,
    Grab,
    Grabbing,
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
    AllScroll,
    ZoomIn,
    ZoomOut,
}
