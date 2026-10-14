pub mod angular_color_stop;
pub mod auto_directional_navigation;
pub mod background_color;
pub mod background_gradient;
pub mod border_color;
pub mod border_gradient;
pub mod border_radius;
pub mod box_shadow;
pub mod color_stop;
pub mod computed_node;
pub mod computed_stack_index;
pub mod conic_gradient;
pub mod corner_radius;
pub mod enums;
pub mod focus_policy;
pub mod gradient;
mod gradient_snapshot;
pub mod grid_placement;
pub mod grid_track;
pub mod grid_track_repetition;
pub mod hovered;
pub mod ignore_scroll;
pub mod image_node;
pub mod linear_gradient;
pub mod markers;
pub mod node;
pub mod node_image_mode;
pub mod outline;
pub mod overflow;
pub mod overflow_clip_margin;
pub mod radial_gradient;
pub mod radial_gradient_shape;
pub mod relative_cursor_position;
pub mod repeated_grid_track;
pub mod scroll_position;
pub mod shadow_style;
pub mod text;
pub mod text_shadow;
pub mod track_sizing_function;
pub mod ui_global_transform;
pub mod ui_position;
pub mod ui_rect;
pub mod ui_scale;
pub mod ui_target_camera;
pub mod ui_transform;
pub mod val;
pub mod val2;
pub mod widgets;
pub mod z_index;

