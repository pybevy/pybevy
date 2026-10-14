use bevy::{color::Color, text::TextCursorStyle};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(
    TextCursorStyle,
    bridge,
    no_reflect,
    view_fields = [selection_radius],
    batch_only_fields = [color, selection_color, unfocused_selection_color],
    domains = [selection_radius => [finite]]
)]
#[pyclass(name = "TextCursorStyle", module = "pybevy.text", extends = PyComponent)]
#[derive(Debug)]
pub struct PyTextCursorStyle {
    pub(crate) storage: ComponentStorage<TextCursorStyle>,
}

impl PyTextCursorStyle {
    fn default_color() -> PyColor {
        TextCursorStyle::default().color.into()
    }

    fn default_selection_color() -> PyColor {
        TextCursorStyle::default().selection_color.into()
    }

    fn default_unfocused_selection_color() -> PyColor {
        TextCursorStyle::default().unfocused_selection_color.into()
    }
}

#[pymethods]
impl PyTextCursorStyle {
    #[new]
    #[pyo3(signature = (
        *,
        color = Self::default_color(),
        selection_color = Self::default_selection_color(),
        unfocused_selection_color = Self::default_unfocused_selection_color(),
        selected_text_color = None,
        selection_radius = 0.0
    ))]
    pub fn new(
        color: PyColor,
        selection_color: PyColor,
        unfocused_selection_color: PyColor,
        selected_text_color: Option<PyColor>,
        selection_radius: f32,
    ) -> PyResult<PyClassInitializer<Self>> {
        let style = TextCursorStyle {
            color: color.try_into()?,
            selection_color: selection_color.try_into()?,
            unfocused_selection_color: unfocused_selection_color.try_into()?,
            selected_text_color: selected_text_color.map(Color::try_from).transpose()?,
            selection_radius: Self::check_selection_radius(selection_radius)?,
        };
        Ok(Self::from_owned(style).into())
    }

    #[getter]
    pub fn color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |style| &style.color, py)
    }

    #[setter]
    pub fn set_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.color = color.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn selection_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |style| &style.selection_color, py)
    }

    #[setter]
    pub fn set_selection_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.selection_color = color.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn unfocused_selection_color(&self, py: Python) -> PyResult<Py<PyColor>> {
        PyColor::from_component_field(&self.storage, |style| &style.unfocused_selection_color, py)
    }

    #[setter]
    pub fn set_unfocused_selection_color(&mut self, color: PyColor) -> PyResult<()> {
        self.as_mut()?.unfocused_selection_color = color.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn selected_text_color(&self, py: Python<'_>) -> PyResult<Option<Py<PyColor>>> {
        self.storage
            .borrow_optional_field(
                "TextCursorStyle.selected_text_color",
                |style| &style.selected_text_color,
                |style| &mut style.selected_text_color,
            )?
            .map(|storage| PyColor::from_storage(storage, py))
            .transpose()
    }

    #[setter]
    pub fn set_selected_text_color(&mut self, color: Option<PyColor>) -> PyResult<()> {
        self.as_mut()?.selected_text_color = color.map(Color::try_from).transpose()?;
        Ok(())
    }

    #[getter]
    pub fn selection_radius(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.selection_radius)
    }

    #[setter]
    pub fn set_selection_radius(&mut self, value: f32) -> PyResult<()> {
        let value = Self::check_selection_radius(value)?;
        self.as_mut()?.selection_radius = value;
        Ok(())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }
}
