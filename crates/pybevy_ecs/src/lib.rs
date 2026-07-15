pub mod shared;

#[cfg(feature = "pyo3")]
pub mod name;

#[cfg(feature = "pyo3")]
use pyo3::prelude::*;

#[cfg(feature = "pyo3")]
pub mod prelude {
    pub use crate::name::PyName;
}

#[cfg(feature = "pyo3")]
pub fn add_ecs_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<name::PyName>()?;
    Ok(())
}
