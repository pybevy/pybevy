use pybevy_core::domain::FieldDomain;
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

pub(crate) const AT_LEAST_ONE: &[FieldDomain] = &[FieldDomain::AtLeast(1)];
pub(crate) const AT_LEAST_TWO: &[FieldDomain] = &[FieldDomain::AtLeast(2)];
pub(crate) const AT_LEAST_THREE: &[FieldDomain] = &[FieldDomain::AtLeast(3)];
pub(crate) const AT_LEAST_FOUR: &[FieldDomain] = &[FieldDomain::AtLeast(4)];
