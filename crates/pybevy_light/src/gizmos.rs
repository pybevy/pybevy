use bevy::{
    color::Color,
    gizmos::config::{GizmoConfig, GizmoConfigStore},
    light::gizmos::{LightGizmoColor, LightGizmoConfigGroup, ShowLightGizmo},
};
use pybevy_color::color::{OwnedColorValue, PyColor};
use pybevy_core::{
    ComponentStorage, FieldStorage, FromBorrowedStorage, PyComponent, ResourceStorage, inventory,
};
use pybevy_gizmos::{GizmoConfigGroupRegistration, prelude::PyGizmoConfigGroup};
use pybevy_macros::{pycomponent, pyenum, pyfield};
use pyo3::{PyTypeInfo, prelude::*};

#[pyenum(LightGizmoColor, empty_tuple, no_repr)]
#[pyclass(
    name = "LightGizmoColor",
    module = "pybevy.light",
    frozen,
    from_py_object
)]
#[derive(Debug, Clone)]
pub enum PyLightGizmoColor {
    #[py_bevy(tuple)]
    Manual {
        #[py_type(OwnedColorValue)]
        value: Color,
    },
    Varied(),
    MatchLightColor(),
    ByLightType(),
}

#[pymethods]
impl PyLightGizmoColor {
    fn __repr__(&self) -> PyResult<String> {
        Ok(match self {
            Self::Manual { value } => {
                let value: PyColor = value.0.into();
                format!("LightGizmoColor.Manual({})", value.__repr__()?)
            }
            Self::Varied() => "LightGizmoColor.Varied()".to_string(),
            Self::MatchLightColor() => "LightGizmoColor.MatchLightColor()".to_string(),
            Self::ByLightType() => "LightGizmoColor.ByLightType()".to_string(),
        })
    }
}

#[pyfield(LightGizmoConfigGroup)]
#[pyclass(
    name = "LightGizmoConfigGroup",
    extends = PyGizmoConfigGroup,
    from_py_object
)]
pub struct PyLightGizmoConfigGroup {
    storage: FieldStorage<LightGizmoConfigGroup>,
}

