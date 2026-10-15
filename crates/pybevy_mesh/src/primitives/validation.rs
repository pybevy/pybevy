use pyo3::{exceptions::PyValueError, prelude::*};

pub(super) const MAX_MESH_VERTICES: u128 = 1 << 24;

pub(crate) fn vertex_budget(builder: &str, vertices: u128) -> PyResult<()> {
    if vertices > MAX_MESH_VERTICES {
        return Err(PyValueError::new_err(format!(
            "{builder} would create {vertices} vertices; maximum is {MAX_MESH_VERTICES}"
        )));
    }
    Ok(())
}

pub(super) fn vertex_budget_product(builder: &str, left: u128, right: u128) -> PyResult<()> {
    let Some(vertices) = left.checked_mul(right) else {
        return Err(PyValueError::new_err(format!(
            "{builder} would create more than {MAX_MESH_VERTICES} vertices"
        )));
    };
    vertex_budget(builder, vertices)
}

pub(super) fn minimum_count(name: &str, count: u32, minimum: u32) -> PyResult<()> {
    if count < minimum {
        return Err(PyValueError::new_err(format!(
            "{name} must be at least {minimum}"
        )));
    }
    Ok(())
}
