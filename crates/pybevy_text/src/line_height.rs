use bevy::text::LineHeight;
use pybevy_core::{ComponentStorage, enum_comparison::compare_values};
use pybevy_macros::pyenum;
use pyo3::{prelude::*, pyclass::CompareOp};

#[pyenum(LineHeight, component, manual_comparison)]
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
    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<Py<PyAny>> {
        compare_values(self, other, op, |left, right| {
            Ok(left.as_ref()? == right.as_ref()?)
        })
    }

    pub fn __repr__(&self) -> PyResult<String> {
        match self.as_ref()?.reborrow() {
            LineHeight::Px(value) => Ok(format!("LineHeight.Px({value})")),
            LineHeight::RelativeToFont(value) => Ok(format!("LineHeight.RelativeToFont({value})")),
        }
    }
}
