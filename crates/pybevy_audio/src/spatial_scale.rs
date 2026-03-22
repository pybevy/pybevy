use bevy::audio::SpatialScale;
use pybevy_math::PyVec3;
use pyo3::{prelude::*, types::PyFloat};

fn parse_scale_arg(ob: &Bound<'_, PyAny>) -> PyResult<SpatialScale> {
    if let Ok(f) = ob.cast::<PyFloat>() {
        Ok(SpatialScale::new(f.extract()?))
    } else if let Ok(i) = ob.extract::<i64>() {
        // Handle int as float
        Ok(SpatialScale::new(i as f32))
    } else if let Ok(v) = ob.extract::<PyVec3>() {
        Ok(SpatialScale(v.into()))
    } else {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Expected float or Vec3",
        ))
    }
}

#[pyclass(name = "SpatialScale", frozen)]
#[derive(Debug, Clone, Copy)]
pub struct PySpatialScale {
    pub inner: SpatialScale,
}

impl From<SpatialScale> for PySpatialScale {
    fn from(scale: SpatialScale) -> Self {
        Self { inner: scale }
    }
}

impl From<PySpatialScale> for SpatialScale {
    fn from(scale: PySpatialScale) -> Self {
        scale.inner
    }
}

#[pymethods]
impl PySpatialScale {
    #[new]
    #[pyo3(signature = (scale=None))]
    pub fn new(scale: Option<Bound<'_, PyAny>>) -> PyResult<Self> {
        let inner = match scale {
            None => SpatialScale::new(1.0),
            Some(ref ob) => parse_scale_arg(ob)?,
        };
        Ok(Self { inner })
    }

    #[staticmethod]
    pub fn new_2d(scale: f32) -> Self {
        Self {
            inner: SpatialScale::new_2d(scale),
        }
    }

    pub fn to_vec3(&self) -> PyVec3 {
        PyVec3::from_vec3(self.inner.0)
    }

    fn __repr__(&self) -> String {
        format!("SpatialScale({:?})", self.inner.0)
    }

    fn __eq__(&self, other: &Self) -> bool {
        self.inner.0 == other.inner.0
    }
}
