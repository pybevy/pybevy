use bevy::{ecs::entity::Entity, window::Ime};
use pybevy_core::PyEntity;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(Ime, message)]
#[pyclass(module = "pybevy.window", name = "Ime")]
pub enum PyIme {
    #[pyo3(constructor = (*, window, value, cursor))]
    Preedit {
        #[py_type(PyEntity)]
        window: Entity,
        value: String,
        #[py_default(None)]
        cursor: Option<(usize, usize)>,
    },
    #[pyo3(constructor = (*, window, value))]
    Commit {
        #[py_type(PyEntity)]
        window: Entity,
        value: String,
    },
    #[pyo3(constructor = (*, window))]
    Enabled {
        #[py_type(PyEntity)]
        window: Entity,
    },
    #[pyo3(constructor = (*, window))]
    Disabled {
        #[py_type(PyEntity)]
        window: Entity,
    },
}
