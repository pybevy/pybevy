use bevy::{audio::SpatialListener, math::Vec3};
use pybevy_core::{
    ComponentStorage, Omittable, PyComponent,
    domain::{NON_ZERO, check_for},
    public_error::SPATIAL_EARS_COINCIDE,
    repr::ReprLiteral,
};
use pybevy_macros::pycomponent;
use pybevy_math::vec3::PyVec3;
use pyo3::{exceptions::PyValueError, prelude::*};

/// rodio's spatial source asserts in debug builds that the ears differ.
pub(crate) fn check_ears(left: Vec3, right: Vec3) -> PyResult<()> {
    if left == right {
        return Err(PyValueError::new_err(SPATIAL_EARS_COINCIDE));
    }
    Ok(())
}

fn validate_listener(listener: &SpatialListener) -> Result<(), &'static str> {
    if listener.left_ear_offset == listener.right_ear_offset {
        return Err(SPATIAL_EARS_COINCIDE);
    }
    Ok(())
}

#[pycomponent(SpatialListener, bridge, reflect_validate = validate_listener)]
#[pyclass(name = "SpatialListener", module = "pybevy.audio", extends = PyComponent)]
pub struct PySpatialListener {
    pub(crate) storage: ComponentStorage<SpatialListener>,
}

#[pymethods]
impl PySpatialListener {
    #[new]
    #[pyo3(signature = (
        gap=4.0,
        *,
        left_ear_offset=Omittable::default(),
        right_ear_offset=Omittable::default(),
    ))]
    pub fn new(
        gap: f32,
        left_ear_offset: Omittable<PyRef<'_, PyVec3>>,
        right_ear_offset: Omittable<PyRef<'_, PyVec3>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        let mut listener = SpatialListener::new(check_for::<Self, _>("gap", gap, NON_ZERO)?);
        if let Some(offset) = left_ear_offset.into_option() {
            listener.left_ear_offset = (&*offset).try_into()?;
        }
        if let Some(offset) = right_ear_offset.into_option() {
            listener.right_ear_offset = (&*offset).try_into()?;
        }
        check_ears(listener.left_ear_offset, listener.right_ear_offset)?;
        Ok(Self::from_owned(listener).into())
    }

    #[getter]
    pub fn left_ear_offset(&self) -> PyResult<PyVec3> {
        Ok(self.storage.borrow_field_as(|l| &l.left_ear_offset)?)
    }

    #[setter]
    pub fn set_left_ear_offset(&mut self, offset: PyVec3) -> PyResult<()> {
        let offset: Vec3 = offset.try_into()?;
        check_ears(offset, self.as_ref()?.right_ear_offset)?;
        self.as_mut()?.left_ear_offset = offset;
        Ok(())
    }

    #[getter]
    pub fn right_ear_offset(&self) -> PyResult<PyVec3> {
        Ok(self.storage.borrow_field_as(|l| &l.right_ear_offset)?)
    }

    #[setter]
    pub fn set_right_ear_offset(&mut self, offset: PyVec3) -> PyResult<()> {
        let offset: Vec3 = offset.try_into()?;
        check_ears(self.as_ref()?.left_ear_offset, offset)?;
        self.as_mut()?.right_ear_offset = offset;
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(listener) => format!(
                "SpatialListener(left_ear={}, right_ear={})",
                listener.left_ear_offset.repr_literal(),
                listener.right_ear_offset.repr_literal()
            ),
            Err(_) => "SpatialListener(<invalid>)".to_string(),
        }
    }
}
