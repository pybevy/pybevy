use bevy::ui::{ConicGradient, Gradient, LinearGradient, RadialGradient};
use pybevy_color::color::PyColor;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

use crate::gradient_snapshot::Snapshot;

#[pyenum(Gradient, empty_tuple)]
#[pyclass(name = "Gradient", module = "pybevy.ui", eq, frozen, from_py_object)]
#[derive(Clone, Debug, PartialEq)]
pub enum PyGradient {
    #[py_bevy(tuple)]
    Linear {
        #[py_type(Snapshot<LinearGradient>)]
        value: LinearGradient,
    },
    #[py_bevy(tuple)]
    Radial {
        #[py_type(Snapshot<RadialGradient>)]
        value: RadialGradient,
    },
    #[py_bevy(tuple)]
    Conic {
        #[py_type(Snapshot<ConicGradient>)]
        value: ConicGradient,
    },
}

#[pymethods]
impl PyGradient {
    pub fn is_empty(&self) -> bool {
        Gradient::from(self.clone()).is_empty()
    }

    pub fn get_single(&self, py: Python) -> PyResult<Option<Py<PyColor>>> {
        match Gradient::from(self.clone()).get_single() {
            Some(color) => Ok(Some(PyColor::from_color(color, py)?)),
            None => Ok(None),
        }
    }
}
