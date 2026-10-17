use bevy::ui::{Node, Val};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use crate::{
    PyAlignContent, PyAlignItems, PyAlignSelf, PyBoxSizing, PyDisplay, PyFlexDirection, PyFlexWrap,
    PyGridAutoFlow, PyInlineDirection, PyJustifyContent, PyJustifyItems, PyJustifySelf,
    PyOverflowAxis, PyPositionType,
    border_radius::PyBorderRadius,
    grid_placement::PyGridPlacement,
    grid_track::PyGridTrack,
    overflow::PyOverflow,
    overflow_clip_margin::PyOverflowClipMargin,
    repeated_grid_track::PyRepeatedGridTrack,
    ui_rect::PyUiRect,
    val::{IntoVal, PyVal},
};

#[pycomponent(Node, bridge,
    domains = [
        scrollbar_width => [finite],
        left => [finite],
        right => [finite],
        top => [finite],
        bottom => [finite],
        width => [finite],
        height => [finite],
        min_width => [finite],
        min_height => [finite],
        max_width => [finite],
        max_height => [finite],
        aspect_ratio => [finite],
        margin => [finite],
        padding => [finite, non_negative],
        border => [finite, non_negative],
        flex_grow => [finite],
        flex_shrink => [finite],
        flex_basis => [finite],
        row_gap => [finite],
        column_gap => [finite],
    ],
    view_fields = [scrollbar_width, flex_grow, flex_shrink],
)]
#[pyclass(name = "Node", module = "pybevy.ui", extends = PyComponent)]
#[derive(Debug)]
pub struct PyNode {
    pub(crate) storage: ComponentStorage<Node>,
}

