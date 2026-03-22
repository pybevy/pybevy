use bevy::mesh::SphereKind;
use pyo3::prelude::*;

#[pyclass(name = "SphereKind", frozen)]
#[derive(Debug)]
pub struct PySphereKind(pub(crate) SphereKind);

#[pymethods]
impl PySphereKind {
    #[new]
    pub fn new() -> Self {
        Self(SphereKind::default())
    }

    #[staticmethod]
    pub fn ico(subdivisions: u32) -> Self {
        Self(SphereKind::Ico { subdivisions })
    }

    #[staticmethod]
    pub fn uv(sectors: u32, stacks: u32) -> Self {
        Self(SphereKind::Uv { sectors, stacks })
    }

    pub fn __repr__(&self) -> String {
        match self.0 {
            SphereKind::Ico { subdivisions } => {
                format!("SphereKind.ico(subdivisions={})", subdivisions)
            }
            SphereKind::Uv { sectors, stacks } => {
                format!("SphereKind.uv(sectors={}, stacks={})", sectors, stacks)
            }
        }
    }
}

impl From<PySphereKind> for SphereKind {
    fn from(kind: PySphereKind) -> Self {
        kind.0
    }
}

impl From<SphereKind> for PySphereKind {
    fn from(kind: SphereKind) -> Self {
        PySphereKind(kind)
    }
}

impl Default for PySphereKind {
    fn default() -> Self {
        Self::new()
    }
}
