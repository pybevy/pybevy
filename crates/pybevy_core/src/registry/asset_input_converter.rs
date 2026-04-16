//! `AssetInputConverter` lets per-asset PyTypes accept "factory" inputs
//! (e.g. `Assets[Mesh].add(Cuboid::ONE)`) in addition to direct asset
//! instances. Wired into a bridge's `try_convert_input` via the
//! `#[pyasset(..., bridge, input_converter)]` macro flag.

use pyo3::prelude::*;

/// Convert a non-asset Python input into a form acceptable by `bridge.add()`.
///
/// `Some(converted)` is forwarded to the standard type check + `add()`.
/// `None` lets the standard type check run unchanged.
pub trait AssetInputConverter {
    fn try_convert_input<'py>(
        asset: &Bound<'py, PyAny>,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyAny>>>;
}
