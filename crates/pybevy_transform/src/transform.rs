use bevy::{
    math::{Dir3, Mat4, Quat, Vec3},
    transform::components::Transform,
};

use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pybevy_math::{
    affine3a::PyAffine3A, bounding::PyIsometry3d, dir3::PyDir3, mat4::PyMat4, quat::PyQuat,
    vec3::PyVec3,
};
use pyo3::{exceptions::PyTypeError, prelude::*};

#[pycomponent(Transform, bridge, view_fields = [
    translation => [finite],
    rotation => [finite],
    scale => [finite]
])]
#[pyclass(name = "Transform", extends = pybevy_core::PyComponent, eq)]
#[derive(Debug)]
pub struct PyTransform {
    pub(crate) storage: ComponentStorage<Transform>,
}

impl PartialEq for PyTransform {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

pub(crate) fn format_transform_repr(
    name: &str,
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
) -> String {
    format!(
        "{name}(translation=Vec3({}, {}, {}), rotation=Quat({}, {}, {}, {}), scale=Vec3({}, {}, {}))",
        translation.x,
        translation.y,
        translation.z,
        rotation.x,
        rotation.y,
        rotation.z,
        rotation.w,
        scale.x,
        scale.y,
        scale.z,
    )
}

#[pymethods]
impl PyTransform {
    #[new]
    #[pyo3(signature = (translation = PyVec3::ZERO, rotation = PyQuat::IDENTITY, scale = PyVec3::ONE))]
    pub fn new(
        translation: PyVec3,
        rotation: PyQuat,
        scale: PyVec3,
    ) -> PyResult<PyClassInitializer<Self>> {
        let transform = Transform {
            translation: translation.try_into()?,
            rotation: rotation.try_into()?,
            scale: scale.try_into()?,
        };

        Ok((transform.into(), PyComponent).into())
    }

    #[staticmethod]
    pub fn from_xyz(py: Python, x: f32, y: f32, z: f32) -> PyResult<Py<Self>> {
        Py::new(py, (Transform::from_xyz(x, y, z).into(), PyComponent))
    }

    #[staticmethod]
    pub fn from_rotation(py: Python, rotation: &PyQuat) -> PyResult<Py<Self>> {
        let transform = Transform::from_rotation(rotation.try_into()?);
        Py::new(py, (transform.into(), PyComponent))
    }

    #[staticmethod]
    pub fn from_scale(py: Python, scale: &PyVec3) -> PyResult<Py<Self>> {
        let transform = Transform::from_scale(scale.try_into()?);
        Py::new(py, (transform.into(), PyComponent))
    }

    #[staticmethod]
    pub fn from_translation(py: Python, translation: &PyVec3) -> PyResult<Py<Self>> {
        let transform = Transform::from_translation(translation.try_into()?);
        Py::new(py, (transform.into(), PyComponent))
    }

    #[staticmethod]
    pub fn from_matrix(py: Python, world_from_local: &PyMat4) -> PyResult<Py<Self>> {
        let bevy_mat: Mat4 = world_from_local.try_into()?;
        let transform = Transform::from_matrix(bevy_mat);
        Py::new(py, (transform.into(), PyComponent))
    }

    #[staticmethod]
    pub fn from_isometry(py: Python, iso: PyIsometry3d) -> PyResult<Py<Self>> {
        let transform = Transform::from_isometry(iso.try_into()?);
        Py::new(py, (transform.into(), PyComponent))
    }

    #[staticmethod]
    #[pyo3(name = "IDENTITY")]
    pub fn identity(py: Python) -> PyResult<Py<Self>> {
        Py::new(py, (Transform::IDENTITY.into(), PyComponent))
    }

