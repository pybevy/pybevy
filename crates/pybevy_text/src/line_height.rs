use bevy::text::LineHeight;
use pybevy_core::ComponentStorage;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(LineHeight, component)]
#[pyclass(name = "LineHeight", module = "pybevy.text")]
pub enum PyLineHeight {
    #[py_bevy(tuple)]
    Px {
        #[py_set]
        value: f32,
    },
    #[py_bevy(tuple)]
    RelativeToFont {
        #[py_set]
        value: f32,
    },
}

#[pymethods]
impl PyLineHeight {
    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()?.reborrow() {
            LineHeight::Px(value) => Ok(format!("LineHeight.Px({value})")),
            LineHeight::RelativeToFont(value) => Ok(format!("LineHeight.RelativeToFont({value})")),
        }
    }
}
