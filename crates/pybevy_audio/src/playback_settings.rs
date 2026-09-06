use std::time::Duration;

use bevy::audio::{PlaybackMode, PlaybackSettings};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::{exceptions::PyValueError, prelude::*};

use crate::{playback_mode::PyPlaybackMode, spatial_scale::PySpatialScale, volume::PyVolume};

#[pycomponent(PlaybackSettings, bridge, view_fields = [speed, paused, muted, spatial])]
#[pyclass(name = "PlaybackSettings", module = "pybevy.audio", extends = PyComponent)]
pub struct PyPlaybackSettings {
    pub(crate) storage: ComponentStorage<PlaybackSettings>,
}

#[pymethods]
impl PyPlaybackSettings {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (
        *,
        mode = None,
        volume = None,
        speed = 1.0,
        paused = false,
        muted = false,
        spatial = false,
        start_position = None,
        duration = None,
        spatial_scale = None,
    ))]
    pub fn new(
        mode: Option<PyPlaybackMode>,
        volume: Option<PyVolume>,
        speed: f32,
        paused: bool,
        muted: bool,
        spatial: bool,
        start_position: Option<Duration>,
        duration: Option<Duration>,
        spatial_scale: Option<PySpatialScale>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let settings = PlaybackSettings {
            mode: mode.map(Into::into).unwrap_or(PlaybackMode::Once),
            volume: volume.map(Into::into).unwrap_or_default(),
            speed: validate_speed(speed)?,
            paused,
            muted,
            spatial,
            start_position,
            duration,
            spatial_scale: spatial_scale.map(|s| s.inner),
        };
        Ok(Self::from_owned(settings).into())
    }

    #[staticmethod]
    #[pyo3(name = "ONCE")]
    pub fn once(py: Python) -> PyResult<Py<PyPlaybackSettings>> {
        Py::new(py, Self::from_owned(PlaybackSettings::ONCE))
    }

    #[staticmethod]
    #[pyo3(name = "LOOP")]
    pub fn loop_settings(py: Python) -> PyResult<Py<PyPlaybackSettings>> {
        Py::new(py, Self::from_owned(PlaybackSettings::LOOP))
    }

    #[staticmethod]
    #[pyo3(name = "DESPAWN")]
    pub fn despawn(py: Python) -> PyResult<Py<PyPlaybackSettings>> {
        Py::new(py, Self::from_owned(PlaybackSettings::DESPAWN))
    }

    #[staticmethod]
    #[pyo3(name = "REMOVE")]
    pub fn remove(py: Python) -> PyResult<Py<PyPlaybackSettings>> {
        Py::new(py, Self::from_owned(PlaybackSettings::REMOVE))
    }

    #[getter]
    pub fn mode(&self) -> PyResult<PyPlaybackMode> {
        Ok(self.as_ref()?.mode.into())
    }

    #[setter]
    pub fn set_mode(&mut self, mode: PyPlaybackMode) -> PyResult<()> {
        self.as_mut()?.mode = mode.into();
        Ok(())
    }

    #[getter]
    pub fn volume(&self) -> PyResult<PyVolume> {
        Ok(self.as_ref()?.volume.into())
    }

    #[setter]
    pub fn set_volume(&mut self, volume: PyVolume) -> PyResult<()> {
        self.as_mut()?.volume = volume.into();
        Ok(())
    }

    #[getter]
    pub fn speed(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.speed)
    }

    #[setter]
    pub fn set_speed(&mut self, speed: f32) -> PyResult<()> {
        self.as_mut()?.speed = validate_speed(speed)?;
        Ok(())
    }

    #[getter]
    pub fn paused(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.paused)
    }

    #[setter]
    pub fn set_paused(&mut self, paused: bool) -> PyResult<()> {
        self.as_mut()?.paused = paused;
        Ok(())
    }

    #[getter]
    pub fn muted(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.muted)
    }

    #[setter]
    pub fn set_muted(&mut self, muted: bool) -> PyResult<()> {
        self.as_mut()?.muted = muted;
        Ok(())
    }

    #[getter]
    pub fn spatial(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.spatial)
    }

    #[setter]
    pub fn set_spatial(&mut self, spatial: bool) -> PyResult<()> {
        self.as_mut()?.spatial = spatial;
        Ok(())
    }

    #[getter]
    pub fn start_position(&self) -> PyResult<Option<Duration>> {
        Ok(self.as_ref()?.start_position)
    }

    #[setter]
    pub fn set_start_position(&mut self, pos: Option<Duration>) -> PyResult<()> {
        self.as_mut()?.start_position = pos;
        Ok(())
    }

    #[getter]
    pub fn duration(&self) -> PyResult<Option<Duration>> {
        Ok(self.as_ref()?.duration)
    }

    #[setter]
    pub fn set_duration(&mut self, duration: Option<Duration>) -> PyResult<()> {
        self.as_mut()?.duration = duration;
        Ok(())
    }

    #[getter]
    pub fn spatial_scale(&self) -> PyResult<Option<PySpatialScale>> {
        Ok(self.as_ref()?.spatial_scale.map(|s| s.into()))
    }

    #[setter]
    pub fn set_spatial_scale(&mut self, scale: Option<PySpatialScale>) -> PyResult<()> {
        self.as_mut()?.spatial_scale = scale.map(|s| s.inner);
        Ok(())
    }

    pub fn with_volume(&self, py: Python, volume: PyVolume) -> PyResult<Py<Self>> {
        let mut settings = *self.as_ref()?;
        settings.volume = volume.into();
        Py::new(py, Self::from_owned(settings))
    }

    pub fn with_speed(&self, py: Python, speed: f32) -> PyResult<Py<Self>> {
        let mut settings = *self.as_ref()?;
        settings.speed = validate_speed(speed)?;
        Py::new(py, Self::from_owned(settings))
    }

    pub fn with_spatial(&self, py: Python, spatial: bool) -> PyResult<Py<Self>> {
        let mut settings = *self.as_ref()?;
        settings.spatial = spatial;
        Py::new(py, Self::from_owned(settings))
    }

    pub fn with_start_position(&self, py: Python, start_position: Duration) -> PyResult<Py<Self>> {
        let mut settings = *self.as_ref()?;
        settings.start_position = Some(start_position);
        Py::new(py, Self::from_owned(settings))
    }

    pub fn with_duration(&self, py: Python, duration: Duration) -> PyResult<Py<Self>> {
        let mut settings = *self.as_ref()?;
        settings.duration = Some(duration);
        Py::new(py, Self::from_owned(settings))
    }

    pub fn with_spatial_scale(
        &self,
        py: Python,
        spatial_scale: PySpatialScale,
    ) -> PyResult<Py<Self>> {
        let mut settings = *self.as_ref()?;
        settings.spatial_scale = Some(spatial_scale.inner);
        Py::new(py, Self::from_owned(settings))
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(settings) => format!(
                "PlaybackSettings(mode={:?}, volume={:?}, speed={}, paused={}, muted={}, spatial={})",
                settings.mode,
                settings.volume,
                settings.speed,
                settings.paused,
                settings.muted,
                settings.spatial
            ),
            Err(_) => "PlaybackSettings(<invalid>)".to_string(),
        }
    }
}

fn validate_speed(speed: f32) -> PyResult<f32> {
    if speed.is_finite() {
        Ok(speed)
    } else {
        Err(PyValueError::new_err(format!(
            "speed must be finite (got {speed})"
        )))
    }
}