    #[getter]
    pub fn translation(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|t| &t.translation, |t| &mut t.translation)?)
    }

    #[setter]
    pub fn set_translation(&mut self, translation: PyVec3) -> PyResult<()> {
        self.as_mut()?.translation = translation.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn rotation(&self) -> PyResult<PyQuat> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|t| &t.rotation, |t| &mut t.rotation)?)
    }

    #[setter]
    pub fn set_rotation(&mut self, rotation: PyQuat) -> PyResult<()> {
        self.as_mut()?.rotation = rotation.try_into()?;
        Ok(())
    }

    #[getter]
    pub fn scale(&self) -> PyResult<PyVec3> {
        Ok(self
            .storage
            .borrow_resolved_field_as(|t| &t.scale, |t| &mut t.scale)?)
    }

    #[setter]
    pub fn set_scale(&mut self, scale: PyVec3) -> PyResult<()> {
        self.as_mut()?.scale = scale.try_into()?;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let transform = self.as_ref()?;
        Ok(format_transform_repr(
            "Transform",
            transform.translation,
            transform.rotation,
            transform.scale,
        ))
    }

    pub fn rotate_x(&mut self, angle: f32) -> PyResult<()> {
        self.as_mut()?.rotate_x(angle);
        Ok(())
    }

    pub fn rotate_y(&mut self, angle: f32) -> PyResult<()> {
        self.as_mut()?.rotate_y(angle);
        Ok(())
    }

    pub fn rotate_z(&mut self, angle: f32) -> PyResult<()> {
        self.as_mut()?.rotate_z(angle);
        Ok(())
    }

    pub fn rotate(&mut self, rotation: &PyQuat) -> PyResult<()> {
        self.as_mut()?.rotate(rotation.try_into()?);
        Ok(())
    }

    pub fn look_at(&mut self, target: &PyVec3, up: &Bound<'_, PyAny>) -> PyResult<()> {
        if up.is_instance_of::<PyDir3>() {
            let up: Dir3 = up.extract::<PyDir3>()?.try_into()?;
            self.as_mut()?.look_at(target.try_into()?, up);
            Ok(())
        } else if up.is_instance_of::<PyVec3>() {
            let up = up.extract::<PyVec3>()?;
            let up: Dir3 = up.try_into()?;
            self.as_mut()?.look_at(target.try_into()?, up);
            Ok(())
        } else {
            Err(PyTypeError::new_err("up must be a Vec3 or Dir3"))
        }
    }

    pub fn looking_at(
        &self,
        py: Python<'_>,
        target: &PyVec3,
        up: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        let mut transform = Self::from(*self.as_ref()?);
        transform.look_at(target, up)?;
        Py::new(py, (transform, PyComponent))
    }

    pub fn looking_to(
        &self,
        py: Python<'_>,
        direction: &PyVec3,
        up: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        let mut transform = Self::from(*self.as_ref()?);
        transform.look_to(direction, up)?;
        Py::new(py, (transform, PyComponent))
    }

    pub fn aligned_by(
        &self,
        py: Python<'_>,
        main_axis: &Bound<'_, PyAny>,
        main_direction: &Bound<'_, PyAny>,
        secondary_axis: &Bound<'_, PyAny>,
        secondary_direction: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Self>> {
        let mut transform = Self::from(*self.as_ref()?);
        transform.align(
            main_axis,
            main_direction,
            secondary_axis,
            secondary_direction,
        )?;
        Py::new(py, (transform, PyComponent))
    }

    pub fn with_translation(&self, py: Python<'_>, translation: &PyVec3) -> PyResult<Py<Self>> {
        let mut transform = *self.as_ref()?;
        transform.translation = translation.try_into()?;
        Py::new(py, (transform.into(), PyComponent))
    }

    pub fn with_rotation(&self, py: Python<'_>, rotation: &PyQuat) -> PyResult<Py<Self>> {
        let mut transform = *self.as_ref()?;
        transform.rotation = rotation.try_into()?;
        Py::new(py, (transform.into(), PyComponent))
    }

    pub fn with_scale(&self, py: Python<'_>, scale: &PyVec3) -> PyResult<Py<Self>> {
        let mut transform = *self.as_ref()?;
        transform.scale = scale.try_into()?;
        Py::new(py, (transform.into(), PyComponent))
    }

    pub fn align(
        &mut self,
        main_axis: &Bound<'_, PyAny>,
        main_direction: &Bound<'_, PyAny>,
        secondary_axis: &Bound<'_, PyAny>,
        secondary_direction: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let main_axis: Dir3 = if main_axis.is_instance_of::<PyDir3>() {
            main_axis.extract::<PyDir3>()?.try_into()?
        } else {
            main_axis.extract::<PyVec3>()?.try_into()?
        };

        let main_direction: Dir3 = if main_direction.is_instance_of::<PyDir3>() {
            main_direction.extract::<PyDir3>()?.try_into()?
        } else {
            main_direction.extract::<PyVec3>()?.try_into()?
        };

        let secondary_axis: Dir3 = if secondary_axis.is_instance_of::<PyDir3>() {
            secondary_axis.extract::<PyDir3>()?.try_into()?
        } else {
            secondary_axis.extract::<PyVec3>()?.try_into()?
        };
        let secondary_direction: Dir3 = if secondary_direction.is_instance_of::<PyDir3>() {
            secondary_direction.extract::<PyDir3>()?.try_into()?
        } else {
            secondary_direction.extract::<PyVec3>()?.try_into()?
        };

        self.as_mut()?.align(
            main_axis,
            main_direction,
            secondary_axis,
            secondary_direction,
        );

        Ok(())
    }

    pub fn to_matrix(&self) -> PyResult<PyMat4> {
        Ok(PyMat4::from_mat4(self.as_ref()?.to_matrix()))
    }

    pub fn compute_affine(&self) -> PyResult<PyAffine3A> {
        Ok(self.as_ref()?.compute_affine().into())
    }

    pub fn local_x(&self) -> PyResult<PyDir3> {
        Ok(self.as_ref()?.local_x().into())
    }

    pub fn local_y(&self) -> PyResult<PyDir3> {
        Ok(self.as_ref()?.local_y().into())
    }

    pub fn local_z(&self) -> PyResult<PyDir3> {
        Ok(self.as_ref()?.local_z().into())
    }

    pub fn left(&self) -> PyResult<PyDir3> {
        Ok(self.as_ref()?.left().into())
    }

    pub fn right(&self) -> PyResult<PyDir3> {
        Ok(self.as_ref()?.right().into())
    }

    pub fn forward(&self) -> PyResult<PyDir3> {
        Ok(self.as_ref()?.forward().into())
    }

    pub fn back(&self) -> PyResult<PyDir3> {
        Ok(self.as_ref()?.back().into())
    }

    pub fn up(&self) -> PyResult<PyDir3> {
        Ok(self.as_ref()?.up().into())
    }

    pub fn down(&self) -> PyResult<PyDir3> {
        Ok(self.as_ref()?.down().into())
    }

    pub fn rotate_axis(&mut self, axis: &PyDir3, angle: f32) -> PyResult<()> {
        self.as_mut()?.rotate_axis(axis.get()?, angle);
        Ok(())
    }

    pub fn rotate_local(&mut self, rotation: &PyQuat) -> PyResult<()> {
        self.as_mut()?.rotate_local(rotation.try_into()?);
        Ok(())
    }

    pub fn rotate_local_axis(&mut self, axis: &PyDir3, angle: f32) -> PyResult<()> {
        self.as_mut()?.rotate_local_axis(axis.get()?, angle);
        Ok(())
    }

    pub fn rotate_local_x(&mut self, angle: f32) -> PyResult<()> {
        self.as_mut()?.rotate_local_x(angle);
        Ok(())
    }

    pub fn rotate_local_y(&mut self, angle: f32) -> PyResult<()> {
        self.as_mut()?.rotate_local_y(angle);
        Ok(())
    }

    pub fn rotate_local_z(&mut self, angle: f32) -> PyResult<()> {
        self.as_mut()?.rotate_local_z(angle);
        Ok(())
    }

    pub fn translate_around(&mut self, point: &PyVec3, rotation: &PyQuat) -> PyResult<()> {
        self.as_mut()?
            .translate_around(point.try_into()?, rotation.try_into()?);
        Ok(())
    }

    pub fn rotate_around(&mut self, point: &PyVec3, rotation: &PyQuat) -> PyResult<()> {
        self.as_mut()?
            .rotate_around(point.try_into()?, rotation.try_into()?);
        Ok(())
    }

    pub fn look_to(&mut self, direction: &PyVec3, up: &Bound<'_, PyAny>) -> PyResult<()> {
        if up.is_instance_of::<PyDir3>() {
            let up: Dir3 = up.extract::<PyDir3>()?.try_into()?;
            self.as_mut()?.look_to(direction, up);
            Ok(())
        } else if up.is_instance_of::<PyVec3>() {
            let up = up.extract::<PyVec3>()?;
            let up: Dir3 = up.try_into()?;
            self.as_mut()?.look_to(direction, up);
            Ok(())
        } else {
            Err(PyTypeError::new_err("up must be a Vec3 or Dir3"))
        }
    }

    pub fn transform_point(&self, point: &PyVec3) -> PyResult<PyVec3> {
        Ok(self
            .as_ref()?
            .transform_point(point.try_into()?)
            .try_into()?)
    }

    pub fn is_finite(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.is_finite())
    }

    pub fn to_isometry(&self) -> PyResult<PyIsometry3d> {
        Ok(self.as_ref()?.to_isometry().into())
    }

    pub fn mul_transform(
        pyself: PyRef<Self>,
        py: Python,
        transform: &PyTransform,
    ) -> PyResult<Py<PyAny>> {
        let self_transform = PyTransform::as_ref(&pyself)?;
        let other_transform = PyTransform::as_ref(transform)?;
        let new: Transform = self_transform.mul_transform(*other_transform);
        let py_transform: PyTransform = new.into();
        Ok(Py::new(py, (py_transform, PyComponent))?.into_any())
    }
}
