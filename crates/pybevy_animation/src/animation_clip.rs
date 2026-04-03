use bevy::animation::{AnimationClip, VariableCurve};
use pybevy_core::{AssetStorage, PyAsset};
use pybevy_macros::pyasset;
use pyo3::{exceptions::PyTypeError, prelude::*, types::PyDict};

use crate::{animation_curve::PyAnimationCurve, animation_target_id::PyAnimationTargetId};

#[pyclass(name = "VariableCurve", extends = PyAnimationCurve)]
#[derive(Debug, Clone)]
pub struct PyVariableCurve(pub(crate) VariableCurve);

#[pyasset(AnimationClip, bridge)]
#[pyclass(name = "AnimationClip", extends = PyAsset)]
#[derive(Debug)]
pub struct PyAnimationClip {
    pub(crate) storage: AssetStorage<AnimationClip>,
}

#[pymethods]
impl PyAnimationClip {
    #[new]
    pub fn new() -> (Self, PyAsset) {
        (Self::from(AnimationClip::default()), PyAsset)
    }

    pub fn duration(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.duration())
    }

    pub fn set_duration(&mut self, duration_sec: f32) -> PyResult<()> {
        self.as_mut()?.set_duration(duration_sec);
        Ok(())
    }

    pub fn curves(&self, py: Python) -> PyResult<Py<PyDict>> {
        let curves = self.as_ref()?.curves();
        let dict = PyDict::new(py);

        for (target_id, curve_vec) in curves.iter() {
            let py_target_id = Py::new(py, PyAnimationTargetId::from_owned(*target_id))?;
            let py_curves: Vec<Py<PyVariableCurve>> = curve_vec
                .iter()
                .map(|c| Py::new(py, (PyVariableCurve(c.clone()), PyAnimationCurve)))
                .collect::<Result<_, _>>()?;
            dict.set_item(py_target_id, py_curves)?;
        }

        Ok(dict.into())
    }

    pub fn curves_for_target(
        &self,
        py: Python,
        target_id: &PyAnimationTargetId,
    ) -> PyResult<Option<Vec<Py<PyVariableCurve>>>> {
        let curves = self.as_ref()?.curves_for_target(target_id.0);

        match curves {
            Some(curve_vec) => {
                let py_curves: Vec<Py<PyVariableCurve>> = curve_vec
                    .iter()
                    .map(|c| Py::new(py, (PyVariableCurve(c.clone()), PyAnimationCurve)))
                    .collect::<Result<_, _>>()?;
                Ok(Some(py_curves))
            }
            None => Ok(None),
        }
    }

    pub fn add_curve_to_target(
        &mut self,
        target_id: &PyAnimationTargetId,
        curve: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        if let Ok(curve) = curve.extract::<PyVariableCurve>() {
            self.as_mut()?
                .add_variable_curve_to_target(target_id.0, curve.0);
            Ok(())
        } else {
            Err(PyTypeError::new_err("Expected a VariableCurve instance."))
        }
    }

    pub fn add_variable_curve_to_target(
        &mut self,
        target_id: &PyAnimationTargetId,
        variable_curve: &PyVariableCurve,
    ) -> PyResult<()> {
        self.as_mut()?
            .add_variable_curve_to_target(target_id.0, variable_curve.0.clone());
        Ok(())
    }
}
