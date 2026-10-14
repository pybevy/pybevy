use bevy::{ecs::entity::Entity, window::Ime};
use pybevy_core::PyEntity;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(Ime, message)]
#[pyclass(module = "pybevy.window", name = "Ime")]
pub enum PyIme {
    Preedit {
        #[py_type(PyEntity)]
        window: Entity,
        value: String,
        #[py_default(None)]
        cursor: Option<(usize, usize)>,
    },
    Commit {
        #[py_type(PyEntity)]
        window: Entity,
        value: String,
    },
    Enabled {
        #[py_type(PyEntity)]
        window: Entity,
    },
    Disabled {
        #[py_type(PyEntity)]
        window: Entity,
    },
}
