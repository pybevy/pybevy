use bevy::ui::{
    AlignContent, AlignItems, AlignSelf, BoxSizing, Display, FlexDirection, FlexWrap, GridAutoFlow,
    InterpolationColorSpace, JustifyContent, JustifyItems, JustifySelf, OverflowAxis,
    OverflowClipBox, PositionType,
};
use pybevy_macros::bevy_enum;
use pyo3::prelude::*;

#[bevy_enum(FlexDirection)]
#[pyclass(name = "FlexDirection", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyFlexDirection {
    Row = 0,
    Column = 1,
    RowReverse = 2,
    ColumnReverse = 3,
}

#[bevy_enum(Display)]
#[pyclass(name = "Display", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyDisplay {
    Flex = 0,
    Grid = 1,
    Block = 2,
    #[pyo3(name = "None_")]
    None = 3,
}

#[bevy_enum(AlignItems)]
#[pyclass(name = "AlignItems", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PyAlignItems {
    #[default]
    Default = 0,
    Start = 1,
    End = 2,
    FlexStart = 3,
    FlexEnd = 4,
    Center = 5,
    Baseline = 6,
    Stretch = 7,
}

#[bevy_enum(AlignSelf)]
#[pyclass(name = "AlignSelf", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyAlignSelf {
    Auto = 0,
    Start = 1,
    End = 2,
    FlexStart = 3,
    FlexEnd = 4,
    Center = 5,
    Baseline = 6,
    Stretch = 7,
}

#[bevy_enum(AlignContent)]
#[pyclass(name = "AlignContent", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PyAlignContent {
    #[default]
    Default = 0,
    Start = 1,
    End = 2,
    FlexStart = 3,
    FlexEnd = 4,
    Center = 5,
    Stretch = 6,
    SpaceBetween = 7,
    SpaceEvenly = 8,
    SpaceAround = 9,
}

#[bevy_enum(JustifyContent)]
#[pyclass(name = "JustifyContent", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyJustifyContent {
    Default = 0,
    Start = 1,
    End = 2,
    FlexStart = 3,
    FlexEnd = 4,
    Center = 5,
    SpaceBetween = 6,
    SpaceAround = 7,
    SpaceEvenly = 8,
    Stretch = 9,
}

#[bevy_enum(JustifyItems)]
#[pyclass(name = "JustifyItems", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyJustifyItems {
    Default = 0,
    Start = 1,
    End = 2,
    Center = 3,
    Baseline = 4,
    Stretch = 5,
}

#[bevy_enum(JustifySelf)]
#[pyclass(name = "JustifySelf", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyJustifySelf {
    Auto = 0,
    Start = 1,
    End = 2,
    Center = 3,
    Baseline = 4,
    Stretch = 5,
}

#[bevy_enum(PositionType)]
#[pyclass(name = "PositionType", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyPositionType {
    Relative = 0,
    Absolute = 1,
}

#[bevy_enum(FlexWrap)]
#[pyclass(name = "FlexWrap", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyFlexWrap {
    NoWrap = 0,
    Wrap = 1,
    WrapReverse = 2,
}

#[bevy_enum(OverflowAxis)]
#[pyclass(name = "OverflowAxis", eq, eq_int)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyOverflowAxis {
    Visible = 0,
    Clip = 1,
    Hidden = 2,
    Scroll = 3,
}

#[pymethods]
impl PyOverflowAxis {
    pub fn is_visible(&self) -> bool {
        matches!(self, PyOverflowAxis::Visible)
    }

}

#[bevy_enum(BoxSizing)]
#[pyclass(name = "BoxSizing", eq, frozen)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PyBoxSizing {
    BorderBox,
    ContentBox,
}

#[bevy_enum(GridAutoFlow)]
#[pyclass(name = "GridAutoFlow", eq)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PyGridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[bevy_enum(InterpolationColorSpace)]
#[pyclass(name = "InterpolationColorSpace", eq)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyInterpolationColorSpace {
    Oklaba,
    Oklcha,
    OklchaLong,
    Srgba,
    LinearRgba,
    Hsla,
    HslaLong,
    Hsva,
    HsvaLong,
}

#[bevy_enum(OverflowClipBox)]
#[pyclass(name = "OverflowClipBox", eq, frozen)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PyOverflowClipBox {
    ContentBox,
    PaddingBox,
    BorderBox,
}
