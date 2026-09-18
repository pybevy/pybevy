use bevy::{
    color::Color,
    gizmos::{
        aabb::{AabbGizmoConfigGroup, ShowAabbGizmo},
        config::{GizmoConfig, GizmoConfigStore},
    },
};
use pybevy_color::color::PyColor;
use pybevy_core::{
    ComponentStorage, FieldStorage, FromBorrowedStorage, PyComponent, ResourceStorage, inventory,
};
use pybevy_macros::{pycomponent, pyfield};
use pyo3::{PyTypeInfo, prelude::*};

use crate::{GizmoConfigGroupRegistration, config::PyGizmoConfigGroup};

#[pyfield(AabbGizmoConfigGroup)]
#[pyclass(
    name = "AabbGizmoConfigGroup",
    module = "pybevy.gizmos",
    extends = PyGizmoConfigGroup,
    from_py_object
)]
pub struct PyAabbGizmoConfigGroup {
    storage: FieldStorage<AabbGizmoConfigGroup>,
}

#[pymethods]
impl PyAabbGizmoConfigGroup {
    #[new]
    #[pyo3(signature = (*, draw_all = false, default_color = None))]
    pub fn new(
        draw_all: bool,
        default_color: Option<PyColor>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let default_color = default_color.map(Color::try_from).transpose()?;
        Ok(
            PyClassInitializer::from(PyGizmoConfigGroup).add_subclass(Self::from_owned(
                AabbGizmoConfigGroup {
                    draw_all,
                    default_color,
                },
            )),
        )
    }

    #[getter]
    pub fn draw_all(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.draw_all)
    }

    #[setter]
    pub fn set_draw_all(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.draw_all = value;
        Ok(())
    }

    #[getter]
    pub fn default_color(&self, py: Python<'_>) -> PyResult<Option<Py<PyColor>>> {
        self.storage
            .borrow_optional_field(|group| &group.default_color)?
            .map(|storage| PyColor::from_storage(storage, py))
            .transpose()
    }

    #[setter]
    pub fn set_default_color(&mut self, color: Option<PyColor>) -> PyResult<()> {
        self.as_mut()?.default_color = color.map(Color::try_from).transpose()?;
        Ok(())
    }
}

fn aabb_gizmo_config(store: &GizmoConfigStore) -> Option<&GizmoConfig> {
    store
        .get_config::<AabbGizmoConfigGroup>()
        .map(|value| value.0)
}

fn aabb_gizmo_config_mut(store: &mut GizmoConfigStore) -> Option<&mut GizmoConfig> {
    store
        .get_config_mut::<AabbGizmoConfigGroup>()
        .map(|value| value.0)
}

fn aabb_gizmo_group(
    py: Python<'_>,
    storage: &ResourceStorage<GizmoConfigStore>,
) -> PyResult<Py<PyAny>> {
    // SAFETY: GizmoConfigStore validated this exact native group immediately
    // before materialization. Its API exposes no insertion or removal, so the
    // boxed group value cannot relocate during the shared validity window.
    let storage = unsafe {
        storage.borrow_projected_ref::<AabbGizmoConfigGroup, FieldStorage<AabbGizmoConfigGroup>>(
            |store| {
                store
                    .get_config::<AabbGizmoConfigGroup>()
                    .expect("AABB gizmo group was validated before projection")
                    .1
            },
        )?
    };
    let initializer = PyClassInitializer::from(PyGizmoConfigGroup)
        .add_subclass(PyAabbGizmoConfigGroup::from_borrowed(storage));
    Ok(Py::new(py, initializer)?.into_any())
}

fn aabb_gizmo_group_mut(
    py: Python<'_>,
    storage: &mut ResourceStorage<GizmoConfigStore>,
) -> PyResult<Py<PyAny>> {
    // SAFETY: as above. This projects the group-specific `.1` field, which is
    // disjoint from the common GizmoConfig `.0` projection materialized by the
    // caller from the same uniquely borrowed store entry.
    let storage = unsafe {
        storage.borrow_projected_mut::<AabbGizmoConfigGroup, FieldStorage<AabbGizmoConfigGroup>>(
            |store| {
                store
                    .get_config_mut::<AabbGizmoConfigGroup>()
                    .expect("AABB gizmo group was validated before projection")
                    .1
            },
        )?
    };
    let initializer = PyClassInitializer::from(PyGizmoConfigGroup)
        .add_subclass(PyAabbGizmoConfigGroup::from_borrowed(storage));
    Ok(Py::new(py, initializer)?.into_any())
}

inventory::submit!(GizmoConfigGroupRegistration {
    py_type: |py| <PyAabbGizmoConfigGroup as PyTypeInfo>::type_object(py).as_type_ptr(),
    name: "AabbGizmoConfigGroup",
    config: aabb_gizmo_config,
    config_mut: aabb_gizmo_config_mut,
    group: aabb_gizmo_group,
    group_mut: aabb_gizmo_group_mut,
});

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
    #[pyo3(signature = (*, color = None))]
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
