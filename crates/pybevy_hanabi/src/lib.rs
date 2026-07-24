//! Hanabi GPU particle bindings — `pybevy.contrib.hanabi` (#13).
//!
//! Deliberately minimal surface: two effect presets (`EffectAsset.fountain`,
//! `EffectAsset.burst`), the `ParticleEffect` component, and `HanabiPlugin`.
//! Hanabi's full modifier/expression graph stays Rust-side until the shape of
//! a Python expression API is decided; presets cover the two effects games
//! reach for first, and more presets are additive later.

use bevy::{
    app::App,
    asset::Handle,
    math::{Vec3, Vec4},
};
use bevy_hanabi::prelude::*;
use pybevy_core::{
    AssetStorage, ComponentStorage, PluginBuild, PyComponent, PyPlugin, handle::PyHandle,
};
use pybevy_macros::{pyasset, pycomponent, pyplugin};
use pyo3::prelude::*;

pub mod prelude {
    pub use crate::{PyEffectAsset, PyHanabiPlugin, PyParticleEffect};
}

#[pyplugin(HanabiPlugin)]
#[pyclass(name = "HanabiPlugin", extends = PyPlugin, frozen, skip_from_py_object)]
#[derive(Debug, Clone, Copy)]
pub struct PyHanabiPlugin;

#[pymethods]
impl PyHanabiPlugin {
    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        (PyHanabiPlugin, PyPlugin).into()
    }
}

impl Default for PyHanabiPlugin {
    fn default() -> Self {
        PyHanabiPlugin
    }
}

impl PluginBuild for PyHanabiPlugin {
    fn build(_py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        app.add_plugins(HanabiPlugin);
        Ok(())
    }
}

/// Vec4 color gradient from evenly spaced RGBA stops.
fn color_gradient(colors: &[(f32, f32, f32, f32)]) -> Gradient<Vec4> {
    let stop = |c: &(f32, f32, f32, f32)| Vec4::new(c.0, c.1, c.2, c.3);
    match colors {
        [] => Gradient::constant(Vec4::ONE),
        [c] => Gradient::constant(stop(c)),
        [a, b] => Gradient::linear(stop(a), stop(b)),
        many => {
            let mut g = Gradient::new();
            let last = (many.len() - 1) as f32;
            for (i, c) in many.iter().enumerate() {
                g.add_key(i as f32 / last, stop(c));
            }
            g
        }
    }
}

#[pyasset(EffectAsset, bridge)]
#[pyclass(name = "EffectAsset", extends = pybevy_core::PyAsset, skip_from_py_object)]
pub struct PyEffectAsset {
    pub(crate) storage: AssetStorage<EffectAsset>,
}

/// Shared tail: age/lifetime init, gravity update, color/size render.
#[allow(clippy::too_many_arguments)]
fn finish_effect(
    writer: ExprWriter,
    capacity: u32,
    spawner: SpawnerSettings,
    name: &str,
    lifetime: (f32, f32),
    gravity: f32,
    size: (f32, f32),
    colors: &[(f32, f32, f32, f32)],
    init_pos: SetPositionSphereModifier,
    init_vel: impl Modifier,
) -> EffectAsset {
    let init_age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
    let init_lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer
            .lit(lifetime.0)
            .uniform(writer.lit(lifetime.1))
            .expr(),
    );
    let update_accel = AccelModifier::new(writer.lit(Vec3::Y * gravity).expr());

    EffectAsset::new(capacity, spawner, writer.finish())
        .with_name(name)
        .init(init_pos)
        .init(init_vel)
        .init(init_age)
        .init(init_lifetime)
        .update(update_accel)
        .render(ColorOverLifetimeModifier::new(color_gradient(colors)))
        .render(SizeOverLifetimeModifier {
            gradient: Gradient::linear(Vec3::splat(size.0), Vec3::splat(size.1)),
            screen_space_size: false,
        })
}

