use bevy::{
    color::Color,
    gizmos::{config::GizmoConfig, prelude::gizmo},
    math::{Isometry2d, Isometry3d, Vec2, Vec3},
};
use pybevy_color::color::PyColor;
use pybevy_core::{FieldStorage, ValidityFlag};
use pybevy_math::{
    bounding::{PyAabb3d, PyIsometry2d, PyIsometry3d},
    vec2::PyVec2,
    vec3::PyVec3,
};
use pyo3::prelude::*;

use crate::config::PyGizmoConfig;

#[pyclass(name = "Gizmos", frozen, skip_from_py_object)]
#[derive(Debug, Clone)]
pub struct PyGizmos {
    config: FieldStorage<GizmoConfig>,
    validity: ValidityFlag,
}

impl PyGizmos {
    pub fn new(config: FieldStorage<GizmoConfig>, validity: ValidityFlag) -> Self {
        Self { config, validity }
    }

    fn check_valid(&self) -> PyResult<()> {
        self.validity.check().map_err(Into::into)
    }
}

fn vec2(value: &PyVec2) -> PyResult<Vec2> {
    value.try_into()
}

fn vec3(value: &PyVec3) -> PyResult<Vec3> {
    value.try_into()
}

fn native_color(value: PyColor) -> PyResult<Color> {
    value.try_into()
}

fn isometry2(value: PyIsometry2d) -> Isometry2d {
    value.into()
}

fn isometry3(value: PyIsometry3d) -> PyResult<Isometry3d> {
    value.try_into()
}

fn vec2s(values: Vec<PyVec2>) -> PyResult<Vec<Vec2>> {
    values.iter().map(vec2).collect()
}

fn vec3s(values: Vec<PyVec3>) -> PyResult<Vec<Vec3>> {
    values.iter().map(vec3).collect()
}

fn vec2_color_pairs(values: Vec<(PyVec2, PyColor)>) -> PyResult<Vec<(Vec2, Color)>> {
    values
        .into_iter()
        .map(|(position, color)| Ok((vec2(&position)?, native_color(color)?)))
        .collect()
}

fn vec3_color_pairs(values: Vec<(PyVec3, PyColor)>) -> PyResult<Vec<(Vec3, Color)>> {
    values
        .into_iter()
        .map(|(position, color)| Ok((vec3(&position)?, native_color(color)?)))
        .collect()
}

#[pymethods]
impl PyGizmos {
    #[getter]
    pub fn config(&self, py: Python<'_>) -> PyResult<Py<PyGizmoConfig>> {
        self.check_valid()?;
        Py::new(py, PyGizmoConfig::from_borrowed(self.config.clone()))
    }

    pub fn line(&self, start: &PyVec3, end: &PyVec3, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().line(vec3(start)?, vec3(end)?, native_color(color)?);
        Ok(())
    }

    pub fn line_gradient(
        &self,
        start: &PyVec3,
        end: &PyVec3,
        start_color: PyColor,
        end_color: PyColor,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().line_gradient(
            vec3(start)?,
            vec3(end)?,
            native_color(start_color)?,
            native_color(end_color)?,
        );
        Ok(())
    }

    pub fn ray(&self, start: &PyVec3, vector: &PyVec3, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().ray(vec3(start)?, vec3(vector)?, native_color(color)?);
        Ok(())
    }

    pub fn ray_gradient(
        &self,
        start: &PyVec3,
        vector: &PyVec3,
        start_color: PyColor,
        end_color: PyColor,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().ray_gradient(
            vec3(start)?,
            vec3(vector)?,
            native_color(start_color)?,
            native_color(end_color)?,
        );
        Ok(())
    }

    pub fn linestrip(&self, positions: Vec<PyVec3>, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().linestrip(vec3s(positions)?, native_color(color)?);
        Ok(())
    }

