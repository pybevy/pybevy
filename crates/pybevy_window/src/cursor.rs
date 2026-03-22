use bevy::window::{CursorGrabMode, SystemCursorIcon};
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(CursorGrabMode)]
#[pyclass(name = "CursorGrabMode", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyCursorGrabMode {
    #[pyo3(name = "None_")]
    None,
    Confined,
    Locked,
}

#[bevy_enum(SystemCursorIcon)]
#[pyclass(name = "SystemCursorIcon", eq)]
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
