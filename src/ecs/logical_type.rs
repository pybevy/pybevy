//! Hidden adapter component used to declare logical-type query access.

use pybevy_core::{ComponentStorage, LogicalTypeMap, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(LogicalTypeMap, no_clone, no_insert, no_reflect, bridge)]
#[pyclass(name = "_LogicalTypeMap", extends = PyComponent)]
pub struct PyLogicalTypeMap {
    #[allow(dead_code)]
    storage: ComponentStorage<LogicalTypeMap>,
}
