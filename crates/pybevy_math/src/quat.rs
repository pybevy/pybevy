use bevy::math::{EulerRot, Quat, Vec3A};
use pybevy_core::{FromBorrowedStorage, StorageMut, StorageRef, ValueStorage};
use pyo3::{
    Bound, IntoPyObjectExt,
    basic::CompareOp,
    exceptions::PyTypeError,
    prelude::*,
    types::{PyAny, PyDict, PyTuple},
};

use crate::{vec3::PyVec3, vec3a::PyVec3A};

#[pyclass(name = "EulerRot", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone, Copy)]
pub enum PyEulerRot {
    ZYX,
    ZXY,
    YXZ,
    YZX,
    XYZ,
    XZY,
    ZYZ,
    ZXZ,
    YXY,
    YZY,
    XYX,
    XZX,
    ZYXEx,
    ZXYEx,
    YXZEx,
    YZXEx,
    XYZEx,
    XZYEx,
    ZYZEx,
    ZXZEx,
    YXYEx,
    YZYEx,
    XYXEx,
    XZXEx,
}

impl From<PyEulerRot> for EulerRot {
    fn from(rot: PyEulerRot) -> Self {
        match rot {
            PyEulerRot::ZYX => EulerRot::ZYX,
            PyEulerRot::ZXY => EulerRot::ZXY,
            PyEulerRot::YXZ => EulerRot::YXZ,
            PyEulerRot::YZX => EulerRot::YZX,
            PyEulerRot::XYZ => EulerRot::XYZ,
            PyEulerRot::XZY => EulerRot::XZY,
            PyEulerRot::ZYZ => EulerRot::ZYZ,
            PyEulerRot::ZXZ => EulerRot::ZXZ,
            PyEulerRot::YXY => EulerRot::YXY,
            PyEulerRot::YZY => EulerRot::YZY,
            PyEulerRot::XYX => EulerRot::XYX,
            PyEulerRot::XZX => EulerRot::XZX,
            PyEulerRot::ZYXEx => EulerRot::ZYXEx,
            PyEulerRot::ZXYEx => EulerRot::ZXYEx,
            PyEulerRot::YXZEx => EulerRot::YXZEx,
            PyEulerRot::YZXEx => EulerRot::YZXEx,
            PyEulerRot::XYZEx => EulerRot::XYZEx,
            PyEulerRot::XZYEx => EulerRot::XZYEx,
            PyEulerRot::ZYZEx => EulerRot::ZYZEx,
            PyEulerRot::ZXZEx => EulerRot::ZXZEx,
            PyEulerRot::YXYEx => EulerRot::YXYEx,
            PyEulerRot::YZYEx => EulerRot::YZYEx,
            PyEulerRot::XYXEx => EulerRot::XYXEx,
            PyEulerRot::XZXEx => EulerRot::XZXEx,
        }
    }
}

#[pyclass(name = "Quat", module = "pybevy.math", from_py_object)]
#[derive(Debug, Clone)]
pub struct PyQuat {
    storage: ValueStorage<Quat>,
}

impl From<Quat> for PyQuat {
    fn from(quat: Quat) -> Self {
        PyQuat::from_quat(quat)
    }
}

impl TryFrom<&PyQuat> for Quat {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_quat: &PyQuat) -> PyResult<Self> {
        Ok(py_quat.storage.get()?)
    }
}

impl TryFrom<PyQuat> for Quat {
    type Error = PyErr;

    #[inline(always)]
    fn try_from(py_quat: PyQuat) -> PyResult<Self> {
        Ok(py_quat.storage.get()?)
    }
}

impl FromBorrowedStorage<ValueStorage<Quat>> for PyQuat {
    fn from_borrowed(storage: ValueStorage<Quat>) -> Self {
        PyQuat { storage }
    }
}

impl PyQuat {
    pub fn from_quat(quat: Quat) -> Self {
        PyQuat {
            storage: ValueStorage::owned(quat),
        }
    }

    #[inline(always)]
    pub const fn quat(quat: Quat) -> Self {
        PyQuat {
            storage: ValueStorage::owned(quat),
        }
    }