#[pymethods]
impl PyNode {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        *,
        display = PyDisplay::Flex,
        box_sizing = PyBoxSizing::BorderBox,
        position_type = PyPositionType::Relative,
        overflow = PyOverflow::py_new(PyOverflowAxis::Visible, PyOverflowAxis::Visible),
        scrollbar_width = 0.0,
        overflow_clip_margin = PyOverflowClipMargin::padding_box(),
        left = IntoVal(Val::Auto),
        right = IntoVal(Val::Auto),
        top = IntoVal(Val::Auto),
        bottom = IntoVal(Val::Auto),
        width = IntoVal(Val::Auto),
        height = IntoVal(Val::Auto),
        min_width = IntoVal(Val::Auto),
        min_height = IntoVal(Val::Auto),
        max_width = IntoVal(Val::Auto),
        max_height = IntoVal(Val::Auto),
        aspect_ratio = None,
        align_items = PyAlignItems::Default,
        justify_items = PyJustifyItems::Default,
        align_self = PyAlignSelf::Auto,
        justify_self = PyJustifySelf::Auto,
        align_content = PyAlignContent::Default,
        justify_content = PyJustifyContent::Default,
        direction = PyInlineDirection::Ltr,
        margin = PyUiRect::default_(),
        padding = PyUiRect::default_(),
        border = PyUiRect::default_(),
        border_radius = PyBorderRadius::zero(),
        flex_direction = PyFlexDirection::Row,
        flex_wrap = PyFlexWrap::NoWrap,
        flex_grow = 0.0,
        flex_shrink = 1.0,
        flex_basis = IntoVal(Val::Auto),
        row_gap = IntoVal(Val::ZERO),
        column_gap = IntoVal(Val::ZERO),
        grid_auto_flow = PyGridAutoFlow::Row,
        grid_template_rows = Vec::new(),
        grid_template_columns = Vec::new(),
        grid_auto_rows = Vec::new(),
        grid_auto_columns = Vec::new(),
        grid_row = PyGridPlacement::default(),
        grid_column = PyGridPlacement::default()
    ))]
    pub fn new(
        display: PyDisplay,
        box_sizing: PyBoxSizing,
        position_type: PyPositionType,
        overflow: PyOverflow,
        scrollbar_width: f32,
        overflow_clip_margin: PyOverflowClipMargin,
        left: IntoVal,
        right: IntoVal,
        top: IntoVal,
        bottom: IntoVal,
        width: IntoVal,
        height: IntoVal,
        min_width: IntoVal,
        min_height: IntoVal,
        max_width: IntoVal,
        max_height: IntoVal,
        aspect_ratio: Option<f32>,
        align_items: PyAlignItems,
        justify_items: PyJustifyItems,
        align_self: PyAlignSelf,
        justify_self: PyJustifySelf,
        align_content: PyAlignContent,
        justify_content: PyJustifyContent,
        direction: PyInlineDirection,
        margin: PyUiRect,
        padding: PyUiRect,
        border: PyUiRect,
        border_radius: PyBorderRadius,
        flex_direction: PyFlexDirection,
        flex_wrap: PyFlexWrap,
        flex_grow: f32,
        flex_shrink: f32,
        flex_basis: IntoVal,
        row_gap: IntoVal,
        column_gap: IntoVal,
        grid_auto_flow: PyGridAutoFlow,
        grid_template_rows: Vec<PyRepeatedGridTrack>,
        grid_template_columns: Vec<PyRepeatedGridTrack>,
        grid_auto_rows: Vec<PyGridTrack>,
        grid_auto_columns: Vec<PyGridTrack>,
        grid_row: PyGridPlacement,
        grid_column: PyGridPlacement,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(Self::from_owned(Node {
            display: display.into(),
            box_sizing: box_sizing.into(),
            position_type: position_type.into(),
            overflow: overflow.try_into()?,
            scrollbar_width: Self::check_scrollbar_width(scrollbar_width)?,
            overflow_clip_margin: overflow_clip_margin.try_into()?,
            left: Self::check_left(left.0)?,
            right: Self::check_right(right.0)?,
            top: Self::check_top(top.0)?,
            bottom: Self::check_bottom(bottom.0)?,
            width: Self::check_width(width.0)?,
            height: Self::check_height(height.0)?,
            min_width: Self::check_min_width(min_width.0)?,
            min_height: Self::check_min_height(min_height.0)?,
            max_width: Self::check_max_width(max_width.0)?,
            max_height: Self::check_max_height(max_height.0)?,
            aspect_ratio: Self::check_aspect_ratio(aspect_ratio)?,
            align_items: align_items.into(),
            justify_items: justify_items.into(),
            align_self: align_self.into(),
            justify_self: justify_self.into(),
            align_content: align_content.into(),
            justify_content: justify_content.into(),
            direction: direction.into(),
            margin: Self::check_margin(margin.try_into()?)?,
            padding: Self::check_padding(padding.try_into()?)?,
            border: Self::check_border(border.try_into()?)?,
            border_radius: border_radius.try_into()?,
            flex_direction: flex_direction.into(),
            flex_wrap: flex_wrap.into(),
            flex_grow: Self::check_flex_grow(flex_grow)?,
            flex_shrink: Self::check_flex_shrink(flex_shrink)?,
            flex_basis: Self::check_flex_basis(flex_basis.0)?,
            row_gap: Self::check_row_gap(row_gap.0)?,
            column_gap: Self::check_column_gap(column_gap.0)?,
            grid_auto_flow: grid_auto_flow.into(),
            grid_template_rows: grid_template_rows.into_iter().map(Into::into).collect(),
            grid_template_columns: grid_template_columns.into_iter().map(Into::into).collect(),
            grid_auto_rows: grid_auto_rows.into_iter().map(Into::into).collect(),
            grid_auto_columns: grid_auto_columns.into_iter().map(Into::into).collect(),
            grid_row: grid_row.into(),
            grid_column: grid_column.into(),
        })
        .into())
    }

    #[setter]
    pub fn set_position_type(&mut self, value: PyPositionType) -> PyResult<()> {
        self.as_mut()?.position_type = value.into();
        Ok(())
    }

    #[getter]
    pub fn position_type(&self) -> PyResult<PyPositionType> {
        Ok(self.as_ref()?.position_type.into())
    }

    #[getter]
    pub fn top(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.top.into())
    }

    #[setter]
    pub fn set_top(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_top(value.0)?;
        self.as_mut()?.top = value;
        Ok(())
    }

    #[getter]
    pub fn left(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.left.into())
    }

    #[setter]
    pub fn set_left(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_left(value.0)?;
        self.as_mut()?.left = value;
        Ok(())
    }

    #[getter]
    pub fn width(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.width.into())
    }

    #[setter]
    pub fn set_width(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_width(value.0)?;
        self.as_mut()?.width = value;
        Ok(())
    }

    #[getter]
    pub fn height(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.height.into())
    }

    #[setter]
    pub fn set_height(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_height(value.0)?;
        self.as_mut()?.height = value;
        Ok(())
    }

    #[getter]
    pub fn right(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.right.into())
    }

    #[setter]
    pub fn set_right(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_right(value.0)?;
        self.as_mut()?.right = value;
        Ok(())
    }

    #[getter]
    pub fn bottom(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.bottom.into())
    }

    #[setter]
    pub fn set_bottom(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_bottom(value.0)?;
        self.as_mut()?.bottom = value;
        Ok(())
    }

    #[getter]
    pub fn min_width(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.min_width.into())
    }

    #[setter]
    pub fn set_min_width(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_min_width(value.0)?;
        self.as_mut()?.min_width = value;
        Ok(())
    }

    #[getter]
    pub fn max_width(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.max_width.into())
    }

    #[setter]
    pub fn set_max_width(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_max_width(value.0)?;
        self.as_mut()?.max_width = value;
        Ok(())
    }

    #[getter]
    pub fn min_height(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.min_height.into())
    }

    #[setter]
    pub fn set_min_height(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_min_height(value.0)?;
        self.as_mut()?.min_height = value;
        Ok(())
    }

    #[getter]
    pub fn max_height(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.max_height.into())
    }

    #[setter]
    pub fn set_max_height(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_max_height(value.0)?;
        self.as_mut()?.max_height = value;
        Ok(())
    }

    #[getter]
    pub fn flex_direction(&self) -> PyResult<PyFlexDirection> {
        Ok(self.as_ref()?.flex_direction.into())
    }

    #[setter]
    pub fn set_flex_direction(&mut self, value: PyFlexDirection) -> PyResult<()> {
        self.as_mut()?.flex_direction = value.into();
        Ok(())
    }

    #[getter]
    pub fn direction(&self) -> PyResult<PyInlineDirection> {
        Ok(self.as_ref()?.direction.into())
    }

    #[setter]
    pub fn set_direction(&mut self, value: PyInlineDirection) -> PyResult<()> {
        self.as_mut()?.direction = value.into();
        Ok(())
    }

    #[getter]
    pub fn display(&self) -> PyResult<PyDisplay> {
        Ok(self.as_ref()?.display.into())
    }

    #[setter]
    pub fn set_display(&mut self, value: PyDisplay) -> PyResult<()> {
        self.as_mut()?.display = value.into();
        Ok(())
    }

    #[getter]
    pub fn align_items(&self) -> PyResult<PyAlignItems> {
        Ok(self.as_ref()?.align_items.into())
    }

    #[setter]
    pub fn set_align_items(&mut self, value: PyAlignItems) -> PyResult<()> {
        self.as_mut()?.align_items = value.into();
        Ok(())
    }

    #[getter]
    pub fn justify_content(&self) -> PyResult<PyJustifyContent> {
        Ok(self.as_ref()?.justify_content.into())
    }

    #[setter]
    pub fn set_justify_content(&mut self, value: PyJustifyContent) -> PyResult<()> {
        self.as_mut()?.justify_content = value.into();
        Ok(())
    }

    #[getter]
    pub fn align_self(&self) -> PyResult<PyAlignSelf> {
        Ok(self.as_ref()?.align_self.into())
    }

    #[setter]
    pub fn set_align_self(&mut self, value: PyAlignSelf) -> PyResult<()> {
        self.as_mut()?.align_self = value.into();
        Ok(())
    }

    #[getter]
    pub fn flex_wrap(&self) -> PyResult<PyFlexWrap> {
        Ok(self.as_ref()?.flex_wrap.into())
    }

    #[setter]
    pub fn set_flex_wrap(&mut self, value: PyFlexWrap) -> PyResult<()> {
        self.as_mut()?.flex_wrap = value.into();
        Ok(())
    }

    #[getter]
    pub fn align_content(&self) -> PyResult<PyAlignContent> {
        Ok(self.as_ref()?.align_content.into())
    }

    #[setter]
    pub fn set_align_content(&mut self, value: PyAlignContent) -> PyResult<()> {
        self.as_mut()?.align_content = value.into();
        Ok(())
    }

    #[getter]
    pub fn justify_items(&self) -> PyResult<PyJustifyItems> {
        Ok(self.as_ref()?.justify_items.into())
    }

    #[setter]
    pub fn set_justify_items(&mut self, value: PyJustifyItems) -> PyResult<()> {
        self.as_mut()?.justify_items = value.into();
        Ok(())
    }

    #[getter]
    pub fn justify_self(&self) -> PyResult<PyJustifySelf> {
        Ok(self.as_ref()?.justify_self.into())
    }

    #[setter]
    pub fn set_justify_self(&mut self, value: PyJustifySelf) -> PyResult<()> {
        self.as_mut()?.justify_self = value.into();
        Ok(())
    }

    #[getter]
    pub fn overflow(&self) -> PyResult<PyOverflow> {
        Ok(self.storage.borrow_field_as(|n| &n.overflow)?)
    }

    #[setter]
    pub fn set_overflow(&mut self, value: PyOverflow) -> PyResult<()> {
        self.as_mut()?.overflow = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn grid_auto_flow(&self) -> PyResult<PyGridAutoFlow> {
        Ok(self.as_ref()?.grid_auto_flow.into())
    }

    #[setter]
    pub fn set_grid_auto_flow(&mut self, value: PyGridAutoFlow) -> PyResult<()> {
        self.as_mut()?.grid_auto_flow = value.into();
        Ok(())
    }

    #[getter]
    pub fn margin(&self) -> PyResult<PyUiRect> {
        Ok(self.storage.borrow_validated_field_as(
            |node| &node.margin,
            |node| &mut node.margin,
            Self::validate_margin,
        )?)
    }

    #[setter]
    pub fn set_margin(&mut self, value: PyUiRect) -> PyResult<()> {
        let value = Self::check_margin(value.try_into()?)?;
        self.as_mut()?.margin = value;
        Ok(())
    }

    #[getter]
    pub fn padding(&self) -> PyResult<PyUiRect> {
        Ok(self.storage.borrow_validated_field_as(
            |node| &node.padding,
            |node| &mut node.padding,
            Self::validate_padding,
        )?)
    }

    #[setter]
    pub fn set_padding(&mut self, value: PyUiRect) -> PyResult<()> {
        let value = Self::check_padding(value.try_into()?)?;
        self.as_mut()?.padding = value;
        Ok(())
    }

    #[getter]
    pub fn border(&self) -> PyResult<PyUiRect> {
        Ok(self.storage.borrow_validated_field_as(
            |node| &node.border,
            |node| &mut node.border,
            Self::validate_border,
        )?)
    }

    #[setter]
    pub fn set_border(&mut self, value: PyUiRect) -> PyResult<()> {
        let value = Self::check_border(value.try_into()?)?;
        self.as_mut()?.border = value;
        Ok(())
    }

    #[getter]
    pub fn flex_grow(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.flex_grow)
    }

    #[setter]
    pub fn set_flex_grow(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_flex_grow(value)?;
        self.as_mut()?.flex_grow = value;
        Ok(())
    }

    #[getter]
    pub fn flex_shrink(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.flex_shrink)
    }

    #[setter]
    pub fn set_flex_shrink(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_flex_shrink(value)?;
        self.as_mut()?.flex_shrink = value;
        Ok(())
    }

    #[getter]
    pub fn flex_basis(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.flex_basis.into())
    }

    #[setter]
    pub fn set_flex_basis(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_flex_basis(value.0)?;
        self.as_mut()?.flex_basis = value;
        Ok(())
    }

    #[getter]
    pub fn row_gap(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.row_gap.into())
    }

    #[setter]
    pub fn set_row_gap(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_row_gap(value.0)?;
        self.as_mut()?.row_gap = value;
        Ok(())
    }

    #[getter]
    pub fn column_gap(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.column_gap.into())
    }

    #[setter]
    pub fn set_column_gap(&mut self, value: IntoVal) -> PyResult<()> {
        let value = Self::check_column_gap(value.0)?;
        self.as_mut()?.column_gap = value;
        Ok(())
    }

    #[getter]
    pub fn aspect_ratio(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.aspect_ratio)
    }

    #[setter]
    pub fn set_aspect_ratio(&mut self, value: Option<f32>) -> PyResult<()> {
        self.as_mut()?.aspect_ratio = Self::check_aspect_ratio(value)?;
        Ok(())
    }

    #[getter]
    pub fn scrollbar_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.scrollbar_width)
    }

    #[setter]
    pub fn set_scrollbar_width(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_scrollbar_width(value)?;
        self.as_mut()?.scrollbar_width = value;
        Ok(())
    }

    #[getter]
    pub fn grid_column(&self) -> PyResult<PyGridPlacement> {
        Ok(self.as_ref()?.grid_column.into())
    }

    #[setter]
    pub fn set_grid_column(&mut self, value: PyGridPlacement) -> PyResult<()> {
        self.as_mut()?.grid_column = value.into();
        Ok(())
    }

    #[getter]
    pub fn grid_row(&self) -> PyResult<PyGridPlacement> {
        Ok(self.as_ref()?.grid_row.into())
    }

    #[setter]
    pub fn set_grid_row(&mut self, value: PyGridPlacement) -> PyResult<()> {
        self.as_mut()?.grid_row = value.into();
        Ok(())
    }

    #[getter]
    pub fn grid_template_columns(&self) -> PyResult<Vec<PyRepeatedGridTrack>> {
        Ok(self
            .as_ref()?
            .grid_template_columns
            .iter()
            .map(|t| t.clone().into())
            .collect())
    }

    #[setter]
    pub fn set_grid_template_columns(&mut self, value: Vec<PyRepeatedGridTrack>) -> PyResult<()> {
        self.as_mut()?.grid_template_columns = value.into_iter().map(|t| t.into()).collect();
        Ok(())
    }

    #[getter]
    pub fn grid_template_rows(&self) -> PyResult<Vec<PyRepeatedGridTrack>> {
        Ok(self
            .as_ref()?
            .grid_template_rows
            .iter()
            .map(|t| t.clone().into())
            .collect())
    }

    #[setter]
    pub fn set_grid_template_rows(&mut self, value: Vec<PyRepeatedGridTrack>) -> PyResult<()> {
        self.as_mut()?.grid_template_rows = value.into_iter().map(|t| t.into()).collect();
        Ok(())
    }

    #[getter]
    pub fn grid_auto_columns(&self) -> PyResult<Vec<PyGridTrack>> {
        Ok(self
            .as_ref()?
            .grid_auto_columns
            .iter()
            .map(|t| (*t).into())
            .collect())
    }

    #[setter]
    pub fn set_grid_auto_columns(&mut self, value: Vec<PyGridTrack>) -> PyResult<()> {
        self.as_mut()?.grid_auto_columns = value.into_iter().map(|t| t.into()).collect();
        Ok(())
    }

    #[getter]
    pub fn grid_auto_rows(&self) -> PyResult<Vec<PyGridTrack>> {
        Ok(self
            .as_ref()?
            .grid_auto_rows
            .iter()
            .copied()
            .map(|t| t.into())
            .collect())
    }

    #[setter]
    pub fn set_grid_auto_rows(&mut self, value: Vec<PyGridTrack>) -> PyResult<()> {
        self.as_mut()?.grid_auto_rows = value.into_iter().map(|t| t.into()).collect();
        Ok(())
    }

    #[getter]
    pub fn box_sizing(&self) -> PyResult<PyBoxSizing> {
        Ok(self.as_ref()?.box_sizing.into())
    }

    #[setter]
    pub fn set_box_sizing(&mut self, value: PyBoxSizing) -> PyResult<()> {
        self.as_mut()?.box_sizing = value.into();
        Ok(())
    }

    #[getter]
    pub fn overflow_clip_margin(&self) -> PyResult<PyOverflowClipMargin> {
        Ok(self
            .storage
            .borrow_field_as(|node| &node.overflow_clip_margin)?)
    }

    #[setter]
    pub fn set_overflow_clip_margin(&mut self, value: PyOverflowClipMargin) -> PyResult<()> {
        self.as_mut()?.overflow_clip_margin = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn border_radius(&self) -> PyResult<crate::border_radius::PyBorderRadius> {
        Ok(self.storage.borrow_field_as(|node| &node.border_radius)?)
    }

    #[setter]
    pub fn set_border_radius(
        &mut self,
        value: crate::border_radius::PyBorderRadius,
    ) -> PyResult<()> {
        self.as_mut()?.border_radius = value.try_into()?;
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
