use bevy::mesh::SphereKind;
use pyo3::prelude::*;

#[pyclass(
    name = "SphereKind",
    module = "pybevy.mesh",
    frozen,
    eq,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PySphereKind {
    Ico { subdivisions: u32 },
    Uv { sectors: u32, stacks: u32 },
}

#[pymethods]
impl PySphereKind {
    pub fn __repr__(&self) -> String {
        match self {
            Self::Ico { subdivisions } => {
                format!("SphereKind.Ico(subdivisions={subdivisions})")
            }
            Self::Uv { sectors, stacks } => {
                format!("SphereKind.Uv(sectors={sectors}, stacks={stacks})")
            }
        }
    }
}

impl From<PySphereKind> for SphereKind {
    fn from(kind: PySphereKind) -> Self {
        match kind {
            PySphereKind::Ico { subdivisions } => SphereKind::Ico { subdivisions },
            PySphereKind::Uv { sectors, stacks } => SphereKind::Uv { sectors, stacks },
        }
    }
}

impl From<SphereKind> for PySphereKind {
    fn from(kind: SphereKind) -> Self {
        match kind {
            SphereKind::Ico { subdivisions } => Self::Ico { subdivisions },
            SphereKind::Uv { sectors, stacks } => Self::Uv { sectors, stacks },
        }
    }
}

impl Default for PySphereKind {
    fn default() -> Self {
        SphereKind::default().into()
    }
}
