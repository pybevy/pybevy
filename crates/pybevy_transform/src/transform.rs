use bevy::{
    math::{Dir3, Mat4, Quat, Vec3},
    transform::components::Transform,
};
use pybevy_core::{
    ComponentStorage, PyComponent,
    domain::{FINITE, check_for},
    public_error::{TRANSFORM_ROTATION_ZERO_LENGTH, direction_argument_type},
    repr::ReprLiteral,
};
use pybevy_macros::pycomponent;
use pybevy_math::{
    affine3a::PyAffine3A, dir3::PyDir3, isometry3d::PyIsometry3d, mat4::PyMat4, quat::PyQuat,
    vec3::PyVec3,
};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
};

#[pycomponent(
    Transform,
    bridge,
    domains = [
        translation => [finite],
        rotation => [finite],
        scale => [finite],
    ],
    view_fields = [translation, rotation, scale]
)]
#[pyclass(name = "Transform", module = "pybevy.transform", extends = PyComponent, eq)]
#[derive(Debug)]
pub struct PyTransform {
    pub(crate) storage: ComponentStorage<Transform>,
}

impl PyTransform {
    fn checked(transform: Transform) -> PyResult<(Self, PyComponent)> {
        Self::validate_domains(&transform)?;
        nonzero_rotation(transform.rotation)?;
        Ok((transform.into(), PyComponent))
    }

    fn checked_rotation(rotation: Quat) -> PyResult<Quat> {
        nonzero_rotation(Self::check_rotation(rotation)?)
    }
}

// A zero quaternion is no rotation at all; MCP writes reject it the same way.
fn nonzero_rotation(rotation: Quat) -> PyResult<Quat> {
    if rotation.length_squared() == 0.0 {
        return Err(PyValueError::new_err(TRANSFORM_ROTATION_ZERO_LENGTH));
    }
    Ok(rotation)
}

impl PartialEq for PyTransform {
    fn eq(&self, other: &Self) -> bool {
        match (self.as_ref(), other.as_ref()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
    }
}

// Per-entity hot paths: a finite argument skips the generic domain check.
fn finite_angle(angle: f32) -> PyResult<f32> {
    if angle.is_finite() {
        return Ok(angle);
    }
    Ok(check_for::<PyTransform, _>("angle", angle, FINITE)?)
}

fn finite_vec3(field: &'static str, value: Vec3) -> PyResult<Vec3> {
    if value.is_finite() {
        return Ok(value);
    }
    Ok(check_for::<PyTransform, _>(field, value, FINITE)?)
}

fn finite_rotation(rotation: Quat) -> PyResult<Quat> {
    if rotation.is_finite() {
        return nonzero_rotation(rotation);
    }
    Ok(PyTransform::check_rotation(rotation)?)
}

// Mirror Bevy: a failed TryInto<Dir3> falls back to the default axis instead of raising.
fn extract_dir3_with_fallback(
    obj: &Bound<'_, PyAny>,
    fallback: Dir3,
    param: &str,
) -> PyResult<Dir3> {
    if obj.is_instance_of::<PyDir3>() {
        return obj.extract::<PyDir3>()?.try_into();
    }
    if let Ok(vec) = obj.extract::<PyVec3>() {
        return Ok(vec.try_into().unwrap_or(fallback));
    }
    Err(PyTypeError::new_err(direction_argument_type(param)))
}

pub(crate) fn format_transform_repr(
    name: &str,
    translation: Vec3,
    rotation: Quat,
    scale: Vec3,
) -> String {
    format!(
        "{name}(translation={}, rotation={}, scale={})",
        translation.repr_literal(),
        rotation.repr_literal(),
        scale.repr_literal(),
    )
}

#[pymethods]
impl PyTransform {
    #[new]
    #[pyo3(signature = (*, translation = PyVec3::ZERO, rotation = PyQuat::IDENTITY, scale = PyVec3::ONE))]
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

        Ok(Self::checked(transform)?.into())
    }

    #[staticmethod]
    pub fn from_xyz(py: Python, x: f32, y: f32, z: f32) -> PyResult<Py<Self>> {
        Py::new(py, Self::checked(Transform::from_xyz(x, y, z))?)
    }

    #[staticmethod]
    pub fn from_rotation(py: Python, rotation: &PyQuat) -> PyResult<Py<Self>> {
        let transform = Transform::from_rotation(rotation.try_into()?);
        Py::new(py, Self::checked(transform)?)
    }

    #[staticmethod]
    pub fn from_scale(py: Python, scale: &PyVec3) -> PyResult<Py<Self>> {
        let transform = Transform::from_scale(scale.try_into()?);
        Py::new(py, Self::checked(transform)?)
    }

    #[staticmethod]
    pub fn from_translation(py: Python, translation: &PyVec3) -> PyResult<Py<Self>> {
        let transform = Transform::from_translation(translation.try_into()?);
        Py::new(py, Self::checked(transform)?)
    }

    #[staticmethod]
    pub fn from_matrix(py: Python, world_from_local: &PyMat4) -> PyResult<Py<Self>> {
        let bevy_mat: Mat4 = world_from_local.try_into()?;
        let transform = Transform::from_matrix(bevy_mat);
        Py::new(py, Self::checked(transform)?)
    }

