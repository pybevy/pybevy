use pyo3::prelude::*;

#[pymodule(gil_used = false)]
fn _pybevy(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pybevy::init_module(m)
}
