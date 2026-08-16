use std::hash::{Hash, Hasher};

use bevy::{
    camera::visibility::RenderLayers,
    gizmos::config::{
        DefaultGizmoConfigGroup, GizmoConfig, GizmoConfigStore, GizmoLineConfig, GizmoLineJoint,
        GizmoLineStyle,
    },
};
use pybevy_camera::render_layers::PyRenderLayers;
use pybevy_core::{
    ComponentStorage, FieldStorage, FromBorrowedStorage, PyResource, ResourceStorage, inventory,
    public_error::{
        DEFAULT_GIZMO_CONFIG_MISSING, UNSUPPORTED_GIZMO_LINE_STYLE, missing_gizmo_config_group,
        unregistered_gizmo_config_group,
    },
};
use pybevy_macros::{pyenum, pyfield, pyresource};
use pyo3::{
    IntoPyObjectExt,
    exceptions::{PyKeyError, PyRuntimeError, PyValueError},
    prelude::*,
    types::{PyTuple, PyType},
};

#[pyclass(name = "GizmoConfigGroup", subclass, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PyGizmoConfigGroup;

pub struct GizmoConfigGroupRegistration {
    pub py_type: fn(Python<'_>) -> *mut pyo3::ffi::PyTypeObject,
    pub name: &'static str,
    pub config: fn(&GizmoConfigStore) -> Option<&GizmoConfig>,
    pub config_mut: fn(&mut GizmoConfigStore) -> Option<&mut GizmoConfig>,
    /// Materializes a read-only Python wrapper for the registered group's
    /// group-specific half. The implementation must project the same entry as
    /// `config`, and must not expose an operation that can relocate it.
    pub group: fn(Python<'_>, &ResourceStorage<GizmoConfigStore>) -> PyResult<Py<PyAny>>,
    /// Mutable twin of `group`. The projection must be disjoint from the
    /// common `GizmoConfig` projection returned through `config_mut`.
    pub group_mut: fn(Python<'_>, &mut ResourceStorage<GizmoConfigStore>) -> PyResult<Py<PyAny>>,
}

inventory::collect!(GizmoConfigGroupRegistration);

fn config_group_registration(
    group: &Bound<'_, PyType>,
) -> PyResult<&'static GizmoConfigGroupRegistration> {
    let target = group.as_type_ptr();
    if let Some(registration) = inventory::iter::<GizmoConfigGroupRegistration>
        .into_iter()
        .find(|registration| (registration.py_type)(group.py()) == target)
    {
        return Ok(registration);
    }
    let name = group.name()?;
    let name = name.to_string_lossy();
    Err(PyKeyError::new_err(unregistered_gizmo_config_group(&name)))
}

#[pyenum(GizmoLineJoint, empty_tuple, unit_parens)]
#[pyclass(name = "GizmoLineJoint", frozen, eq, hash, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyGizmoLineJoint {
    #[pyo3(name = "None_")]
    None(),
    Miter(),
    #[py_bevy(tuple)]
    Round {
        value: u32,
    },
    Bevel(),
}

#[pyenum(GizmoLineStyle, manual, empty_tuple, unit_parens)]
#[pyclass(name = "GizmoLineStyle", frozen, eq, hash, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PyGizmoLineStyle {
    Solid(),
    Dotted(),
    Dashed { gap_scale: f32, line_scale: f32 },
}

impl Eq for PyGizmoLineStyle {}

impl Hash for PyGizmoLineStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Solid() => 0_u64.hash(state),
            Self::Dotted() => 1_u64.hash(state),
            Self::Dashed {
                gap_scale,
                line_scale,
            } => {
                2_u64.hash(state);
                gap_scale.to_bits().hash(state);
                line_scale.to_bits().hash(state);
            }
        }
    }
}

impl From<PyGizmoLineStyle> for GizmoLineStyle {
    fn from(style: PyGizmoLineStyle) -> Self {
        match style {
            PyGizmoLineStyle::Solid() => GizmoLineStyle::Solid,
            PyGizmoLineStyle::Dotted() => GizmoLineStyle::Dotted,
            PyGizmoLineStyle::Dashed {
                gap_scale,
                line_scale,
            } => GizmoLineStyle::Dashed {
                gap_scale,
                line_scale,
            },
        }
    }
}

impl TryFrom<GizmoLineStyle> for PyGizmoLineStyle {
    type Error = PyErr;

