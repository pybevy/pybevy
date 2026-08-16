use bevy::text::LetterSpacing;
use pybevy_core::ComponentStorage;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(LetterSpacing, component)]
#[pyclass(name = "LetterSpacing", module = "pybevy.text")]
pub enum PyLetterSpacing {
    #[py_bevy(tuple)]
    Px {
        #[py_set]
        value: f32,
    },
    #[py_bevy(tuple)]
    Rem {
        #[py_set]
        value: f32,
    },
}

#[pymethods]
impl PyLetterSpacing {
    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()?.reborrow() {
            LetterSpacing::Px(value) => Ok(format!("LetterSpacing.Px({value})")),
            LetterSpacing::Rem(value) => Ok(format!("LetterSpacing.Rem({value})")),
        }
    }
}