    #[inline(always)]
    fn as_ref(&self) -> PyResult<StorageRef<'_, Quat>> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<StorageMut<'_, Quat>> {
        Ok(self.storage.as_mut()?)
    }

    #[inline(always)]
    pub fn try_get(&self) -> PyResult<Quat> {
        Ok(self.storage.get()?)
    }

    pub const IDENTITY: PyQuat = PyQuat::quat(Quat::IDENTITY);
    pub const NAN: PyQuat = PyQuat::quat(Quat::NAN);
}

#[pymethods]
impl PyQuat {
    #[new]
    #[pyo3(signature = (*_args, **_kwargs))]
    pub fn py_new(
        _args: &Bound<'_, PyTuple>,
        _kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        Err(PyTypeError::new_err(
            "Quat cannot be constructed directly (raw xyzw components are error-prone); \
             use Quat.from_xyzw(x, y, z, w), Quat.from_axis_angle(axis, angle), \
             Quat.from_euler(EulerRot.XYZ, x, y, z), or Quat.IDENTITY",
        ))
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    pub fn identity() -> Self {
        Self::quat(Quat::IDENTITY)
    }
    #[staticmethod]
    #[pyo3(name = "NAN")]
    pub fn nan() -> Self {
        Self::quat(Quat::NAN)
    }

    #[staticmethod]
    pub fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self {
        PyQuat {
            storage: ValueStorage::owned(Quat::from_xyzw(x, y, z, w)),
        }
    }

    #[staticmethod]
    pub fn from_axis_angle(axis: PyVec3, angle: f32) -> PyResult<Self> {
        Ok(PyQuat {
            storage: ValueStorage::owned(Quat::from_axis_angle(axis.try_into()?, angle)),
        })
    }

    #[staticmethod]
    pub fn from_scaled_axis(v: PyVec3) -> PyResult<Self> {
        Ok(PyQuat {
            storage: ValueStorage::owned(Quat::from_scaled_axis(v.try_into()?)),
        })
    }

    #[staticmethod]
    pub fn from_rotation_x(x: f32) -> Self {
        PyQuat {
            storage: ValueStorage::owned(Quat::from_rotation_x(x)),
        }
    }

    #[staticmethod]
    pub fn from_rotation_y(y: f32) -> Self {
        PyQuat {
            storage: ValueStorage::owned(Quat::from_rotation_y(y)),
        }
    }

    #[staticmethod]
    pub fn from_rotation_z(z: f32) -> Self {
        PyQuat {
            storage: ValueStorage::owned(Quat::from_rotation_z(z)),
        }
    }

    #[staticmethod]
    pub fn from_rotation_arc(start: PyVec3, end: PyVec3) -> PyResult<Self> {
        if !start.is_normalized()? || !end.is_normalized()? {
            return Err(PyTypeError::new_err(
                "start and end vectors must be normalized",
            ));
        }
        Ok(PyQuat {
            storage: ValueStorage::owned(Quat::from_rotation_arc(
                start.try_into()?,
                end.try_into()?,
            )),
        })
    }

    #[getter]
    pub fn x(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.x)
    }

    #[setter]
    pub fn set_x(&mut self, x: f32) -> PyResult<()> {
        self.as_mut()?.x = x;
        Ok(())
    }

