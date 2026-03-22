use bevy::math::{EulerRot, Quat};
use pybevy_core::{FromBorrowedStorage, ValueStorage};
use pyo3::{
    Bound, IntoPyObjectExt, basic::CompareOp, exceptions::PyTypeError, prelude::*, types::PyAny,
};

use crate::vec3::PyVec3;

#[pyclass(name = "EulerRot")]
#[derive(Debug, Clone, Copy)]
pub enum PyEulerRot {
    ZYX,
    ZXY,
    YXZ,
    YZX,
    XYZ,
    XZY,
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
        }
    }
}

#[pyclass(name = "Quat")]
#[derive(Debug, Clone)]
pub struct PyQuat {
    storage: ValueStorage<Quat>,
}

impl From<Quat> for PyQuat {
    fn from(quat: Quat) -> Self {
        PyQuat::from_quat(quat)
    }
}

impl From<&PyQuat> for Quat {
    fn from(py_quat: &PyQuat) -> Self {
        py_quat.storage.get().unwrap()
    }
}

impl From<PyQuat> for Quat {
    fn from(py_quat: PyQuat) -> Self {
        py_quat.storage.get().unwrap()
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
    fn as_ref(&self) -> PyResult<&Quat> {
        Ok(self.storage.as_ref()?)
    }

    #[inline(always)]
    fn as_mut(&mut self) -> PyResult<&mut Quat> {
        Ok(self.storage.as_mut()?)
    }

    #[inline(always)]
    pub fn get(&self) -> Quat {
        self.storage.get().unwrap()
    }

    pub const IDENTITY: PyQuat = PyQuat::quat(Quat::IDENTITY);
    pub const NAN: PyQuat = PyQuat::quat(Quat::NAN);
}

#[pymethods]
impl PyQuat {
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
    pub fn from_axis_angle(axis: PyVec3, angle: f32) -> Self {
        PyQuat {
            storage: ValueStorage::owned(Quat::from_axis_angle(axis.into(), angle)),
        }
    }

    #[staticmethod]
    pub fn from_scaled_axis(v: PyVec3) -> Self {
        PyQuat {
            storage: ValueStorage::owned(Quat::from_scaled_axis(v.into())),
        }
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
            storage: ValueStorage::owned(Quat::from_rotation_arc(start.into(), end.into())),
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
            storage: ValueStorage::owned(self.as_ref()?.lerp(rhs.get(), s)),
        })
    }

    pub fn slerp(&self, rhs: &PyQuat, s: f32) -> PyResult<Self> {
        Ok(PyQuat {
            storage: ValueStorage::owned(self.as_ref()?.slerp(rhs.get(), s)),
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

    pub fn __mul__(&self, other: &Bound<'_, PyAny>, py: Python) -> PyResult<Py<PyAny>> {
        let self_quat = self.as_ref()?;
        if let Ok(other_quat) = other.extract::<PyQuat>() {
            Ok(Py::new(
                py,
                PyQuat {
                    storage: ValueStorage::owned(*self_quat * other_quat.get()),
                },
            )?
            .into_any())
        } else if let Ok(other_vec3) = other.extract::<PyVec3>() {
            let v = *self_quat * other_vec3.get();
            Ok(Py::new(py, PyVec3::from_vec3(v))?.into_any())
        } else {
            Err(PyTypeError::new_err(
                "Quat can only be multiplied by Quat or Vec3",
            ))
        }
    }

    pub fn __rmul__(&self, other: &PyQuat, py: Python) -> PyResult<Py<PyAny>> {
        let self_quat = self.as_ref()?;
        Py::new(
            py,
            PyQuat {
                storage: ValueStorage::owned(other.get() * *self_quat),
            },
        )?
        .into_py_any(py)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let q = self.as_ref()?;
        Ok(format!("Quat({}, {}, {}, {})", q.x, q.y, q.z, q.w))
    }

    pub fn __richcmp__(&self, other: &PyQuat, op: CompareOp) -> PyResult<bool> {
        match op {
            CompareOp::Eq => Ok(self.get() == other.get()),
            CompareOp::Ne => Ok(self.get() != other.get()),
            _ => Err(PyTypeError::new_err("Unsupported comparison operation")),
        }
    }
}
