use bevy::text::LineHeight;
use pybevy_core::{
    ComponentStorage, domain::FieldDomain, enum_comparison::compare_values, repr::float_repr,
};
use pybevy_macros::pyenum;
use pyo3::{prelude::*, pyclass::CompareOp};

const LINE_HEIGHT_PX: &[FieldDomain] = &[FieldDomain::RangeInclusive {
    lo: -65536.0,
    hi: 65536.0,
}];
const LINE_HEIGHT_RELATIVE: &[FieldDomain] = &[FieldDomain::RangeInclusive {
    lo: -1000.0,
    hi: 1000.0,
}];

#[pyenum(LineHeight, component, manual_comparison)]
#[pyclass(name = "LineHeight", module = "pybevy.text")]
pub enum PyLineHeight {
    #[py_bevy(tuple)]
    Px {
        #[py_set]
        #[py_domain(LINE_HEIGHT_PX)]
        value: f32,
    },
    #[py_bevy(tuple)]
    RelativeToFont {
        #[py_set]
        #[py_domain(LINE_HEIGHT_RELATIVE)]
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
            LineHeight::Px(value) => Ok(format!("LineHeight.Px({})", float_repr(*value))),
            LineHeight::RelativeToFont(value) => {
                Ok(format!("LineHeight.RelativeToFont({})", float_repr(*value)))
            }
        }
    }
}
