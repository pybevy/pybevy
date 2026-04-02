pub mod name;

pub use name::PyName;
use pyo3::prelude::*;

pub fn add_ecs_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyName>()?;
    Ok(())
}