    fn try_from(style: GizmoLineStyle) -> PyResult<Self> {
        match style {
            GizmoLineStyle::Solid => Ok(Self::Solid()),
            GizmoLineStyle::Dotted => Ok(Self::Dotted()),
            GizmoLineStyle::Dashed {
                gap_scale,
                line_scale,
            } => Ok(Self::Dashed {
                gap_scale,
                line_scale,
            }),
            _ => Err(PyValueError::new_err(UNSUPPORTED_GIZMO_LINE_STYLE)),
        }
    }
}

#[pymethods]
impl PyGizmoLineStyle {
    pub fn __copy__(&self) -> Self {
        *self
    }

    pub fn __repr__(&self) -> String {
        match self {
            Self::Solid() => "GizmoLineStyle.Solid()".to_owned(),
            Self::Dotted() => "GizmoLineStyle.Dotted()".to_owned(),
            Self::Dashed {
                gap_scale,
                line_scale,
            } => {
                format!("GizmoLineStyle.Dashed(gap_scale={gap_scale:?}, line_scale={line_scale:?})")
            }
        }
    }
}

#[pyfield(GizmoLineConfig)]
#[pyclass(name = "GizmoLineConfig", from_py_object)]
#[derive(Debug)]
pub struct PyGizmoLineConfig {
    storage: FieldStorage<GizmoLineConfig>,
}

impl Default for PyGizmoLineConfig {
    fn default() -> Self {
        GizmoLineConfig::default().into()
    }
}

#[pymethods]
impl PyGizmoLineConfig {
    #[new]
    #[pyo3(signature = (
        width = 2.0,
        perspective = false,
        style = PyGizmoLineStyle::Solid(),
        joints = PyGizmoLineJoint::None(),
    ))]
    pub fn new(
        width: f32,
        perspective: bool,
        style: PyGizmoLineStyle,
        joints: PyGizmoLineJoint,
    ) -> Self {
        Self::from_owned(GizmoLineConfig {
            width,
            perspective,
            style: style.into(),
            joints: joints.into(),
        })
    }

    #[getter]
    pub fn width(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.width)
    }

    #[setter]
    pub fn set_width(&mut self, width: f32) -> PyResult<()> {
        self.as_mut()?.width = width;
        Ok(())
    }

    #[getter]
    pub fn perspective(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.perspective)
    }

    #[setter]
    pub fn set_perspective(&mut self, perspective: bool) -> PyResult<()> {
        self.as_mut()?.perspective = perspective;
        Ok(())
    }

    #[getter]
    pub fn style(&self) -> PyResult<PyGizmoLineStyle> {
        self.as_ref()?.style.try_into()
    }

    #[setter]
    pub fn set_style(&mut self, style: PyGizmoLineStyle) -> PyResult<()> {
        self.as_mut()?.style = style.into();
        Ok(())
    }

    #[getter]
    pub fn joints(&self) -> PyResult<PyGizmoLineJoint> {
        Ok(self.as_ref()?.joints.into())
    }

    #[setter]
    pub fn set_joints(&mut self, joints: PyGizmoLineJoint) -> PyResult<()> {
        self.as_mut()?.joints = joints.into();
        Ok(())
    }
}

#[pyfield(GizmoConfig)]
#[pyclass(name = "GizmoConfig", from_py_object)]
#[derive(Debug)]
pub struct PyGizmoConfig {
    storage: FieldStorage<GizmoConfig>,
}

#[pymethods]
impl PyGizmoConfig {
    #[new]
    #[pyo3(signature = (
        enabled = true,
        line = PyGizmoLineConfig::default(),
        depth_bias = 0.0,
        render_layers = None,
    ))]
    pub fn new(
        enabled: bool,
        line: PyGizmoLineConfig,
        depth_bias: f32,
        render_layers: Option<&PyRenderLayers>,
    ) -> PyResult<Self> {
        let render_layers = match render_layers {
            Some(layers) => layers.as_ref()?.clone(),
            None => RenderLayers::default(),
        };
        Ok(Self::from_owned(GizmoConfig {
            enabled,
            line: (&line).try_into()?,
            depth_bias,
            render_layers,
        }))
    }

    #[getter]
    pub fn enabled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.enabled)
    }

    #[setter]
    pub fn set_enabled(&mut self, enabled: bool) -> PyResult<()> {
        self.as_mut()?.enabled = enabled;
        Ok(())
    }

    #[getter]
    pub fn line(&self) -> PyResult<PyGizmoLineConfig> {
        Ok(self.storage.borrow_field_as(|config| &config.line)?)
    }

    #[setter]
    pub fn set_line(&mut self, line: &PyGizmoLineConfig) -> PyResult<()> {
        self.as_mut()?.line = line.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn depth_bias(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.depth_bias)
    }

    #[setter]
    pub fn set_depth_bias(&mut self, depth_bias: f32) -> PyResult<()> {
        self.as_mut()?.depth_bias = depth_bias;
        Ok(())
    }

    #[getter]
    pub fn render_layers(&self, py: Python<'_>) -> PyResult<Py<PyRenderLayers>> {
        let storage: ComponentStorage<RenderLayers> =
            self.storage.borrow_field(|config| &config.render_layers)?;
        Py::new(py, PyRenderLayers::from_borrowed(storage))
    }

    #[setter]
    pub fn set_render_layers(&mut self, render_layers: &PyRenderLayers) -> PyResult<()> {
        self.as_mut()?.render_layers = render_layers.as_ref()?.clone();
        Ok(())
    }
}

