pub mod angular_color_stop;
pub mod background_color;
pub mod background_gradient;
pub mod border_color;
pub mod border_gradient;
pub mod border_radius;
pub mod box_shadow;
pub mod color_stop;
pub mod computed_node;
pub mod conic_gradient;
pub mod enums;
pub mod focus_policy;
pub mod gradient;
pub mod grid_placement;
pub mod grid_track;
pub mod image_node;
pub mod interaction;
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
pub mod ui_position;
pub mod ui_rect;
pub mod ui_scale;
pub mod ui_target_camera;
pub mod ui_transform;
pub mod val;
pub mod val2;
pub mod z_index;

pub use angular_color_stop::PyAngularColorStop;
pub use background_color::PyBackgroundColor;
pub use background_gradient::PyBackgroundGradient;
pub use border_color::PyBorderColor;
pub use border_gradient::PyBorderGradient;
pub use border_radius::PyBorderRadius;
pub use box_shadow::PyBoxShadow;
pub use color_stop::PyColorStop;
pub use computed_node::PyComputedNode;
pub use conic_gradient::PyConicGradient;
pub use enums::{
    PyAlignContent, PyAlignItems, PyAlignSelf, PyBoxSizing, PyDisplay, PyFlexDirection, PyFlexWrap,
    PyGridAutoFlow, PyInterpolationColorSpace, PyJustifyContent, PyJustifyItems, PyJustifySelf,
    PyOverflowAxis, PyOverflowClipBox, PyPositionType,
};
pub use focus_policy::PyFocusPolicy;
pub use gradient::PyGradient;
pub use grid_placement::PyGridPlacement;
pub use grid_track::PyGridTrack;
pub use image_node::PyImageNode;
pub use interaction::PyInteraction;
pub use linear_gradient::PyLinearGradient;
pub use markers::{
    PyButton, PyChecked, PyInteractionDisabled, PyIsDefaultUiCamera, PyLabel, PyPressed,
};
pub use node::PyNode;
pub use node_image_mode::PyNodeImageMode;
pub use outline::PyOutline;
pub use overflow::PyOverflow;
pub use overflow_clip_margin::PyOverflowClipMargin;
use pyo3::prelude::*;
pub use radial_gradient::PyRadialGradient;
pub use radial_gradient_shape::PyRadialGradientShape;
pub use relative_cursor_position::PyRelativeCursorPosition;
pub use repeated_grid_track::PyRepeatedGridTrack;
pub use scroll_position::PyScrollPosition;
pub use shadow_style::PyShadowStyle;
pub use text::PyText;
pub use text_shadow::PyTextShadow;
pub use ui_position::PyUiPosition;
pub use ui_rect::PyUiRect;
pub use ui_scale::PyUiScale;
pub use ui_target_camera::PyUiTargetCamera;
pub use ui_transform::PyUiTransform;
pub use val::PyVal;
pub use val2::PyVal2;
pub use z_index::{PyGlobalZIndex, PyZIndex};

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "ui")?;
    m.add_class::<PyBackgroundColor>()?;
    m.add_class::<PyBackgroundGradient>()?;
    m.add_class::<PyBorderColor>()?;
    m.add_class::<PyBorderGradient>()?;
    m.add_class::<PyBoxShadow>()?;
    m.add_class::<PyComputedNode>()?;
    m.add_class::<PyImageNode>()?;
    m.add_class::<PyNode>()?;
    m.add_class::<PyOutline>()?;
    m.add_class::<PyRelativeCursorPosition>()?;
    m.add_class::<PyScrollPosition>()?;
    m.add_class::<PyUiTargetCamera>()?;
    m.add_class::<PyUiTransform>()?;
    m.add_class::<PyText>()?;
    m.add_class::<PyTextShadow>()?;

    m.add_class::<PyLabel>()?;
    m.add_class::<PyChecked>()?;
    m.add_class::<PyPressed>()?;
    m.add_class::<PyInteractionDisabled>()?;
    m.add_class::<PyIsDefaultUiCamera>()?;
    m.add_class::<PyButton>()?;

    m.add_class::<PyFocusPolicy>()?;
    m.add_class::<PyInteraction>()?;
    m.add_class::<PyZIndex>()?;
    m.add_class::<PyGlobalZIndex>()?;

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
    m.add_class::<PyOverflowAxis>()?;
    m.add_class::<PyBoxSizing>()?;
    m.add_class::<PyGridAutoFlow>()?;
    m.add_class::<PyInterpolationColorSpace>()?;
    m.add_class::<PyOverflowClipBox>()?;
    m.add_class::<PyOverflow>()?;
    m.add_class::<PyOverflowClipMargin>()?;
    m.add_class::<PyNodeImageMode>()?;
    m.add_class::<PyGridPlacement>()?;
    m.add_class::<PyRepeatedGridTrack>()?;

    m.add_class::<PyBorderRadius>()?;
    m.add_class::<PyAngularColorStop>()?;
    m.add_class::<PyGridTrack>()?;
    m.add_class::<PyVal>()?;
    m.add_class::<PyUiRect>()?;
    m.add_class::<PyVal2>()?;
    m.add_class::<PyRadialGradient>()?;
    m.add_class::<PyRadialGradientShape>()?;
    m.add_class::<PyShadowStyle>()?;
    m.add_class::<PyColorStop>()?;
    m.add_class::<PyUiPosition>()?;
    m.add_class::<PyConicGradient>()?;
    m.add_class::<PyLinearGradient>()?;
    m.add_class::<PyGradient>()?;

    m.add_class::<PyUiScale>()?;
    parent.add_submodule(&m)
}
