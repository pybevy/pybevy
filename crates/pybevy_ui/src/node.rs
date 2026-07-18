use bevy::ui::Node;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

use crate::{
    PyAlignContent, PyAlignItems, PyAlignSelf, PyBoxSizing, PyDisplay, PyFlexDirection, PyFlexWrap,
    PyGridAutoFlow, PyInlineDirection, PyJustifyContent, PyJustifyItems, PyJustifySelf,
    PyPositionType,
    grid_placement::PyGridPlacement,
    grid_track::PyGridTrack,
    overflow::PyOverflow,
    overflow_clip_margin::PyOverflowClipMargin,
    repeated_grid_track::PyRepeatedGridTrack,
    ui_rect::PyUiRect,
    val::{PyVal, extract_val_from_any},
};

#[pycomponent(Node, bridge)]
#[pyclass(name = "Node", extends = PyComponent)]
#[derive(Debug)]
pub struct PyNode {
    pub(crate) storage: ComponentStorage<Node>,
}

#[pymethods]
impl PyNode {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        Self::from_owned(Node::default()).into()
    }

    #[setter]
    pub fn set_position_type(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        // Accept both PyPositionType enum and raw integer for backward compatibility
        let pos = if let Ok(v) = value.extract::<PyPositionType>() {
            v.into()
        } else if let Ok(v) = value.extract::<i64>() {
            match v {
                0 => bevy::ui::PositionType::Relative,
                1 => bevy::ui::PositionType::Absolute,
                _ => {
                    return Err(PyValueError::new_err(format!(
                        "invalid PositionType value: {v}, expected 0 (Relative) or 1 (Absolute)"
                    )));
                }
            }
        } else {
            return Err(PyTypeError::new_err(
                "position_type accepts PositionType enum or integer (0=Relative, 1=Absolute)",
            ));
        };
        self.as_mut()?.position_type = pos;
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
    pub fn set_top(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.top = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn left(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.left.into())
    }

    #[setter]
    pub fn set_left(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.left = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn width(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.width.into())
    }

    #[setter]
    pub fn set_width(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.width = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn height(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.height.into())
    }

    #[setter]
    pub fn set_height(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.height = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn right(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.right.into())
    }

    #[setter]
    pub fn set_right(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.right = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn bottom(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.bottom.into())
    }

    #[setter]
    pub fn set_bottom(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.bottom = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn min_width(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.min_width.into())
    }

    #[setter]
    pub fn set_min_width(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.min_width = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn max_width(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.max_width.into())
    }

    #[setter]
    pub fn set_max_width(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.max_width = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn min_height(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.min_height.into())
    }

    #[setter]
    pub fn set_min_height(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.min_height = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn max_height(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.max_height.into())
    }

    #[setter]
    pub fn set_max_height(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.max_height = extract_val_from_any(value)?;
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
        self.as_mut()?.overflow = value.into();
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
        Ok(self.as_ref()?.margin.into())
    }

    #[setter]
    pub fn set_margin(&mut self, value: PyUiRect) -> PyResult<()> {
        self.as_mut()?.margin = value.into();
        Ok(())
    }

    #[getter]
    pub fn padding(&self) -> PyResult<PyUiRect> {
        Ok(self.as_ref()?.padding.into())
    }

    #[setter]
    pub fn set_padding(&mut self, value: PyUiRect) -> PyResult<()> {
        self.as_mut()?.padding = value.into();
        Ok(())
    }

    #[getter]
    pub fn border(&self) -> PyResult<PyUiRect> {
        Ok(self.as_ref()?.border.into())
    }

    #[setter]
    pub fn set_border(&mut self, value: PyUiRect) -> PyResult<()> {
        self.as_mut()?.border = value.into();
        Ok(())
    }

    #[getter]
    pub fn flex_grow(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.flex_grow)
    }

    #[setter]
    pub fn set_flex_grow(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.flex_grow = value;
        Ok(())
    }

    #[getter]
    pub fn flex_shrink(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.flex_shrink)
    }

    #[setter]
    pub fn set_flex_shrink(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.flex_shrink = value;
        Ok(())
    }

    #[getter]
    pub fn flex_basis(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.flex_basis.into())
    }

    #[setter]
    pub fn set_flex_basis(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.flex_basis = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn row_gap(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.row_gap.into())
    }

    #[setter]
    pub fn set_row_gap(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.row_gap = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn column_gap(&self) -> PyResult<PyVal> {
        Ok(self.as_ref()?.column_gap.into())
    }

    #[setter]
    pub fn set_column_gap(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.column_gap = extract_val_from_any(value)?;
        Ok(())
    }

    #[getter]
    pub fn aspect_ratio(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.aspect_ratio)
    }

    #[setter]
    pub fn set_aspect_ratio(&mut self, value: Option<f32>) -> PyResult<()> {
        self.as_mut()?.aspect_ratio = value;
        Ok(())
    }

    #[getter]
    pub fn scrollbar_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.scrollbar_width)
    }

    #[setter]
    pub fn set_scrollbar_width(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.scrollbar_width = value;
        Ok(())
    }

    // Grid layout fields

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
        Ok(self.as_ref()?.overflow_clip_margin.into())
    }

    #[setter]
    pub fn set_overflow_clip_margin(&mut self, value: PyOverflowClipMargin) -> PyResult<()> {
        self.as_mut()?.overflow_clip_margin = value.into();
        Ok(())
    }

    #[getter]
    pub fn border_radius(&self) -> PyResult<crate::border_radius::PyBorderRadius> {
        Ok(self.as_ref()?.border_radius.into())
    }

    #[setter]
    pub fn set_border_radius(
        &mut self,
        value: crate::border_radius::PyBorderRadius,
    ) -> PyResult<()> {
        self.as_mut()?.border_radius = value.into();
        Ok(())
    }
}
