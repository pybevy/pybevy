use bevy::{color::Color, gizmos::aabb::ShowAabbGizmo};
use pybevy_color::color::PyColor;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

fn clone_show_aabb_gizmo(value: &ShowAabbGizmo) -> ShowAabbGizmo {
    ShowAabbGizmo { color: value.color }
}

#[pycomponent(ShowAabbGizmo, bridge, clone_with = clone_show_aabb_gizmo)]
#[pyclass(name = "ShowAabbGizmo", module = "pybevy.gizmos", extends = PyComponent)]
#[derive(Debug)]
pub struct PyShowAabbGizmo {
    pub(crate) storage: ComponentStorage<ShowAabbGizmo>,
}

#[pymethods]
impl PyShowAabbGizmo {
    #[new]
    #[pyo3(signature = (color = None))]
    pub fn new(color: Option<PyColor>) -> PyResult<PyClassInitializer<Self>> {
        let color = color.map(Color::try_from).transpose()?;
        Ok((
            Self {
                storage: ComponentStorage::owned(ShowAabbGizmo { color }),
            },
            PyComponent,
        )
            .into())
    }

    #[getter]
    pub fn color(&self, py: Python<'_>) -> PyResult<Option<Py<PyColor>>> {
        self.storage
            .borrow_optional_field(|gizmo| &gizmo.color)?
            .map(|storage| PyColor::from_storage(storage, py))
            .transpose()
    }

    #[setter]
    pub fn set_color(&mut self, color: Option<PyColor>) -> PyResult<()> {
        let color = color.map(Color::try_from).transpose()?;
        self.as_mut()?.color = color;
        Ok(())
    }
}
