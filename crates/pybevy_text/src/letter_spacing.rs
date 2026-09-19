use bevy::text::LetterSpacing;
use pybevy_core::{ComponentStorage, enum_comparison::compare_values};
use pybevy_macros::pyenum;
use pyo3::{prelude::*, pyclass::CompareOp};

#[pyenum(LetterSpacing, component, manual_comparison)]
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
    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<Py<PyAny>> {
        compare_values(self, other, op, |left, right| {
            Ok(left.as_ref()? == right.as_ref()?)
        })
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()?.reborrow() {
            LetterSpacing::Px(value) => Ok(format!("LetterSpacing.Px({value})")),
            LetterSpacing::Rem(value) => Ok(format!("LetterSpacing.Rem({value})")),
        }
    }
}
