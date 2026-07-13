use std::time::Duration;

use bevy::text::EditableText;
use pybevy_core::{ComponentStorage, PyComponent, duration_from_py};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(EditableText, bridge, no_reflect)]
#[pyclass(name = "EditableText", extends = PyComponent)]
pub struct PyEditableText {
    pub(crate) storage: ComponentStorage<EditableText>,
}

#[pymethods]
impl PyEditableText {
    #[new]
    #[pyo3(signature = (
        text = "",
        cursor_width = 0.2,
        cursor_blink_period = None,
        max_characters = None,
        visible_lines = Some(1.0),
        visible_width = None,
        allow_newlines = false
    ))]
    pub fn new(
        text: &str,
        cursor_width: f32,
        cursor_blink_period: Option<&Bound<'_, PyAny>>,
        max_characters: Option<usize>,
        visible_lines: Option<f32>,
        visible_width: Option<f32>,
        allow_newlines: bool,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut editable = EditableText::new(text);
        editable.cursor_width = cursor_width;
        editable.cursor_blink_period = match cursor_blink_period {
            Some(value) => duration_from_py(value)?,
            None => Duration::from_secs(1),
        };
        editable.max_characters = max_characters;
        editable.visible_lines = visible_lines;
        editable.visible_width = visible_width;
        editable.allow_newlines = allow_newlines;
        Ok(Self::from_owned(editable).into())
    }

    /// The current text content of the editor.
    #[getter]
    pub fn value(&self) -> PyResult<String> {
        Ok(self.as_ref()?.value().to_string())
    }

    #[getter]
    pub fn max_characters(&self) -> PyResult<Option<usize>> {
        Ok(self.as_ref()?.max_characters)
    }

    #[setter]
    pub fn set_max_characters(&mut self, value: Option<usize>) -> PyResult<()> {
        self.as_mut()?.max_characters = value;
        Ok(())
    }

    #[getter]
    pub fn allow_newlines(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.allow_newlines)
    }

    #[setter]
    pub fn set_allow_newlines(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.allow_newlines = value;
        Ok(())
    }

    #[getter]
    pub fn cursor_width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.cursor_width)
    }

    #[setter]
    pub fn set_cursor_width(&mut self, value: f32) -> PyResult<()> {
        self.as_mut()?.cursor_width = value;
        Ok(())
    }

    /// Cursor blink period as a `timedelta` (accepts `timedelta`, `float`/`int` seconds).
    #[getter]
    pub fn cursor_blink_period(&self) -> PyResult<Duration> {
        Ok(self.as_ref()?.cursor_blink_period)
    }

    #[setter]
    pub fn set_cursor_blink_period(&mut self, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.as_mut()?.cursor_blink_period = duration_from_py(value)?;
        Ok(())
    }

    #[getter]
    pub fn visible_lines(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.visible_lines)
    }

    #[setter]
    pub fn set_visible_lines(&mut self, value: Option<f32>) -> PyResult<()> {
        self.as_mut()?.visible_lines = value;
        Ok(())
    }

    #[getter]
    pub fn visible_width(&self) -> PyResult<Option<f32>> {
        Ok(self.as_ref()?.visible_width)
    }

    #[setter]
    pub fn set_visible_width(&mut self, value: Option<f32>) -> PyResult<()> {
        self.as_mut()?.visible_width = value;
        Ok(())
    }
}