#[pyresource(
    GizmoConfigStore,
    no_clone,
    bridge,
    no_insert,
    no_remove,
    preserve_on_reload
)]
#[pyclass(name = "GizmoConfigStore", extends = PyResource)]
pub struct PyGizmoConfigStore {
    pub(crate) storage: ResourceStorage<GizmoConfigStore>,
}

#[pymethods]
impl PyGizmoConfigStore {
    #[pyo3(signature = (group = None))]
    pub fn config(&self, py: Python<'_>, group: Option<&Bound<'_, PyType>>) -> PyResult<Py<PyAny>> {
        let registration = group.map(config_group_registration).transpose()?;
        let (project, missing_error): (fn(&GizmoConfigStore) -> Option<&GizmoConfig>, String) =
            match registration {
                Some(registration) => (
                    registration.config,
                    missing_gizmo_config_group(registration.name),
                ),
                None => (
                    |store| {
                        store
                            .get_config::<DefaultGizmoConfigGroup>()
                            .map(|value| value.0)
                    },
                    DEFAULT_GIZMO_CONFIG_MISSING.to_owned(),
                ),
            };
        let store = self.as_ref()?;
        if project(&store).is_none() {
            return Err(PyRuntimeError::new_err(missing_error));
        }
        // SAFETY: this wrapper exposes no operation that inserts or removes a
        // config group, so the store entry cannot relocate during this validity
        // window. Native systems cannot conflict with the declared resource read.
        let storage = unsafe {
            self.storage
                .borrow_projected_ref::<GizmoConfig, FieldStorage<GizmoConfig>>(|store| {
                    project(store).expect("gizmo config group was validated before projection")
                })?
        };
        let config = Py::new(py, PyGizmoConfig::from_borrowed(storage))?.into_any();
        let Some(registration) = registration else {
            return Ok(config);
        };
        let group = (registration.group)(py, &self.storage)?;
        PyTuple::new(py, [config, group])?.into_py_any(py)
    }

    #[pyo3(signature = (group = None))]
    pub fn config_mut(
        &mut self,
        py: Python<'_>,
        group: Option<&Bound<'_, PyType>>,
    ) -> PyResult<Py<PyAny>> {
        let registration = group.map(config_group_registration).transpose()?;
        let (project, missing_error): (
            fn(&mut GizmoConfigStore) -> Option<&mut GizmoConfig>,
            String,
        ) = match registration {
            Some(registration) => (
                registration.config_mut,
                missing_gizmo_config_group(registration.name),
            ),
            None => (
                |store| {
                    store
                        .get_config_mut::<DefaultGizmoConfigGroup>()
                        .map(|value| value.0)
                },
                DEFAULT_GIZMO_CONFIG_MISSING.to_owned(),
            ),
        };
        let mut store = self.as_mut()?;
        if project(&mut store).is_none() {
            return Err(PyRuntimeError::new_err(missing_error));
        }
        // SAFETY: as above, and ResMut[GizmoConfigStore] supplies the unique
        // parent access used to derive this mutable projection. When a group
        // was requested, its registration projects the disjoint second tuple
        // field from the same store entry.
        let storage = unsafe {
            self.storage
                .borrow_projected_mut::<GizmoConfig, FieldStorage<GizmoConfig>>(|store| {
                    project(store).expect("gizmo config group was validated before projection")
                })?
        };
        let config = Py::new(py, PyGizmoConfig::from_borrowed(storage))?.into_any();
        let Some(registration) = registration else {
            return Ok(config);
        };
        let group = (registration.group_mut)(py, &mut self.storage)?;
        PyTuple::new(py, [config, group])?.into_py_any(py)
    }
}
