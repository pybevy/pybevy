use pyo3::{
    Bound,
    prelude::*,
    types::{PyAny, PyType},
};

use crate::ecs::query::query_helpers::construct_query_class_item_with_options;

/// A query parameter that enforces exactly one entity matches the query.
///
/// Panics if zero or multiple entities match the query filter.
///
/// Example:
/// ```python
/// def system(player: Single[tuple[Mut[Transform], Player]]) -> None:
///     transform, player_data = player  # Guaranteed to be single entity
///     transform.translation.x += 10.0
/// ```
#[pyclass(name = "Single", frozen)]
pub struct PySingle;

#[pymethods]
impl PySingle {
    /// Creates a new `QueryParam` from the given type parameters with single-entity enforcement.
    ///
    /// This enables syntax like `Single[Transform]` or `Single[tuple[Transform, Velocity], With[Player]]`
    #[classmethod]
    #[pyo3(signature = (key, /))]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        // Reuse the Query construction logic but mark it as single-entity enforced
        construct_query_class_item_with_options(cls, key, true)
    }
}
