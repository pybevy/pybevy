use bevy::text::LetterSpacing;
use pybevy_core::{
    ComponentStorage, domain::FieldDomain, enum_comparison::compare_values, repr::float_repr,
};
use pybevy_macros::pyenum;
use pyo3::{prelude::*, pyclass::CompareOp};

const LETTER_SPACING_PX: &[FieldDomain] = &[FieldDomain::RangeInclusive {
    lo: -65536.0,
    hi: 65536.0,
}];
const LETTER_SPACING_REM: &[FieldDomain] = &[FieldDomain::RangeInclusive {
    lo: -4096.0,
    hi: 4096.0,
}];

#[pyenum(LetterSpacing, component, manual_comparison)]
#[pyclass(name = "LetterSpacing", module = "pybevy.text")]
pub enum PyLetterSpacing {
    #[py_bevy(tuple)]
    Px {
        #[py_set]
        #[py_domain(LETTER_SPACING_PX)]
        value: f32,
    },
    #[py_bevy(tuple)]
    Rem {
        #[py_set]
        #[py_domain(LETTER_SPACING_REM)]
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
            LetterSpacing::Px(value) => Ok(format!("LetterSpacing.Px({})", float_repr(*value))),
            LetterSpacing::Rem(value) => Ok(format!("LetterSpacing.Rem({})", float_repr(*value))),
        }
    }
}
