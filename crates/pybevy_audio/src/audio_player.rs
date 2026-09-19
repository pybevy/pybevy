use std::any::TypeId;

use bevy::{
    asset::Handle,
    audio::{AudioPlayer, AudioSource, Pitch},
};
use pybevy_core::{
    ComponentStorage, PyComponent, handle::PyHandle, public_error, registry::global_registry,
};
use pybevy_macros::pycomponent;
use pyo3::{PyTypeInfo, exceptions::PyTypeError, prelude::*, types::PyType};

use crate::{audio_source::PyAudioSource, pitch::PyPitch};

#[pyclass(name = "AudioPlayer", module = "pybevy.audio", extends = PyComponent, subclass)]
pub struct PyAudioPlayerBase;

#[pymethods]
impl PyAudioPlayerBase {
    #[new]
    pub fn new(py: Python<'_>, source: PyHandle) -> PyResult<Py<PyAny>> {
        if source.asset_type_id() == Some(TypeId::of::<Pitch>()) {
            return materialize_pitch_player(
                py,
                ComponentStorage::owned(AudioPlayer(Handle::<Pitch>::try_from(&source)?)),
            );
        }
        materialize_source_player(
            py,
            ComponentStorage::owned(AudioPlayer(Handle::<AudioSource>::try_from(&source)?)),
        )
    }

    #[classmethod]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        asset_type: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyType>> {
        let py = cls.py();
        if asset_type.is(py.get_type::<PyAudioSource>()) {
            return Ok(py.get_type::<PyAudioPlayer>().unbind());
        }
        if asset_type.is(py.get_type::<PyPitch>()) {
            return Ok(py.get_type::<PyPitchPlayer>().unbind());
        }
        Err(PyTypeError::new_err(
            public_error::AUDIO_PLAYER_SOURCE_TYPES,
        ))
    }

    #[getter]
    pub fn source(slf: &Bound<'_, Self>) -> PyResult<PyHandle> {
        if let Ok(player) = slf.cast::<PyPitchPlayer>() {
            return Ok(PyHandle::from(&player.try_borrow()?.storage.as_ref()?.0));
        }
        Ok(PyHandle::from(
            &slf.cast::<PyAudioPlayer>()?
                .try_borrow()?
                .storage
                .as_ref()?
                .0,
        ))
    }

    #[setter]
    pub fn set_source(slf: &Bound<'_, Self>, source: PyHandle) -> PyResult<()> {
        if let Ok(player) = slf.cast::<PyPitchPlayer>() {
            let handle = Handle::<Pitch>::try_from(&source)?;
            player.try_borrow_mut()?.storage.as_mut()?.0 = handle;
        } else {
            let handle = Handle::<AudioSource>::try_from(&source)?;
            slf.cast::<PyAudioPlayer>()?
                .try_borrow_mut()?
                .storage
                .as_mut()?
                .0 = handle;
        }
        Ok(())
    }
}

#[pycomponent(AudioPlayer<AudioSource>, bridge, materialize = materialize_source_player)]
#[pyclass(name = "_AudioSourcePlayer", module = "pybevy.audio", extends = PyAudioPlayerBase)]
pub struct PyAudioPlayer {
    pub(crate) storage: ComponentStorage<AudioPlayer<AudioSource>>,
}

#[pycomponent(AudioPlayer<Pitch>, bridge, materialize = materialize_pitch_player)]
#[pyclass(name = "_PitchPlayer", module = "pybevy.audio", extends = PyAudioPlayerBase)]
pub struct PyPitchPlayer {
    pub(crate) storage: ComponentStorage<AudioPlayer<Pitch>>,
}

fn materialize_source_player(
    py: Python<'_>,
    storage: ComponentStorage<AudioPlayer<AudioSource>>,
) -> PyResult<Py<PyAny>> {
    Ok(Py::new(
        py,
        PyClassInitializer::from(PyComponent)
            .add_subclass(PyAudioPlayerBase)
            .add_subclass(PyAudioPlayer { storage }),
    )?
    .into_any())
}

fn materialize_pitch_player(
    py: Python<'_>,
    storage: ComponentStorage<AudioPlayer<Pitch>>,
) -> PyResult<Py<PyAny>> {
    Ok(Py::new(
        py,
        PyClassInitializer::from(PyComponent)
            .add_subclass(PyAudioPlayerBase)
            .add_subclass(PyPitchPlayer { storage }),
    )?
    .into_any())
}

#[pymethods]
impl PyAudioPlayer {
    #[new]
    fn new(source: PyHandle) -> PyResult<PyClassInitializer<Self>> {
        let handle = Handle::<AudioSource>::try_from(&source)?;
        Ok(PyClassInitializer::from(PyComponent)
            .add_subclass(PyAudioPlayerBase)
            .add_subclass(Self {
                storage: ComponentStorage::owned(AudioPlayer(handle)),
            }))
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(player) => format!("AudioPlayer(source={:?})", player.0.id()),
            Err(_) => "AudioPlayer(<invalid>)".to_string(),
        }
    }
}

#[pymethods]
impl PyPitchPlayer {
    #[new]
    fn new(source: PyHandle) -> PyResult<PyClassInitializer<Self>> {
        let handle = Handle::<Pitch>::try_from(&source)?;
        Ok(PyClassInitializer::from(PyComponent)
            .add_subclass(PyAudioPlayerBase)
            .add_subclass(Self {
                storage: ComponentStorage::owned(AudioPlayer(handle)),
            }))
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(player) => format!("AudioPlayer[Pitch](source={:?})", player.0.id()),
            Err(_) => "AudioPlayer[Pitch](<invalid>)".to_string(),
        }
    }
}

pub fn register_audio_players(py: Python<'_>) {
    assert!(global_registry::register_component_bridge_alias(
        PyAudioPlayerBase::type_object_raw(py),
        PyAudioPlayer::type_object_raw(py),
    ));
}