    pub fn linestrip_gradient(&self, points: Vec<(PyVec3, PyColor)>) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().linestrip_gradient(vec3_color_pairs(points)?);
        Ok(())
    }

    pub fn lineloop(&self, positions: Vec<PyVec3>, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().lineloop(vec3s(positions)?, native_color(color)?);
        Ok(())
    }

    pub fn line_2d(&self, start: &PyVec2, end: &PyVec2, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().line_2d(vec2(start)?, vec2(end)?, native_color(color)?);
        Ok(())
    }

    pub fn line_gradient_2d(
        &self,
        start: &PyVec2,
        end: &PyVec2,
        start_color: PyColor,
        end_color: PyColor,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().line_gradient_2d(
            vec2(start)?,
            vec2(end)?,
            native_color(start_color)?,
            native_color(end_color)?,
        );
        Ok(())
    }

    pub fn ray_2d(&self, start: &PyVec2, vector: &PyVec2, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().ray_2d(vec2(start)?, vec2(vector)?, native_color(color)?);
        Ok(())
    }

    pub fn ray_gradient_2d(
        &self,
        start: &PyVec2,
        vector: &PyVec2,
        start_color: PyColor,
        end_color: PyColor,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().ray_gradient_2d(
            vec2(start)?,
            vec2(vector)?,
            native_color(start_color)?,
            native_color(end_color)?,
        );
        Ok(())
    }

    pub fn linestrip_2d(&self, positions: Vec<PyVec2>, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().linestrip_2d(vec2s(positions)?, native_color(color)?);
        Ok(())
    }

    pub fn linestrip_gradient_2d(&self, points: Vec<(PyVec2, PyColor)>) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().linestrip_gradient_2d(vec2_color_pairs(points)?);
        Ok(())
    }

    pub fn lineloop_2d(&self, positions: Vec<PyVec2>, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().lineloop_2d(vec2s(positions)?, native_color(color)?);
        Ok(())
    }

    pub fn rect(&self, isometry: PyIsometry3d, size: &PyVec2, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().rect(isometry3(isometry)?, vec2(size)?, native_color(color)?);
        Ok(())
    }

    pub fn cube(&self, transform: PyIsometry3d, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().cube(isometry3(transform)?, native_color(color)?);
        Ok(())
    }

    pub fn aabb_3d(
        &self,
        aabb: &PyAabb3d,
        transform: PyIsometry3d,
        color: PyColor,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().aabb_3d(
            aabb.try_to_bevy()?,
            isometry3(transform)?,
            native_color(color)?,
        );
        Ok(())
    }

    pub fn rect_2d(&self, isometry: PyIsometry2d, size: &PyVec2, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().rect_2d(isometry2(isometry), vec2(size)?, native_color(color)?);
        Ok(())
    }

    pub fn cross(&self, isometry: PyIsometry3d, half_size: f32, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().cross(isometry3(isometry)?, half_size, native_color(color)?);
        Ok(())
    }

    pub fn cross_2d(&self, isometry: PyIsometry2d, half_size: f32, color: PyColor) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo().cross_2d(isometry2(isometry), half_size, native_color(color)?);
        Ok(())
    }

    #[pyo3(signature = (isometry, half_size, color, resolution = 32))]
    pub fn ellipse(
        &self,
        isometry: PyIsometry3d,
        half_size: &PyVec2,
        color: PyColor,
        resolution: u32,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo()
            .ellipse(isometry3(isometry)?, vec2(half_size)?, native_color(color)?)
            .resolution(resolution);
        Ok(())
    }

    #[pyo3(signature = (isometry, half_size, color, resolution = 32))]
    pub fn ellipse_2d(
        &self,
        isometry: PyIsometry2d,
        half_size: &PyVec2,
        color: PyColor,
        resolution: u32,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo()
            .ellipse_2d(isometry2(isometry), vec2(half_size)?, native_color(color)?)
            .resolution(resolution);
        Ok(())
    }

    #[pyo3(signature = (isometry, radius, color, resolution = 32))]
    pub fn circle(
        &self,
        isometry: PyIsometry3d,
        radius: f32,
        color: PyColor,
        resolution: u32,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo()
            .circle(isometry3(isometry)?, radius, native_color(color)?)
            .resolution(resolution);
        Ok(())
    }

    #[pyo3(signature = (isometry, radius, color, resolution = 32))]
    pub fn circle_2d(
        &self,
        isometry: PyIsometry2d,
        radius: f32,
        color: PyColor,
        resolution: u32,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo()
            .circle_2d(isometry2(isometry), radius, native_color(color)?)
            .resolution(resolution);
        Ok(())
    }

    #[pyo3(signature = (isometry, radius, color, resolution = 32))]
    pub fn sphere(
        &self,
        isometry: PyIsometry3d,
        radius: f32,
        color: PyColor,
        resolution: u32,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        gizmo()
            .sphere(isometry3(isometry)?, radius, native_color(color)?)
            .resolution(resolution);
        Ok(())
    }

    #[pyo3(signature = (start, end, color, *, tip_length = None, double_ended = false))]
    pub fn arrow(
        &self,
        start: &PyVec3,
        end: &PyVec3,
        color: PyColor,
        tip_length: Option<f32>,
        double_ended: bool,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        let mut buffer = gizmo();
        let mut arrow = buffer.arrow(vec3(start)?, vec3(end)?, native_color(color)?);
        if let Some(tip_length) = tip_length {
            arrow = arrow.with_tip_length(tip_length);
        }
        if double_ended {
            arrow = arrow.with_double_end();
        }
        drop(arrow);
        Ok(())
    }

    #[pyo3(signature = (start, end, color, *, tip_length = None, double_ended = false))]
    pub fn arrow_2d(
        &self,
        start: &PyVec2,
        end: &PyVec2,
        color: PyColor,
        tip_length: Option<f32>,
        double_ended: bool,
    ) -> PyResult<()> {
        self.check_valid()?;
        if !self.config.as_ref()?.enabled {
            return Ok(());
        }
        let mut buffer = gizmo();
        let mut arrow = buffer.arrow_2d(vec2(start)?, vec2(end)?, native_color(color)?);
        if let Some(tip_length) = tip_length {
            arrow = arrow.with_tip_length(tip_length);
        }
        if double_ended {
            arrow = arrow.with_double_end();
        }
        drop(arrow);
        Ok(())
    }
}