    #[staticmethod]
    pub fn from_isometry(py: Python, iso: PyIsometry3d) -> PyResult<Py<Self>> {
        let transform = Transform::from_isometry(iso.try_into()?);
        Py::new(py, Self::checked(transform)?)
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
        let translation = Self::check_translation(Vec3::try_from(translation)?)?;
        self.as_mut()?.translation = translation;
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
        let rotation = Self::checked_rotation(Quat::try_from(rotation)?)?;
        self.as_mut()?.rotation = rotation;
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
        let scale = Self::check_scale(Vec3::try_from(scale)?)?;
        self.as_mut()?.scale = scale;
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
        let angle = finite_angle(angle)?;
        self.as_mut()?.rotate_x(angle);
        Ok(())
    }

    pub fn rotate_y(&mut self, angle: f32) -> PyResult<()> {
        let angle = finite_angle(angle)?;
        self.as_mut()?.rotate_y(angle);
        Ok(())
    }

    pub fn rotate_z(&mut self, angle: f32) -> PyResult<()> {
        let angle = finite_angle(angle)?;
        self.as_mut()?.rotate_z(angle);
        Ok(())
    }

    pub fn rotate(&mut self, rotation: &PyQuat) -> PyResult<()> {
        let rotation = finite_rotation(Quat::try_from(rotation)?)?;
        self.as_mut()?.rotate(rotation);
        Ok(())
    }

    pub fn look_at(&mut self, target: &PyVec3, up: &Bound<'_, PyAny>) -> PyResult<()> {
        let target = finite_vec3("target", Vec3::try_from(target)?)?;
        let up = extract_dir3_with_fallback(up, Dir3::Y, "up")?;
        self.as_mut()?.look_at(target, up);
        Ok(())
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
        direction: &Bound<'_, PyAny>,
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
        transform.translation = Self::check_translation(Vec3::try_from(translation)?)?;
        Py::new(py, (transform.into(), PyComponent))
    }

    pub fn with_rotation(&self, py: Python<'_>, rotation: &PyQuat) -> PyResult<Py<Self>> {
        let mut transform = *self.as_ref()?;
        transform.rotation = Self::checked_rotation(Quat::try_from(rotation)?)?;
        Py::new(py, (transform.into(), PyComponent))
    }

    pub fn with_scale(&self, py: Python<'_>, scale: &PyVec3) -> PyResult<Py<Self>> {
        let mut transform = *self.as_ref()?;
        transform.scale = Self::check_scale(Vec3::try_from(scale)?)?;
        Py::new(py, (transform.into(), PyComponent))
    }

    pub fn align(
        &mut self,
        main_axis: &Bound<'_, PyAny>,
        main_direction: &Bound<'_, PyAny>,
        secondary_axis: &Bound<'_, PyAny>,
        secondary_direction: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let main_axis = extract_dir3_with_fallback(main_axis, Dir3::X, "main_axis")?;
        let main_direction = extract_dir3_with_fallback(main_direction, Dir3::X, "main_direction")?;
        let secondary_axis = extract_dir3_with_fallback(secondary_axis, Dir3::Y, "secondary_axis")?;
        let secondary_direction =
            extract_dir3_with_fallback(secondary_direction, Dir3::Y, "secondary_direction")?;

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
        let axis = axis.get()?;
        let angle = finite_angle(angle)?;
        self.as_mut()?.rotate_axis(axis, angle);
        Ok(())
    }

    pub fn rotate_local(&mut self, rotation: &PyQuat) -> PyResult<()> {
        let rotation = finite_rotation(Quat::try_from(rotation)?)?;
        self.as_mut()?.rotate_local(rotation);
        Ok(())
    }

    pub fn rotate_local_axis(&mut self, axis: &PyDir3, angle: f32) -> PyResult<()> {
        let axis = axis.get()?;
        let angle = finite_angle(angle)?;
        self.as_mut()?.rotate_local_axis(axis, angle);
        Ok(())
    }

    pub fn rotate_local_x(&mut self, angle: f32) -> PyResult<()> {
        let angle = finite_angle(angle)?;
        self.as_mut()?.rotate_local_x(angle);
        Ok(())
    }

    pub fn rotate_local_y(&mut self, angle: f32) -> PyResult<()> {
        let angle = finite_angle(angle)?;
        self.as_mut()?.rotate_local_y(angle);
        Ok(())
    }

    pub fn rotate_local_z(&mut self, angle: f32) -> PyResult<()> {
        let angle = finite_angle(angle)?;
        self.as_mut()?.rotate_local_z(angle);
        Ok(())
    }

    pub fn translate_around(&mut self, point: &PyVec3, rotation: &PyQuat) -> PyResult<()> {
        let point = finite_vec3("point", Vec3::try_from(point)?)?;
        let rotation = finite_rotation(Quat::try_from(rotation)?)?;
        self.as_mut()?.translate_around(point, rotation);
        Ok(())
    }

    pub fn rotate_around(&mut self, point: &PyVec3, rotation: &PyQuat) -> PyResult<()> {
        let point = finite_vec3("point", Vec3::try_from(point)?)?;
        let rotation = finite_rotation(Quat::try_from(rotation)?)?;
        self.as_mut()?.rotate_around(point, rotation);
        Ok(())
    }

    pub fn look_to(&mut self, direction: &Bound<'_, PyAny>, up: &Bound<'_, PyAny>) -> PyResult<()> {
        let direction = extract_dir3_with_fallback(direction, Dir3::NEG_Z, "direction")?;
        let up = extract_dir3_with_fallback(up, Dir3::Y, "up")?;
        self.as_mut()?.look_to(direction, up);
        Ok(())
    }

    pub fn transform_point(&self, point: &PyVec3) -> PyResult<PyVec3> {
        Ok(self.as_ref()?.transform_point(point.try_into()?).into())
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
