use bevy::{math::DVec2, window::Window};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::component_storage;
use pybevy_math::{PyCompassOctant, PyUVec2, PyVec2};
use pyo3::prelude::*;

use crate::{
    PyCompositeAlphaMode, PyEnabledButtons, PyPresentMode, PyScreenEdge, PyWindowLevel,
    PyWindowMode, PyWindowPosition, PyWindowResizeConstraints, PyWindowResolution, PyWindowTheme,
};

pub const DEFAULT_APP_TITLE: &str = "PyBevy App";

#[component_storage(Window)]
#[pyclass(name = "Window", extends = PyComponent)]
#[derive(Debug, Clone)]
pub struct PyWindow {
    pub(crate) storage: ComponentStorage<Window>,
}

#[pymethods]
impl PyWindow {
    #[new]
    #[pyo3(signature = (
        title = DEFAULT_APP_TITLE.to_string(),
        resolution = PyWindowResolution::default(),
        decorations = true,
        resizable = true,
        mode = PyWindowMode::default(),
        transparent = false,
        window_level = PyWindowLevel::default(),
    ))]
    pub fn new(
        title: String,
        resolution: PyWindowResolution,
        decorations: bool,
        resizable: bool,
        mode: PyWindowMode,
        transparent: bool,
        window_level: PyWindowLevel,
    ) -> PyResult<(Self, PyComponent)> {
        let window = Window {
            title,
            resolution: resolution.try_into()?,
            decorations,
            resizable,
            transparent,
            mode: mode.into(),
            window_level: window_level.into(),
            ..Default::default()
        };

        Ok(Self::from_owned(window))
    }

    #[getter]
    pub fn title(&self) -> PyResult<String> {
        Ok(self.as_ref()?.title.clone())
    }

    #[setter]
    pub fn set_title(&mut self, title: String) -> PyResult<()> {
        self.as_mut()?.title = title;
        Ok(())
    }

    #[getter]
    pub fn resolution(&self) -> PyResult<PyWindowResolution> {
        Ok(self.storage.borrow_field_as(|w| &w.resolution)?)
    }

    #[setter]
    pub fn set_resolution(&mut self, resolution: PyWindowResolution) -> PyResult<()> {
        self.as_mut()?.resolution = resolution.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn focused(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.focused)
    }

    #[getter]
    pub fn decorations(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.decorations)
    }

    #[setter]
    pub fn set_decorations(&mut self, decorations: bool) -> PyResult<()> {
        self.as_mut()?.decorations = decorations;
        Ok(())
    }

    #[getter]
    pub fn resizable(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.resizable)
    }

    #[setter]
    pub fn set_resizable(&mut self, resizable: bool) -> PyResult<()> {
        self.as_mut()?.resizable = resizable;
        Ok(())
    }

    #[getter]
    pub fn mode(&self) -> PyResult<PyWindowMode> {
        Ok(self.as_ref()?.mode.into())
    }

    #[setter]
    pub fn set_mode(&mut self, mode: PyWindowMode) -> PyResult<()> {
        self.as_mut()?.mode = mode.into();
        Ok(())
    }

    #[getter]
    pub fn window_level(&self) -> PyResult<PyWindowLevel> {
        Ok(self.as_ref()?.window_level.into())
    }

    #[setter]
    pub fn set_window_level(&mut self, window_level: PyWindowLevel) -> PyResult<()> {
        self.as_mut()?.window_level = window_level.into();
        Ok(())
    }

    #[getter]
    pub fn transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.transparent)
    }

    #[setter]
    pub fn set_transparent(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.transparent = value;
        Ok(())
    }

    #[getter]
    pub fn visible(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.visible)
    }

    #[setter]
    pub fn set_visible(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.visible = value;
        Ok(())
    }

    #[getter]
    pub fn skip_taskbar(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.skip_taskbar)
    }

    #[setter]
    pub fn set_skip_taskbar(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.skip_taskbar = value;
        Ok(())
    }

    #[getter]
    pub fn clip_children(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.clip_children)
    }

    #[setter]
    pub fn set_clip_children(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.clip_children = value;
        Ok(())
    }

    #[getter]
    pub fn fit_canvas_to_parent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.fit_canvas_to_parent)
    }

    #[setter]
    pub fn set_fit_canvas_to_parent(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.fit_canvas_to_parent = value;
        Ok(())
    }

    #[getter]
    pub fn prevent_default_event_handling(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.prevent_default_event_handling)
    }

    #[setter]
    pub fn set_prevent_default_event_handling(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.prevent_default_event_handling = value;
        Ok(())
    }

    #[getter]
    pub fn ime_enabled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.ime_enabled)
    }

    #[setter]
    pub fn set_ime_enabled(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.ime_enabled = value;
        Ok(())
    }

    #[getter]
    pub fn ime_position(&self) -> PyResult<PyVec2> {
        Ok(self.storage.borrow_field_as(|w| &w.ime_position)?)
    }

    #[setter]
    pub fn set_ime_position(&mut self, value: PyVec2) -> PyResult<()> {
        self.as_mut()?.ime_position = value.into();
        Ok(())
    }

    #[getter]
    pub fn has_shadow(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.has_shadow)
    }

    #[setter]
    pub fn set_has_shadow(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.has_shadow = value;
        Ok(())
    }

    #[getter]
    pub fn movable_by_window_background(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.movable_by_window_background)
    }

    #[setter]
    pub fn set_movable_by_window_background(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.movable_by_window_background = value;
        Ok(())
    }

    #[getter]
    pub fn fullsize_content_view(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.fullsize_content_view)
    }

    #[setter]
    pub fn set_fullsize_content_view(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.fullsize_content_view = value;
        Ok(())
    }

    #[getter]
    pub fn recognize_doubletap_gesture(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.recognize_doubletap_gesture)
    }

    #[setter]
    pub fn set_recognize_doubletap_gesture(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.recognize_doubletap_gesture = value;
        Ok(())
    }

    #[getter]
    pub fn recognize_pinch_gesture(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.recognize_pinch_gesture)
    }

    #[setter]
    pub fn set_recognize_pinch_gesture(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.recognize_pinch_gesture = value;
        Ok(())
    }

    #[getter]
    pub fn recognize_pan_gesture(&self) -> PyResult<Option<(u8, u8)>> {
        Ok(self.as_ref()?.recognize_pan_gesture)
    }

    #[setter]
    pub fn set_recognize_pan_gesture(&mut self, value: Option<(u8, u8)>) -> PyResult<()> {
        self.as_mut()?.recognize_pan_gesture = value;
        Ok(())
    }

    #[getter]
    pub fn prefers_home_indicator_hidden(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.prefers_home_indicator_hidden)
    }

    #[setter]
    pub fn set_prefers_home_indicator_hidden(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.prefers_home_indicator_hidden = value;
        Ok(())
    }

    #[getter]
    pub fn prefers_status_bar_hidden(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.prefers_status_bar_hidden)
    }

    #[setter]
    pub fn set_prefers_status_bar_hidden(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.prefers_status_bar_hidden = value;
        Ok(())
    }

    #[getter]
    pub fn name(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.name.clone())
    }

    #[setter]
    pub fn set_name(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.name = value;
        Ok(())
    }

    #[getter]
    pub fn canvas(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.canvas.clone())
    }

    #[setter]
    pub fn set_canvas(&mut self, value: Option<String>) -> PyResult<()> {
        self.as_mut()?.canvas = value;
        Ok(())
    }

    #[getter]
    pub fn recognize_rotation_gesture(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.recognize_rotation_gesture)
    }

    #[setter]
    pub fn set_recognize_rotation_gesture(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.recognize_rotation_gesture = value;
        Ok(())
    }

    #[getter]
    pub fn titlebar_shown(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.titlebar_shown)
    }

    #[setter]
    pub fn set_titlebar_shown(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.titlebar_shown = value;
        Ok(())
    }

    #[getter]
    pub fn titlebar_transparent(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.titlebar_transparent)
    }

    #[setter]
    pub fn set_titlebar_transparent(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.titlebar_transparent = value;
        Ok(())
    }

    #[getter]
    pub fn titlebar_show_title(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.titlebar_show_title)
    }

    #[setter]
    pub fn set_titlebar_show_title(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.titlebar_show_title = value;
        Ok(())
    }

    #[getter]
    pub fn titlebar_show_buttons(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.titlebar_show_buttons)
    }

    #[setter]
    pub fn set_titlebar_show_buttons(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.titlebar_show_buttons = value;
        Ok(())
    }

    #[getter]
    pub fn window_theme(&self) -> PyResult<Option<PyWindowTheme>> {
        Ok(self.as_ref()?.window_theme.map(Into::into))
    }

    #[setter]
    pub fn set_window_theme(&mut self, value: Option<PyWindowTheme>) -> PyResult<()> {
        self.as_mut()?.window_theme = value.map(Into::into);
        Ok(())
    }

    #[getter]
    pub fn enabled_buttons(&self) -> PyResult<PyEnabledButtons> {
        Ok(self.as_ref()?.enabled_buttons.into())
    }

    #[setter]
    pub fn set_enabled_buttons(&mut self, value: PyEnabledButtons) -> PyResult<()> {
        self.as_mut()?.enabled_buttons = value.into();
        Ok(())
    }

    #[getter]
    pub fn position(&self) -> PyResult<PyWindowPosition> {
        Ok(self.as_ref()?.position.into())
    }

    #[setter]
    pub fn set_position(&mut self, value: PyWindowPosition) -> PyResult<()> {
        self.as_mut()?.position = value.into();
        Ok(())
    }

    #[getter]
    pub fn present_mode(&self) -> PyResult<PyPresentMode> {
        Ok(self.as_ref()?.present_mode.into())
    }

    #[setter]
    pub fn set_present_mode(&mut self, value: PyPresentMode) -> PyResult<()> {
        self.as_mut()?.present_mode = value.into();
        Ok(())
    }

    #[getter]
    pub fn composite_alpha_mode(&self) -> PyResult<PyCompositeAlphaMode> {
        Ok(self.as_ref()?.composite_alpha_mode.into())
    }

    #[setter]
    pub fn set_composite_alpha_mode(&mut self, value: PyCompositeAlphaMode) -> PyResult<()> {
        self.as_mut()?.composite_alpha_mode = value.into();
        Ok(())
    }

    #[getter]
    pub fn resize_constraints(&self) -> PyResult<PyWindowResizeConstraints> {
        Ok(self.as_ref()?.resize_constraints.into())
    }

    #[setter]
    pub fn set_resize_constraints(&mut self, value: PyWindowResizeConstraints) -> PyResult<()> {
        self.as_mut()?.resize_constraints = value.into();
        Ok(())
    }

    #[getter]
    pub fn preferred_screen_edges_deferring_system_gestures(&self) -> PyResult<PyScreenEdge> {
        Ok(self
            .as_ref()?
            .preferred_screen_edges_deferring_system_gestures
            .into())
    }

    #[setter]
    pub fn set_preferred_screen_edges_deferring_system_gestures(
        &mut self,
        value: PyScreenEdge,
    ) -> PyResult<()> {
        self.as_mut()?
            .preferred_screen_edges_deferring_system_gestures = value.into();
        Ok(())
    }

    pub fn width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.resolution.width())
    }

    pub fn height(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.resolution.height())
    }

    pub fn size(&self) -> PyResult<PyVec2> {
        Ok(self.as_ref()?.resolution.size().into())
    }

    pub fn physical_width(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.resolution.physical_width())
    }

    pub fn physical_height(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.resolution.physical_height())
    }

    pub fn physical_size(&self) -> PyResult<PyUVec2> {
        Ok(self.as_ref()?.resolution.physical_size().into())
    }

    pub fn scale_factor(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.resolution.scale_factor())
    }

    pub fn cursor_position(&self) -> PyResult<Option<PyVec2>> {
        Ok(self.as_ref()?.cursor_position().map(|v| v.into()))
    }

    pub fn physical_cursor_position(&self) -> PyResult<Option<PyVec2>> {
        Ok(self.as_ref()?.physical_cursor_position().map(|v| v.into()))
    }

    pub fn set_cursor_position(&mut self, position: Option<PyVec2>) -> PyResult<()> {
        self.as_mut()?
            .set_cursor_position(position.map(|v| v.into()));
        Ok(())
    }

    pub fn set_maximized(&mut self, maximized: bool) -> PyResult<()> {
        self.as_mut()?.set_maximized(maximized);
        Ok(())
    }

    pub fn set_minimized(&mut self, minimized: bool) -> PyResult<()> {
        self.as_mut()?.set_minimized(minimized);
        Ok(())
    }

    pub fn start_drag_move(&mut self) -> PyResult<()> {
        self.as_mut()?.start_drag_move();
        Ok(())
    }

    pub fn start_drag_resize(&mut self, direction: PyCompassOctant) -> PyResult<()> {
        self.as_mut()?.start_drag_resize(direction.into());
        Ok(())
    }

    pub fn set_physical_cursor_position(&mut self, position: Option<(f64, f64)>) -> PyResult<()> {
        self.as_mut()?
            .set_physical_cursor_position(position.map(|(x, y)| DVec2::new(x, y)));
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let window = self.as_ref()?;
        Ok(format!(
            "Window(title='{}', {}x{}, focused={})",
            window.title,
            window.resolution.width(),
            window.resolution.height(),
            window.focused
        ))
    }
}