    #[getter]
    pub fn y(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.y)
    }

    #[setter]
    pub fn set_y(&mut self, y: f32) -> PyResult<()> {
        self.as_mut()?.y = y;
        Ok(())
    }

    #[getter]
    pub fn z(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.z)
    }

    #[setter]
    pub fn set_z(&mut self, z: f32) -> PyResult<()> {
        self.as_mut()?.z = z;
        Ok(())
    }

    #[getter]
    pub fn w(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.w)
    }

    #[setter]
    pub fn set_w(&mut self, w: f32) -> PyResult<()> {
        self.as_mut()?.w = w;
        Ok(())
    }

    pub fn length(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length())
    }

    pub fn dot(&self, rhs: &PyQuat) -> PyResult<f32> {
        Ok(self.as_ref()?.dot(rhs.try_get()?))
    }

    pub fn length_squared(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.length_squared())
    }

    pub fn normalize(&self) -> PyResult<Self> {
        Ok(PyQuat {
            storage: ValueStorage::owned(self.as_ref()?.normalize()),
        })
    }

    pub fn conjugate(&self) -> PyResult<Self> {
        Ok(PyQuat {
            storage: ValueStorage::owned(self.as_ref()?.conjugate()),
        })
    }

    pub fn inverse(&self) -> PyResult<Self> {
        Ok(PyQuat {
            storage: ValueStorage::owned(self.as_ref()?.inverse()),
        })
    }

    pub fn lerp(&self, rhs: &PyQuat, s: f32) -> PyResult<Self> {
        Ok(PyQuat {
            storage: ValueStorage::owned(self.as_ref()?.lerp(rhs.try_get()?, s)),
        })
    }

    pub fn slerp(&self, rhs: &PyQuat, s: f32) -> PyResult<Self> {
        Ok(PyQuat {
            storage: ValueStorage::owned(self.as_ref()?.slerp(rhs.try_get()?, s)),
        })
    }

    #[staticmethod]
    pub fn from_euler(order: PyEulerRot, x: f32, y: f32, z: f32) -> Self {
        PyQuat {
            storage: ValueStorage::owned(Quat::from_euler(order.into(), x, y, z)),
        }
    }

    pub fn to_euler(&self, order: PyEulerRot) -> PyResult<(f32, f32, f32)> {
        Ok(self.as_ref()?.to_euler(order.into()))
    }

    pub fn to_axis_angle(&self) -> PyResult<(PyVec3, f32)> {
        let (axis, angle) = self.as_ref()?.to_axis_angle();
        Ok((PyVec3::from_vec3(axis), angle))
    }

    pub fn to_scaled_axis(&self) -> PyResult<PyVec3> {
        Ok(PyVec3::from_vec3(self.as_ref()?.to_scaled_axis()))
    }

    pub fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python) -> PyResult<Py<PyAny>> {
        let self_quat = self.as_ref()?;
        if let Ok(other_quat) = other.extract::<PyQuat>() {
            Ok(Py::new(
                py,
                PyQuat {
                    storage: ValueStorage::owned(*self_quat * other_quat.try_get()?),
                },
            )?
            .into_any())
        } else if let Ok(other_vec3) = other.extract::<PyVec3>() {
            let v = *self_quat * other_vec3.try_get()?;
            Ok(Py::new(py, PyVec3::from_vec3(v))?.into_any())
        } else if let Ok(other_vec3a) = other.extract::<PyVec3A>() {
            let v = *self_quat * Vec3A::try_from(&other_vec3a)?;
            Ok(Py::new(py, PyVec3A::from_vec3a(v))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __rmul__(&self, other: &PyQuat, py: Python) -> PyResult<Py<PyAny>> {
        let self_quat = self.as_ref()?;
        Py::new(
            py,
            PyQuat {
                storage: ValueStorage::owned(other.try_get()? * *self_quat),
            },
        )?
        .into_py_any(py)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let q = self.as_ref()?;
        Ok(format!("Quat({}, {}, {}, {})", q.x, q.y, q.z, q.w))
    }

    pub fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(other_quat) = other.extract::<PyQuat>() {
            return match op {
                CompareOp::Eq => Ok(self.try_get()? == other_quat.try_get()?),
                CompareOp::Ne => Ok(self.try_get()? != other_quat.try_get()?),
                _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
            };
        }
        Err(PyTypeError::new_err(
            "Can only compare Quat with another Quat",
        ))
    }
}

/// Accept a PyQuat, matching bevy's `impl Into<Quat>` parameters.
pub fn extract_quat_from_any(obj: &Bound<'_, PyAny>) -> PyResult<Quat> {
    if let Ok(value) = obj.extract::<PyQuat>() {
        return Quat::try_from(value);
    }
    Err(PyTypeError::new_err(format!(
        "expected Quat, got {}",
        obj.get_type().name()?
    )))
}
