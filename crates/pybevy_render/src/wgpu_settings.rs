use std::{borrow::Cow, fmt};

use bevy::render::settings::{Backends, Gles3MinorVersion, WgpuSettings, WgpuSettingsPriority};
use pybevy_core::{
    BorrowableStorage, FieldStorage, FromBorrowedStorage, Omittable, ValueStorage,
    repr::{ReprLiteral, object_repr, str_repr},
};
use pybevy_macros::{pyenum, pyfield, pyvalue};
use pyo3::prelude::*;

use crate::power_preference::PyPowerPreference;

#[pyvalue]
#[pyclass(name = "Backends", module = "pybevy.render", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyBackends {
    storage: ValueStorage<Backends>,
}

impl From<Backends> for PyBackends {
    fn from(value: Backends) -> Self {
        Self::from_owned(value)
    }
}

impl TryFrom<PyBackends> for Backends {
    type Error = PyErr;

    fn try_from(value: PyBackends) -> PyResult<Self> {
        value.to_bevy()
    }
}

impl TryFrom<&PyBackends> for Backends {
    type Error = PyErr;

    fn try_from(value: &PyBackends) -> PyResult<Self> {
        value.to_bevy()
    }
}

const BACKEND_FLAGS: [(Backends, &str); 6] = [
    (Backends::NOOP, "NOOP"),
    (Backends::VULKAN, "VULKAN"),
    (Backends::GL, "GL"),
    (Backends::METAL, "METAL"),
    (Backends::DX12, "DX12"),
    (Backends::BROWSER_WEBGPU, "BROWSER_WEBGPU"),
];

#[pymethods]
impl PyBackends {
    #[new]
    pub fn new() -> Self {
        Backends::default().into()
    }

    #[staticmethod]
    #[pyo3(name = "NOOP")]
    pub fn noop_flag() -> Self {
        Backends::NOOP.into()
    }

    #[staticmethod]
    #[pyo3(name = "VULKAN")]
    pub fn vulkan_flag() -> Self {
        Backends::VULKAN.into()
    }

    #[staticmethod]
    #[pyo3(name = "GL")]
    pub fn gl_flag() -> Self {
        Backends::GL.into()
    }

    #[staticmethod]
    #[pyo3(name = "METAL")]
    pub fn metal_flag() -> Self {
        Backends::METAL.into()
    }

    #[staticmethod]
    #[pyo3(name = "DX12")]
    pub fn dx12_flag() -> Self {
        Backends::DX12.into()
    }

    #[staticmethod]
    #[pyo3(name = "BROWSER_WEBGPU")]
    pub fn browser_webgpu_flag() -> Self {
        Backends::BROWSER_WEBGPU.into()
    }

    #[staticmethod]
    #[pyo3(name = "PRIMARY")]
    pub fn primary_flag() -> Self {
        Backends::PRIMARY.into()
    }

    #[staticmethod]
    #[pyo3(name = "SECONDARY")]
    pub fn secondary_flag() -> Self {
        Backends::SECONDARY.into()
    }

    fn __or__(&self, other: &Self) -> PyResult<Self> {
        Ok((self.to_bevy()? | other.to_bevy()?).into())
    }

    fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.to_bevy()? == other.to_bevy()?)
    }

    fn __copy__(&self) -> PyResult<Self> {
        Ok(self.to_bevy()?.into())
    }

    pub fn contains(&self, other: &Self) -> PyResult<bool> {
        Ok(self.to_bevy()?.contains(other.to_bevy()?))
    }

    pub fn insert(slf: &Bound<'_, Self>, other: Self) -> PyResult<()> {
        let other = other.to_bevy()?;
        slf.try_borrow_mut()?.as_mut()?.insert(other);
        Ok(())
    }

    pub fn remove(slf: &Bound<'_, Self>, other: Self) -> PyResult<()> {
        let other = other.to_bevy()?;
        slf.try_borrow_mut()?.as_mut()?.remove(other);
        Ok(())
    }

    pub fn toggle(slf: &Bound<'_, Self>, other: Self) -> PyResult<()> {
        let other = other.to_bevy()?;
        slf.try_borrow_mut()?.as_mut()?.toggle(other);
        Ok(())
    }

    pub fn set(slf: &Bound<'_, Self>, other: Self, value: bool) -> PyResult<()> {
        let other = other.to_bevy()?;
        slf.try_borrow_mut()?.as_mut()?.set(other, value);
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(backends_repr(self.to_bevy()?))
    }
}

impl Default for PyBackends {
    fn default() -> Self {
        Self::new()
    }
}

fn backends_repr(backends: Backends) -> String {
    let parts: Vec<_> = BACKEND_FLAGS
        .iter()
        .filter(|(flag, _)| backends.contains(*flag))
        .map(|(_, name)| format!("Backends.{name}"))
        .collect();
    if parts.is_empty() {
        "Backends()".to_string()
    } else {
        parts.join(" | ")
    }
}

#[pyenum(WgpuSettingsPriority)]
#[pyclass(
    name = "WgpuSettingsPriority",
    module = "pybevy.render",
    eq,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PyWgpuSettingsPriority {
    WebGPU,
    Functionality,
    WebGL2,
}

#[pyenum(Gles3MinorVersion)]
#[pyclass(
    name = "Gles3MinorVersion",
    module = "pybevy.render",
    eq,
    hash,
    frozen,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PyGles3MinorVersion {
    Automatic,
    Version0,
    Version1,
    Version2,
}

#[pyfield]
#[pyclass(name = "WgpuSettings", module = "pybevy.render", from_py_object)]
pub struct PyWgpuSettings {
    storage: FieldStorage<WgpuSettings>,
}