#[pymethods]
impl PyLightGizmoConfigGroup {
    #[new]
    #[pyo3(signature = (
        draw_all = false,
        color = PyLightGizmoColor::MatchLightColor(),
        point_light_color = Self::default_point_light_color(),
        spot_light_color = Self::default_spot_light_color(),
        directional_light_color = Self::default_directional_light_color(),
        rect_light_color = Self::default_rect_light_color(),
    ))]
    pub fn new(
        draw_all: bool,
        color: PyLightGizmoColor,
        point_light_color: PyColor,
        spot_light_color: PyColor,
        directional_light_color: PyColor,
        rect_light_color: PyColor,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok(
            PyClassInitializer::from(PyGizmoConfigGroup).add_subclass(Self::from_owned(
                LightGizmoConfigGroup {
                    draw_all,
                    color: color.into(),
                    point_light_color: point_light_color.try_into()?,
                    spot_light_color: spot_light_color.try_into()?,
                    directional_light_color: directional_light_color.try_into()?,
                    rect_light_color: rect_light_color.try_into()?,
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
    pub fn color(&self) -> PyResult<PyLightGizmoColor> {
        Ok(self.as_ref()?.color.into())
    }

    #[setter]
    pub fn set_color(&mut self, value: PyLightGizmoColor) -> PyResult<()> {
        self.as_mut()?.color = value.into();
        Ok(())
    }

    #[getter]
    pub fn point_light_color(&self, py: Python<'_>) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.point_light_color, py)
    }

    #[setter]
    pub fn set_point_light_color(&mut self, value: PyColor) -> PyResult<()> {
        self.as_mut()?.point_light_color = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn spot_light_color(&self, py: Python<'_>) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.spot_light_color, py)
    }

    #[setter]
    pub fn set_spot_light_color(&mut self, value: PyColor) -> PyResult<()> {
        self.as_mut()?.spot_light_color = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn directional_light_color(&self, py: Python<'_>) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.directional_light_color, py)
    }

    #[setter]
    pub fn set_directional_light_color(&mut self, value: PyColor) -> PyResult<()> {
        self.as_mut()?.directional_light_color = value.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn rect_light_color(&self, py: Python<'_>) -> PyResult<Py<PyColor>> {
        PyColor::from_color(self.as_ref()?.rect_light_color, py)
    }

    #[setter]
    pub fn set_rect_light_color(&mut self, value: PyColor) -> PyResult<()> {
        self.as_mut()?.rect_light_color = value.try_into()?;
        Ok(())
    }
}

impl PyLightGizmoConfigGroup {
    fn default_point_light_color() -> PyColor {
        LightGizmoConfigGroup::default().point_light_color.into()
    }

    fn default_spot_light_color() -> PyColor {
        LightGizmoConfigGroup::default().spot_light_color.into()
    }

    fn default_directional_light_color() -> PyColor {
        LightGizmoConfigGroup::default()
            .directional_light_color
            .into()
    }

    fn default_rect_light_color() -> PyColor {
        LightGizmoConfigGroup::default().rect_light_color.into()
    }
}

fn light_gizmo_config(store: &GizmoConfigStore) -> Option<&GizmoConfig> {
    store
        .get_config::<LightGizmoConfigGroup>()
        .map(|value| value.0)
}

fn light_gizmo_config_mut(store: &mut GizmoConfigStore) -> Option<&mut GizmoConfig> {
    store
        .get_config_mut::<LightGizmoConfigGroup>()
        .map(|value| value.0)
}

fn light_gizmo_group(
    py: Python<'_>,
    storage: &ResourceStorage<GizmoConfigStore>,
) -> PyResult<Py<PyAny>> {
    // SAFETY: GizmoConfigStore validated this exact native group immediately
    // before materialization. Its API exposes no insertion or removal, so the
    // boxed group value cannot relocate during the shared validity window.
    let storage = unsafe {
        storage.borrow_projected_ref::<LightGizmoConfigGroup, FieldStorage<LightGizmoConfigGroup>>(
            |store| {
                store
                    .get_config::<LightGizmoConfigGroup>()
                    .expect("light gizmo group was validated before projection")
                    .1
            },
        )?
    };
    let initializer = PyClassInitializer::from(PyGizmoConfigGroup)
        .add_subclass(PyLightGizmoConfigGroup::from_borrowed(storage));
    Ok(Py::new(py, initializer)?.into_any())
}

fn light_gizmo_group_mut(
    py: Python<'_>,
    storage: &mut ResourceStorage<GizmoConfigStore>,
) -> PyResult<Py<PyAny>> {
    // SAFETY: as above. This projects the group-specific `.1` field, which is
    // disjoint from the common GizmoConfig `.0` projection materialized by the
    // caller from the same uniquely borrowed store entry.
    let storage = unsafe {
        storage.borrow_projected_mut::<LightGizmoConfigGroup, FieldStorage<LightGizmoConfigGroup>>(
            |store| {
                store
                    .get_config_mut::<LightGizmoConfigGroup>()
                    .expect("light gizmo group was validated before projection")
                    .1
            },
        )?
    };
    let initializer = PyClassInitializer::from(PyGizmoConfigGroup)
        .add_subclass(PyLightGizmoConfigGroup::from_borrowed(storage));
    Ok(Py::new(py, initializer)?.into_any())
}

inventory::submit!(GizmoConfigGroupRegistration {
    py_type: |py| <PyLightGizmoConfigGroup as PyTypeInfo>::type_object(py).as_type_ptr(),
    name: "LightGizmoConfigGroup",
    config: light_gizmo_config,
    config_mut: light_gizmo_config_mut,
    group: light_gizmo_group,
    group_mut: light_gizmo_group_mut,
});

fn clone_show_light_gizmo(value: &ShowLightGizmo) -> ShowLightGizmo {
    ShowLightGizmo { color: value.color }
}

#[pycomponent(ShowLightGizmo, bridge, clone_with = clone_show_light_gizmo)]
#[pyclass(name = "ShowLightGizmo", extends = PyComponent)]
#[derive(Debug)]
pub struct PyShowLightGizmo {
    pub(crate) storage: ComponentStorage<ShowLightGizmo>,
}

#[pymethods]
impl PyShowLightGizmo {
    #[new]
    #[pyo3(signature = (color = None))]
    pub fn new(color: Option<PyLightGizmoColor>) -> PyClassInitializer<Self> {
        (
            Self {
                storage: ComponentStorage::owned(ShowLightGizmo {
                    color: color.map(Into::into),
                }),
            },
            PyComponent,
        )
            .into()
    }

    #[getter]
    pub fn color(&self) -> PyResult<Option<PyLightGizmoColor>> {
        Ok(self.as_ref()?.color.map(Into::into))
    }

    #[setter]
    pub fn set_color(&mut self, color: Option<PyLightGizmoColor>) -> PyResult<()> {
        self.as_mut()?.color = color.map(Into::into);
        Ok(())
    }
}
