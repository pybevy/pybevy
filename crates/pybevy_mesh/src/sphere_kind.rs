use bevy::mesh::SphereKind;
use pybevy_macros::pyenum;
use pyo3::prelude::*;

#[pyenum(SphereKind)]
#[pyclass(
    name = "SphereKind",
    module = "pybevy.mesh",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PySphereKind {
    #[pyo3(constructor = (*, subdivisions))]
    Ico { subdivisions: u32 },
    #[pyo3(constructor = (*, sectors, stacks))]
    Uv { sectors: u32, stacks: u32 },
}

impl Default for PySphereKind {
    fn default() -> Self {
        SphereKind::default().into()
    }
}
