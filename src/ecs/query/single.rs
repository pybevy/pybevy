use pyo3::{
    Bound,
    prelude::*,
    types::{PyAny, PyTuple, PyType},
};

use crate::ecs::query::{
    query_helpers::construct_query_class_item_with_options, query_param::PyQueryParam,
};

/// A query parameter that enforces exactly one entity matches the query.
///
/// Skips the system if zero or multiple entities match the query filter.
/// Optional annotations instead inject None and run the system.
///
/// Example:
/// ```python
/// def system(player: Single[tuple[Mut[Transform], Player]]) -> None:
///     transform, player_data = player[0], player[1]
///     transform.translation.x += 10.0
/// ```
#[pyclass(name = "Single", module = "pybevy.ecs", frozen)]
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
        if let Ok(args) = key.cast::<PyTuple>()
            && args.len() == 1
            && args.get_item(0)?.is_instance_of::<PyQueryParam>()
        {
            return cls
                .py()
                .import("types")?
                .getattr("GenericAlias")?
                .call1((cls, args))
                .map(Bound::unbind);
        }
        // Reuse the Query construction logic but mark it as single-entity enforced
        construct_query_class_item_with_options(cls, key, true)
    }
}