#[pymethods]
impl PyEffectAsset {
    /// Continuous upward stream of particles.
    #[staticmethod]
    #[pyo3(signature = (capacity=4096, rate=100.0, lifetime=1.2, speed=(3.0, 5.0),
                        spread=0.25, radius=0.1, gravity=-6.0, size=(0.15, 0.03),
                        colors=vec![(1.0, 0.85, 0.4, 1.0), (1.0, 0.3, 0.05, 0.0)]))]
    #[allow(clippy::too_many_arguments)]
    pub fn fountain(
        py: Python<'_>,
        capacity: u32,
        rate: f32,
        lifetime: f32,
        speed: (f32, f32),
        spread: f32,
        radius: f32,
        gravity: f32,
        size: (f32, f32),
        colors: Vec<(f32, f32, f32, f32)>,
    ) -> PyResult<Py<Self>> {
        let writer = ExprWriter::new();
        let init_pos = SetPositionSphereModifier {
            center: writer.lit(Vec3::ZERO).expr(),
            radius: writer.lit(radius).expr(),
            dimension: ShapeDimension::Volume,
        };
        // mostly-upward velocity with sideways spread
        let side = writer.lit(speed.1 * spread);
        let vx = writer.lit(-1.0).uniform(writer.lit(1.0)) * side.clone();
        let vz = writer.lit(-1.0).uniform(writer.lit(1.0)) * side;
        let vy = writer.lit(speed.0).uniform(writer.lit(speed.1));
        let init_vel = SetAttributeModifier::new(Attribute::VELOCITY, vx.vec3(vy, vz).expr());

        let asset = finish_effect(
            writer,
            capacity,
            SpawnerSettings::rate(rate.into()),
            "pybevy.fountain",
            (lifetime * 0.8, lifetime * 1.2),
            gravity,
            size,
            &colors,
            init_pos,
            init_vel,
        );
        Py::new(py, Self::from_owned(asset))
    }

    /// One-shot radial explosion of particles.
    #[staticmethod]
    #[pyo3(signature = (capacity=2048, count=500.0, lifetime=0.9, speed=(2.0, 8.0),
                        radius=0.2, gravity=-3.0, size=(0.12, 0.02),
                        colors=vec![(1.0, 1.0, 0.6, 1.0), (1.0, 0.4, 0.1, 0.0)]))]
    #[allow(clippy::too_many_arguments)]
    pub fn burst(
        py: Python<'_>,
        capacity: u32,
        count: f32,
        lifetime: f32,
        speed: (f32, f32),
        radius: f32,
        gravity: f32,
        size: (f32, f32),
        colors: Vec<(f32, f32, f32, f32)>,
    ) -> PyResult<Py<Self>> {
        let writer = ExprWriter::new();
        let init_pos = SetPositionSphereModifier {
            center: writer.lit(Vec3::ZERO).expr(),
            radius: writer.lit(radius).expr(),
            dimension: ShapeDimension::Volume,
        };
        let init_vel = SetVelocitySphereModifier {
            center: writer.lit(Vec3::ZERO).expr(),
            speed: writer.lit(speed.0).uniform(writer.lit(speed.1)).expr(),
        };

        let asset = finish_effect(
            writer,
            capacity,
            SpawnerSettings::once(count.into()),
            "pybevy.burst",
            (lifetime * 0.7, lifetime * 1.3),
            gravity,
            size,
            &colors,
            init_pos,
            init_vel,
        );
        Py::new(py, Self::from_owned(asset))
    }

    #[getter]
    pub fn name(&self) -> PyResult<String> {
        Ok(self.as_ref()?.name.clone())
    }

    #[getter]
    pub fn capacity(&self) -> PyResult<u32> {
        Ok(self.as_ref()?.capacity())
    }

    fn __repr__(&self) -> String {
        match self.storage.as_ref() {
            Ok(asset) => format!(
                "EffectAsset(name={:?}, capacity={})",
                asset.name,
                asset.capacity()
            ),
            Err(_) => "EffectAsset(<invalid>)".to_string(),
        }
    }
}

#[pycomponent(ParticleEffect, bridge)]
#[pyclass(name = "ParticleEffect", extends = PyComponent)]
pub struct PyParticleEffect {
    pub(crate) storage: ComponentStorage<ParticleEffect>,
}

#[pymethods]
impl PyParticleEffect {
    #[new]
    pub fn new(effect: PyHandle) -> PyResult<PyClassInitializer<Self>> {
        let handle = Handle::<EffectAsset>::try_from(&effect)?;
        Ok(Self::from_owned(ParticleEffect::new(handle)).into())
    }

    #[getter]
    pub fn effect(&self) -> PyResult<PyHandle> {
        Ok(PyHandle::from(&self.as_ref()?.handle))
    }

    #[setter]
    pub fn set_effect(&mut self, handle: PyHandle) -> PyResult<()> {
        self.as_mut()?.handle = Handle::<EffectAsset>::try_from(&handle)?;
        Ok(())
    }

    fn __repr__(&self) -> String {
        "ParticleEffect(...)".to_string()
    }
}

pub fn add_module(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let m = PyModule::new(parent.py(), "hanabi")?;
    m.add_class::<PyHanabiPlugin>()?;
    m.add_class::<PyEffectAsset>()?;
    m.add_class::<PyParticleEffect>()?;
    parent.add_submodule(&m)
}
