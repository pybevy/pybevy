use bevy::ui::{
    AlignContent, AlignItems, AlignSelf, BoxSizing, Display, FlexDirection, FlexWrap, GridAutoFlow,
    InlineDirection, InterpolationColorSpace, JustifyContent, JustifyItems, JustifySelf,
    OverflowAxis, PositionType, VisualBox,
};
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(FlexDirection)]
#[pyclass(name = "FlexDirection", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyFlexDirection {
    Row = 0,
    Column = 1,
    RowReverse = 2,
    ColumnReverse = 3,
}

#[pyenum(Display)]
#[pyclass(name = "Display", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyDisplay {
    Flex = 0,
    Grid = 1,
    Block = 2,
    #[pyo3(name = "None_")]
    None = 3,
}

#[pyenum(AlignItems)]
#[pyclass(name = "AlignItems", eq, eq_int, from_py_object)]
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

#[pyenum(AlignSelf)]
#[pyclass(name = "AlignSelf", eq, eq_int, from_py_object)]
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

#[pyenum(AlignContent)]
#[pyclass(name = "AlignContent", eq, eq_int, from_py_object)]
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

#[pyenum(JustifyContent)]
#[pyclass(name = "JustifyContent", eq, eq_int, from_py_object)]
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

#[pyenum(JustifyItems)]
#[pyclass(name = "JustifyItems", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyJustifyItems {
    Default = 0,
    Start = 1,
    End = 2,
    Center = 3,
    Baseline = 4,
    Stretch = 5,
}

#[pyenum(JustifySelf)]
#[pyclass(name = "JustifySelf", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyJustifySelf {
    Auto = 0,
    Start = 1,
    End = 2,
    Center = 3,
    Baseline = 4,
    Stretch = 5,
}

#[pyenum(PositionType)]
#[pyclass(name = "PositionType", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyPositionType {
    Relative = 0,
    Absolute = 1,
}

#[pyenum(FlexWrap)]
#[pyclass(name = "FlexWrap", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyFlexWrap {
    NoWrap = 0,
    Wrap = 1,
    WrapReverse = 2,
}

#[pyenum(InlineDirection)]
#[pyclass(name = "InlineDirection", eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PyInlineDirection {
    Ltr = 0,
    Rtl = 1,
}

#[pyenum(OverflowAxis)]
#[pyclass(name = "OverflowAxis", eq, eq_int, from_py_object)]
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

#[pyenum(BoxSizing)]
#[pyclass(name = "BoxSizing", eq, frozen, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PyBoxSizing {
    BorderBox,
    ContentBox,
}

#[pyenum(GridAutoFlow)]
#[pyclass(name = "GridAutoFlow", eq, from_py_object)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PyGridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

#[pyenum(InterpolationColorSpace)]
#[pyclass(name = "InterpolationColorSpace", eq, from_py_object)]
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

#[pyenum(VisualBox)]
#[pyclass(name = "VisualBox", eq, frozen, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PyVisualBox {
    ContentBox,
    PaddingBox,
    BorderBox,
}
