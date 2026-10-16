use bevy::math::{Affine3A, EulerRot, Mat3, Mat4, Quat, Vec2, Vec3, Vec3A};
use pybevy_core::{
    FromBorrowedStorage, StorageMut, StorageRef, ValueStorage,
    domain::{FINITE, check},
    public_error::{
        QUAT_NOT_NORMALIZED, ROTATION_ARC_NOT_NORMALIZED, ROTATION_AXES_NOT_NORMALIZED,
        UNSUPPORTED_COMPARISON, quat_axis_not_normalized,
    },
    repr::ReprLiteral,
};
use pybevy_macros::pyenum;
use pyo3::{
    Bound,
    basic::CompareOp,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyAny, PyDict, PyTuple},
};

use crate::{
    affine3a::PyAffine3A,
    mat3::PyMat3,
    mat4::{PyMat4, look_at_inputs, look_to_axes},
    richcmp::comparison_result,
    vec2::PyVec2,
    vec3::PyVec3,
    vec3a::PyVec3A,
};

#[pyenum(EulerRot)]
#[pyclass(
    name = "EulerRot",
    module = "pybevy.math",
    frozen,
    eq,
    hash,
    from_py_object
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    #[pyo3(signature = (*_args, **_kwargs), text_signature = "()")]
    pub fn py_new(
        _args: &Bound<'_, PyTuple>,
        _kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        if _args.is_empty() && _kwargs.is_none_or(|kwargs| kwargs.is_empty()) {
            return Ok(Self::IDENTITY);
        }
        Err(PyTypeError::new_err(
            "Quat cannot be constructed directly (raw xyzw components are error-prone); \
             use Quat.from_euler(EulerRot.XYZ, x, y, z), \
             Quat.from_axis_angle(axis, angle) with a normalized axis, or \
             Quat.IDENTITY. Quat.from_xyzw(x, y, z, w) takes raw components and \
             does not normalize them.",
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
        // glam does not enforce its normalized-axis precondition in release builds.
        let axis: Vec3 = axis.try_into()?;
        let angle = check("Quat", "angle", angle, FINITE)?;
        let axis = check("Quat", "axis", axis, FINITE)?;
        if !axis.is_normalized() {
            return Err(PyValueError::new_err(quat_axis_not_normalized(
                axis.length(),
            )));
        }
        Ok(PyQuat {
            storage: ValueStorage::owned(Quat::from_axis_angle(axis, angle)),
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
            return Err(PyValueError::new_err(ROTATION_ARC_NOT_NORMALIZED));
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
        if let Ok(other_quat) = other.cast::<PyQuat>() {
            let other_quat = Quat::try_from(&*other_quat.try_borrow()?)?;
            Ok(Py::new(py, Self::from_quat(*self_quat * other_quat))?.into_any())
        } else if let Ok(other_vec3) = other.cast::<PyVec3>() {
            let v = *self_quat * Vec3::try_from(&*other_vec3.try_borrow()?)?;
            Ok(Py::new(py, PyVec3::from_vec3(v))?.into_any())
        } else if let Ok(other_vec3a) = other.cast::<PyVec3A>() {
            let v = *self_quat * Vec3A::try_from(&*other_vec3a.try_borrow()?)?;
            Ok(Py::new(py, PyVec3A::from_vec3a(v))?.into_any())
        } else if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_quat(*self_quat * scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __add__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(other_quat) = other.cast::<PyQuat>() {
            let other_quat = Quat::try_from(&*other_quat.try_borrow()?)?;
            Ok(Py::new(py, Self::from_quat(*value + other_quat))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __sub__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(other_quat) = other.cast::<PyQuat>() {
            let other_quat = Quat::try_from(&*other_quat.try_borrow()?)?;
            Ok(Py::new(py, Self::from_quat(*value - other_quat))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __rmul__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(other_quat) = other.cast::<PyQuat>() {
            let other_quat = Quat::try_from(&*other_quat.try_borrow()?)?;
            Ok(Py::new(py, Self::from_quat(other_quat * *value))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __truediv__(&self, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let value = self.as_ref()?;
        if let Ok(scalar) = other.extract::<f32>() {
            Ok(Py::new(py, Self::from_quat(*value / scalar))?.into_any())
        } else {
            Ok(py.NotImplemented().into_any())
        }
    }

    pub fn __neg__(&self) -> PyResult<Self> {
        Ok(Self::from_quat(-*self.as_ref()?))
    }

    pub fn __copy__(&self) -> PyResult<Self> {
        Ok(PyQuat {
            storage: ValueStorage::owned(*self.as_ref()?),
        })
    }

    pub fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> PyResult<Self> {
        self.__copy__()
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let q = self.as_ref()?;
        Ok(q.repr_literal())
    }

    pub fn __richcmp__(
        &self,
        other: &Bound<'_, PyAny>,
        op: CompareOp,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let Ok(other_quat) = other.extract::<PyQuat>() else {
            return Ok(py.NotImplemented());
        };
        let a = self.try_get()?;
        let b = other_quat.try_get()?;
        let result = match op {
            CompareOp::Eq => a == b,
            CompareOp::Ne => a != b,
            _ => return Err(PyTypeError::new_err(UNSUPPORTED_COMPARISON)),
        };
        Ok(comparison_result(py, result))
    }

    pub fn mul_vec3(&self, rhs: &PyVec3) -> PyResult<PyVec3> {
        let rotation = normalized_quat(*self.as_ref()?)?;
        Ok(rotation.mul_vec3(rhs.try_into()?).into())
    }

    pub fn mul_quat(&self, rhs: &PyQuat) -> PyResult<PyQuat> {
        Ok(self.as_ref()?.mul_quat(*rhs.as_ref()?).into())
    }

    pub fn is_normalized(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_normalized())
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn is_nan(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_nan())
    }

    pub fn is_near_identity(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_near_identity())
    }

    pub fn angle_between(&self, rhs: &PyQuat) -> PyResult<f32> {
        let rotation = normalized_quat(*self.as_ref()?)?;
        Ok(rotation.angle_between(normalized_quat(*rhs.as_ref()?)?))
    }

    pub fn rotate_towards(&self, rhs: &PyQuat, max_angle: f32) -> PyResult<PyQuat> {
        let rotation = normalized_quat(*self.as_ref()?)?;
        Ok(rotation
            .rotate_towards(normalized_quat(*rhs.as_ref()?)?, max_angle)
            .into())
    }

    #[staticmethod]
    pub fn from_mat3(mat: &PyMat3) -> PyResult<PyQuat> {
        let mat: Mat3 = mat.try_into()?;
        rotation_axes(mat.x_axis, mat.y_axis, mat.z_axis)?;
        Ok(Quat::from_mat3(&mat).into())
    }

    #[staticmethod]
    pub fn from_mat4(mat: &PyMat4) -> PyResult<PyQuat> {
        let mat: Mat4 = mat.try_into()?;
        rotation_axes(
            mat.x_axis.truncate(),
            mat.y_axis.truncate(),
            mat.z_axis.truncate(),
        )?;
        Ok(Quat::from_mat4(&mat).into())
    }

    #[staticmethod]
    pub fn from_affine3a(a: &PyAffine3A) -> PyResult<PyQuat> {
        let a: Affine3A = a.try_into()?;
        rotation_axes(
            a.matrix3.x_axis.into(),
            a.matrix3.y_axis.into(),
            a.matrix3.z_axis.into(),
        )?;
        Ok(Quat::from_affine3a(&a).into())
    }

    #[staticmethod]
    pub fn look_to_lh(dir: &PyVec3, up: &PyVec3) -> PyResult<PyQuat> {
        let (dir, up) = look_to_axes(dir, up)?;
        Ok(Quat::look_to_lh(dir, up).into())
    }

    #[staticmethod]
    pub fn look_to_rh(dir: &PyVec3, up: &PyVec3) -> PyResult<PyQuat> {
        let (dir, up) = look_to_axes(dir, up)?;
        Ok(Quat::look_to_rh(dir, up).into())
    }

    #[staticmethod]
    pub fn look_at_lh(eye: &PyVec3, center: &PyVec3, up: &PyVec3) -> PyResult<PyQuat> {
        let (_, dir, up) = look_at_inputs(eye, center, up)?;
        Ok(Quat::look_to_lh(dir, up).into())
    }

    #[staticmethod]
    pub fn look_at_rh(eye: &PyVec3, center: &PyVec3, up: &PyVec3) -> PyResult<PyQuat> {
        let (_, dir, up) = look_at_inputs(eye, center, up)?;
        Ok(Quat::look_to_rh(dir, up).into())
    }

    #[staticmethod]
    pub fn from_rotation_arc_2d(from_: &PyVec2, to: &PyVec2) -> PyResult<PyQuat> {
        let (from, to): (Vec2, Vec2) = (from_.try_into()?, to.try_into()?);
        if !from.is_normalized() || !to.is_normalized() {
            return Err(PyValueError::new_err(ROTATION_ARC_NOT_NORMALIZED));
        }
        Ok(Quat::from_rotation_arc_2d(from, to).into())
    }

    #[staticmethod]
    pub fn from_rotation_arc_colinear(from_: &PyVec3, to: &PyVec3) -> PyResult<PyQuat> {
        let (from, to): (Vec3, Vec3) = (from_.try_into()?, to.try_into()?);
        if !from.is_normalized() || !to.is_normalized() {
            return Err(PyValueError::new_err(ROTATION_ARC_NOT_NORMALIZED));
        }
        Ok(Quat::from_rotation_arc_colinear(from, to).into())
    }

    #[staticmethod]
    pub fn from_array(a: (f32, f32, f32, f32)) -> PyQuat {
        Quat::from_array([a.0, a.1, a.2, a.3]).into()
    }

    pub fn to_array(&self) -> PyResult<(f32, f32, f32, f32)> {
        let [x, y, z, w] = self.as_ref()?.to_array();
        Ok((x, y, z, w))
    }

    pub fn abs_diff_eq(&self, rhs: &PyQuat, max_abs_diff: f32) -> PyResult<bool> {
        Ok(self.as_ref()?.abs_diff_eq(*rhs.as_ref()?, max_abs_diff))
    }
}

fn normalized_quat(rotation: Quat) -> PyResult<Quat> {
    if !rotation.is_normalized() {
        return Err(PyValueError::new_err(QUAT_NOT_NORMALIZED));
    }
    Ok(rotation)
}

fn rotation_axes(x_axis: Vec3, y_axis: Vec3, z_axis: Vec3) -> PyResult<()> {
    if !(x_axis.is_normalized() && y_axis.is_normalized() && z_axis.is_normalized()) {
        return Err(PyValueError::new_err(ROTATION_AXES_NOT_NORMALIZED));
    }
    Ok(())
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
