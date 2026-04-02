use std::sync::Arc;

use bevy::{
    camera::visibility::Visibility,
    ecs::{entity::Entity, world::World},
};
use pybevy_core::{BatchComponent, registry::global_registry};
use pyo3::prelude::*;

#[pyclass(name = "VisibilityBatch")]
#[derive(Debug)]
pub struct PyVisibilityBatch {
    /// Boolean or integer array indicating visibility state.
    /// True/1 = Visible, False/0 = Hidden
    visibility: Py<PyAny>,
}

impl Clone for PyVisibilityBatch {
    fn clone(&self) -> Self {
        Python::attach(|py| PyVisibilityBatch {
            visibility: self.visibility.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyVisibilityBatch {
    #[new]
    pub fn new(visibility: Py<PyAny>) -> Self {
        PyVisibilityBatch { visibility }
    }
    pub fn count(&self, py: Python) -> PyResult<usize> {
        let array = self.visibility.bind(py);
        let shape = array.getattr("shape")?;
        let count: usize = shape.get_item(0)?.extract()?;
        Ok(count)
    }
}

pub struct VisibilityBatchBridge;

impl BatchComponent for VisibilityBatchBridge {
    fn name(&self) -> &'static str {
        "VisibilityBatch"
    }

    fn count(&self, py: Python, batch: &Bound<PyAny>) -> PyResult<usize> {
        let batch = batch.extract::<PyVisibilityBatch>()?;
        batch.count(py)
    }

    fn insert_bulk(
        &self,
        py: Python,
        batch: &Bound<PyAny>,
        entities: &[Entity],
        world: &mut World,
    ) -> PyResult<()> {
        let batch = batch.extract::<PyVisibilityBatch>()?;
        let array = batch.visibility.bind(py);

        // Cast to bool dtype via numpy (handles int, bool, etc.), then extract
        let np = py.import("numpy")?;
        let bool_array = array.call_method1("astype", (np.getattr("bool_")?,))?;
        let values: Vec<bool> = bool_array.call_method0("tolist")?.extract()?;

        // Tight loop — pure Rust, no Python FFI per entity
        for (&entity_id, &visible) in entities.iter().zip(values.iter()) {
            let visibility = if visible {
                Visibility::Visible
            } else {
                Visibility::Hidden
            };
            world.entity_mut(entity_id).insert(visibility);
        }

        Ok(())
    }
}

pub fn register_visibility_batch_bridge() {
    Python::attach(|py| {
        let ptr = <PyVisibilityBatch as pyo3::PyTypeInfo>::type_object(py).as_type_ptr();
        global_registry::register_batch_bridge(ptr, Arc::new(VisibilityBatchBridge));
    });
}