use enums::{
    PyAlignContent, PyAlignItems, PyAlignSelf, PyBoxSizing, PyDisplay, PyFlexDirection, PyFlexWrap,
    PyGridAutoFlow, PyInlineDirection, PyInterpolationColorSpace, PyJustifyContent, PyJustifyItems,
    PyJustifySelf, PyOverflowAxis, PyPositionType, PyVisualBox,
};
use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{
        background_color::PyBackgroundColor,
        border_color::PyBorderColor,
        border_radius::PyBorderRadius,
        corner_radius::PyCornerRadius,
        enums::{
            PyAlignContent, PyAlignItems, PyAlignSelf, PyDisplay, PyFlexDirection, PyFlexWrap,
            PyGridAutoFlow, PyInlineDirection, PyJustifyContent, PyJustifyItems, PyJustifySelf,
            PyPositionType,
        },
        grid_placement::PyGridPlacement,
        grid_track::PyGridTrack,
        image_node::PyImageNode,
        markers::PyLabel,
        node::PyNode,
        outline::PyOutline,
        overflow::PyOverflow,
        text::PyText,
        ui_global_transform::PyUiGlobalTransform,
        ui_position::PyUiPosition,
        ui_rect::PyUiRect,
        ui_scale::PyUiScale,
        ui_target_camera::PyUiTargetCamera,
        ui_transform::PyUiTransform,
        val::PyVal,
        val2::PyVal2,
        z_index::{PyGlobalZIndex, PyZIndex},
    };
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "ui")?;
    m.add_class::<background_color::PyBackgroundColor>()?;
    m.add_class::<background_gradient::PyBackgroundGradient>()?;
    m.add_class::<border_color::PyBorderColor>()?;
    m.add_class::<border_gradient::PyBorderGradient>()?;
    m.add_class::<box_shadow::PyBoxShadow>()?;
    m.add_class::<box_shadow::PyShadowStyles>()?;
    m.add_class::<computed_node::PyComputedNode>()?;
    m.add_class::<computed_stack_index::PyComputedStackIndex>()?;
    m.add_class::<image_node::PyImageNode>()?;
    m.add_class::<node::PyNode>()?;
    m.add_class::<outline::PyOutline>()?;
    m.add_class::<relative_cursor_position::PyRelativeCursorPosition>()?;
    m.add_class::<scroll_position::PyScrollPosition>()?;
    m.add_class::<ui_target_camera::PyUiTargetCamera>()?;
    m.add_class::<ui_global_transform::PyUiGlobalTransform>()?;
    m.add_class::<ui_transform::PyUiTransform>()?;
    m.add_class::<text::PyText>()?;
    m.add_class::<text_shadow::PyTextShadow>()?;

    m.add_class::<markers::PyLabel>()?;
    m.add_class::<markers::PyChecked>()?;
    m.add_class::<markers::PyPressed>()?;
    m.add_class::<markers::PyInteractionDisabled>()?;
    m.add_class::<markers::PyIsDefaultUiCamera>()?;
    m.add_class::<markers::PySelectable>()?;

    m.add_class::<focus_policy::PyFocusPolicy>()?;
    m.add_class::<z_index::PyZIndex>()?;
    m.add_class::<z_index::PyGlobalZIndex>()?;

    m.add_class::<PyFlexDirection>()?;
    m.add_class::<PyDisplay>()?;
    m.add_class::<PyAlignItems>()?;
    m.add_class::<PyAlignSelf>()?;
    m.add_class::<PyAlignContent>()?;
    m.add_class::<PyJustifyContent>()?;
    m.add_class::<PyJustifyItems>()?;
    m.add_class::<PyJustifySelf>()?;
    m.add_class::<PyPositionType>()?;
    m.add_class::<PyFlexWrap>()?;
    m.add_class::<PyInlineDirection>()?;
    m.add_class::<PyOverflowAxis>()?;
    m.add_class::<PyBoxSizing>()?;
    m.add_class::<PyGridAutoFlow>()?;
    m.add_class::<PyInterpolationColorSpace>()?;
    m.add_class::<PyVisualBox>()?;
    m.add_class::<overflow::PyOverflow>()?;
    m.add_class::<overflow_clip_margin::PyOverflowClipMargin>()?;
    m.add_class::<node_image_mode::PyNodeImageMode>()?;
    m.add_class::<grid_placement::PyGridPlacement>()?;
    m.add_class::<repeated_grid_track::PyRepeatedGridTrack>()?;
    m.add_class::<repeated_grid_track::PyRepeatedGridTracks>()?;
    m.add_class::<grid_track_repetition::PyGridTrackRepetition>()?;
    m.add_class::<track_sizing_function::PyMinTrackSizingFunction>()?;
    m.add_class::<track_sizing_function::PyMaxTrackSizingFunction>()?;
    m.add_class::<auto_directional_navigation::PyAutoDirectionalNavigation>()?;
    m.add_class::<ignore_scroll::PyIgnoreScroll>()?;

    m.add_class::<border_radius::PyBorderRadius>()?;
    m.add_class::<corner_radius::PyCornerRadius>()?;
    m.add_class::<angular_color_stop::PyAngularColorStop>()?;
    m.add_class::<grid_track::PyGridTrack>()?;
    m.add_class::<grid_track::PyGridTracks>()?;
    m.add_class::<val::PyVal>()?;
    m.add_function(wrap_pyfunction!(val::py_px, &m)?)?;
    m.add_function(wrap_pyfunction!(val::py_percent, &m)?)?;
    m.add_function(wrap_pyfunction!(val::py_vw, &m)?)?;
    m.add_function(wrap_pyfunction!(val::py_vh, &m)?)?;
    m.add_function(wrap_pyfunction!(val::py_vmin, &m)?)?;
    m.add_function(wrap_pyfunction!(val::py_vmax, &m)?)?;
    m.add_function(wrap_pyfunction!(val::py_em, &m)?)?;
    m.add_function(wrap_pyfunction!(val::py_rem, &m)?)?;
    m.add_class::<ui_rect::PyUiRect>()?;
    m.add_class::<val2::PyVal2>()?;
    m.add_class::<radial_gradient::PyRadialGradient>()?;
    m.add_class::<radial_gradient_shape::PyRadialGradientShape>()?;
    m.add_class::<shadow_style::PyShadowStyle>()?;
    m.add_class::<color_stop::PyColorStop>()?;
    m.add_class::<ui_position::PyUiPosition>()?;
    m.add_class::<conic_gradient::PyConicGradient>()?;
    m.add_class::<linear_gradient::PyLinearGradient>()?;
    m.add_class::<gradient::PyGradient>()?;
    m.add_class::<gradient::PyGradients>()?;

    m.add_class::<ui_scale::PyUiScale>()?;
    parent.add_submodule(&m)?;

    let widgets = PyModule::new(parent.py(), "ui_widgets")?;
    widgets.add_class::<widgets::PyButton>()?;
    widgets.add_class::<widgets::PyTextInput>()?;
    parent.add_submodule(&widgets)?;

    let picking = PyModule::new(parent.py(), "picking")?;
    picking.add_class::<hovered::PyHovered>()?;
    parent.add_submodule(&picking)
}