impl fmt::Debug for PyWgpuSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PyWgpuSettings").finish_non_exhaustive()
    }
}

impl PyWgpuSettings {
    pub fn read_only_snapshot(settings: &WgpuSettings) -> Self {
        Self::from_borrowed(FieldStorage::snapshot(settings))
    }
}

#[pymethods]
impl PyWgpuSettings {
    #[new]
    #[pyo3(signature = (
        *,
        device_label = Omittable::default(),
        backends = Omittable::default(),
        power_preference = Omittable::default(),
        priority = Omittable::default(),
        gles3_minor_version = Omittable::default(),
        force_fallback_adapter = false,
        adapter_name = None
    ))]
    pub fn new(
        device_label: Omittable<Option<String>>,
        backends: Omittable<Option<PyBackends>>,
        power_preference: Omittable<Py<PyPowerPreference>>,
        priority: Omittable<Py<PyWgpuSettingsPriority>>,
        gles3_minor_version: Omittable<Py<PyGles3MinorVersion>>,
        force_fallback_adapter: bool,
        adapter_name: Option<String>,
    ) -> PyResult<Self> {
        let mut settings = WgpuSettings::default();
        if let Omittable::Supplied(device_label) = device_label {
            settings.device_label = device_label.map(Cow::Owned);
        }
        if let Omittable::Supplied(backends) = backends {
            settings.backends = backends.map(Backends::try_from).transpose()?;
        }
        if let Omittable::Supplied(power_preference) = power_preference {
            settings.power_preference = (*power_preference.get()).into();
        }
        if let Omittable::Supplied(priority) = priority {
            settings.priority = (*priority.get()).into();
        }
        if let Omittable::Supplied(gles3_minor_version) = gles3_minor_version {
            settings.gles3_minor_version = (*gles3_minor_version.get()).into();
        }
        settings.force_fallback_adapter = force_fallback_adapter;
        settings.adapter_name = adapter_name;
        Ok(Self::from_owned(settings))
    }

    #[getter]
    pub fn device_label(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.device_label.as_deref().map(str::to_string))
    }

    #[setter]
    pub fn set_device_label(&mut self, device_label: Option<String>) -> PyResult<()> {
        self.as_mut()?.device_label = device_label.map(Cow::Owned);
        Ok(())
    }

    #[getter]
    pub fn backends(&self) -> PyResult<Option<PyBackends>> {
        Ok(self
            .storage
            .borrow_optional_field(
                "WgpuSettings.backends",
                |settings| &settings.backends,
                |settings| &mut settings.backends,
            )?
            .map(<PyBackends as FromBorrowedStorage<_>>::from_borrowed))
    }

    #[setter]
    pub fn set_backends(&mut self, backends: Option<PyBackends>) -> PyResult<()> {
        let backends = backends.map(Backends::try_from).transpose()?;
        self.as_mut()?.backends = backends;
        Ok(())
    }

    #[getter]
    pub fn power_preference(&self) -> PyResult<PyPowerPreference> {
        Ok(self.as_ref()?.power_preference.into())
    }

    #[setter]
    pub fn set_power_preference(&mut self, power_preference: PyPowerPreference) -> PyResult<()> {
        self.as_mut()?.power_preference = power_preference.into();
        Ok(())
    }

    #[getter]
    pub fn priority(&self) -> PyResult<PyWgpuSettingsPriority> {
        Ok(self.as_ref()?.priority.clone().into())
    }

    #[setter]
    pub fn set_priority(&mut self, priority: PyWgpuSettingsPriority) -> PyResult<()> {
        self.as_mut()?.priority = priority.into();
        Ok(())
    }

    #[getter]
    pub fn gles3_minor_version(&self) -> PyResult<PyGles3MinorVersion> {
        Ok(self.as_ref()?.gles3_minor_version.into())
    }

    #[setter]
    pub fn set_gles3_minor_version(&mut self, version: PyGles3MinorVersion) -> PyResult<()> {
        self.as_mut()?.gles3_minor_version = version.into();
        Ok(())
    }

    #[getter]
    pub fn force_fallback_adapter(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.force_fallback_adapter)
    }

    #[setter]
    pub fn set_force_fallback_adapter(&mut self, value: bool) -> PyResult<()> {
        self.as_mut()?.force_fallback_adapter = value;
        Ok(())
    }

    #[getter]
    pub fn adapter_name(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.adapter_name.clone())
    }

    #[setter]
    pub fn set_adapter_name(&mut self, adapter_name: Option<String>) -> PyResult<()> {
        self.as_mut()?.adapter_name = adapter_name;
        Ok(())
    }

    fn __copy__(&self) -> PyResult<Self> {
        Ok(Self::from_owned(self.as_ref()?.clone()))
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(wgpu_settings_repr(&*self.as_ref()?))
    }
}

pub(crate) fn wgpu_settings_repr(settings: &WgpuSettings) -> String {
    let optional_str = |value: Option<&str>| value.map_or("None".to_string(), str_repr);
    format!(
        "WgpuSettings(device_label={}, backends={}, power_preference={}, priority={}, gles3_minor_version={}, force_fallback_adapter={}, adapter_name={})",
        optional_str(settings.device_label.as_deref()),
        settings.backends.map_or("None".to_string(), backends_repr),
        object_repr(PyPowerPreference::from(settings.power_preference)),
        object_repr(PyWgpuSettingsPriority::from(settings.priority.clone())),
        object_repr(PyGles3MinorVersion::from(settings.gles3_minor_version)),
        settings.force_fallback_adapter.repr_literal(),
        optional_str(settings.adapter_name.as_deref()),
    )
}
