pub mod name;

use pyo3::prelude::*;

pub mod prelude {
    pub use crate::name::PyName;
}

pub fn add_ecs_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<name::PyName>()?;
    Ok(())
}
