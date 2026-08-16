use accesskit::{Node, Rect};
use bevy::a11y::AccessibilityNode;
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

use super::role::PyRole;

#[pycomponent(AccessibilityNode, bridge, no_reflect)]
#[pyclass(name = "AccessibilityNode", extends = PyComponent)]
pub struct PyAccessibilityNode {
    pub(crate) storage: ComponentStorage<AccessibilityNode>,
}

#[pymethods]
impl PyAccessibilityNode {
    #[new]
    #[pyo3(signature = (role = PyRole::Unknown))]
    pub fn new(role: PyRole) -> PyClassInitializer<Self> {
        let node = Node::new(role.into());
        Self::from_owned(AccessibilityNode::from(node)).into()
    }

    #[getter]
    pub fn role(&self) -> PyResult<PyRole> {
        Ok(self.as_ref()?.0.role().into())
    }

    pub fn set_role(&mut self, role: PyRole) -> PyResult<()> {
        self.as_mut()?.0.set_role(role.into());
        Ok(())
    }

    #[getter]
    pub fn label(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.0.label().map(|s| s.to_string()))
    }

    pub fn set_label(&mut self, label: &str) -> PyResult<()> {
        self.as_mut()?.0.set_label(label);
        Ok(())
    }

    pub fn clear_label(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear_label();
        Ok(())
    }

    #[getter]
    pub fn value(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.0.value().map(|s| s.to_string()))
    }

    pub fn set_value(&mut self, value: &str) -> PyResult<()> {
        self.as_mut()?.0.set_value(value);
        Ok(())
    }

    pub fn clear_value(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear_value();
        Ok(())
    }

    #[getter]
    pub fn description(&self) -> PyResult<Option<String>> {
        Ok(self.as_ref()?.0.description().map(|s| s.to_string()))
    }

    pub fn set_description(&mut self, description: &str) -> PyResult<()> {
        self.as_mut()?.0.set_description(description);
        Ok(())
    }

    pub fn clear_description(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear_description();
        Ok(())
    }

    #[getter]
    pub fn is_disabled(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.is_disabled())
    }

    pub fn set_disabled(&mut self, disabled: bool) -> PyResult<()> {
        if disabled {
            self.as_mut()?.0.set_disabled();
        } else {
            self.as_mut()?.0.clear_disabled();
        }
        Ok(())
    }

    #[getter]
    pub fn is_hidden(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.0.is_hidden())
    }

    pub fn set_hidden(&mut self, hidden: bool) -> PyResult<()> {
        if hidden {
            self.as_mut()?.0.set_hidden();
        } else {
            self.as_mut()?.0.clear_hidden();
        }
        Ok(())
    }

    #[getter]
    pub fn is_expanded(&self) -> PyResult<Option<bool>> {
        Ok(self.as_ref()?.0.is_expanded())
    }

    pub fn set_expanded(&mut self, expanded: bool) -> PyResult<()> {
        self.as_mut()?.0.set_expanded(expanded);
        Ok(())
    }

    pub fn clear_expanded(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear_expanded();
        Ok(())
    }

    #[getter]
    pub fn is_selected(&self) -> PyResult<Option<bool>> {
        Ok(self.as_ref()?.0.is_selected())
    }

    pub fn set_selected(&mut self, selected: bool) -> PyResult<()> {
        self.as_mut()?.0.set_selected(selected);
        Ok(())
    }

    pub fn clear_selected(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear_selected();
        Ok(())
    }

    #[getter]
    pub fn numeric_value(&self) -> PyResult<Option<f64>> {
        Ok(self.as_ref()?.0.numeric_value())
    }

    pub fn set_numeric_value(&mut self, value: f64) -> PyResult<()> {
        self.as_mut()?.0.set_numeric_value(value);
        Ok(())
    }

    pub fn clear_numeric_value(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear_numeric_value();
        Ok(())
    }

    #[getter]
    pub fn min_numeric_value(&self) -> PyResult<Option<f64>> {
        Ok(self.as_ref()?.0.min_numeric_value())
    }

    pub fn set_min_numeric_value(&mut self, value: f64) -> PyResult<()> {
        self.as_mut()?.0.set_min_numeric_value(value);
        Ok(())
    }

    pub fn clear_min_numeric_value(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear_min_numeric_value();
        Ok(())
    }

    #[getter]
    pub fn max_numeric_value(&self) -> PyResult<Option<f64>> {
        Ok(self.as_ref()?.0.max_numeric_value())
    }

    pub fn set_max_numeric_value(&mut self, value: f64) -> PyResult<()> {
        self.as_mut()?.0.set_max_numeric_value(value);
        Ok(())
    }

    pub fn clear_max_numeric_value(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear_max_numeric_value();
        Ok(())
    }

    pub fn set_bounds(&mut self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> PyResult<()> {
        let rect = Rect::new(min_x, min_y, max_x, max_y);
        self.as_mut()?.0.set_bounds(rect);
        Ok(())
    }

    pub fn clear_bounds(&mut self) -> PyResult<()> {
        self.as_mut()?.0.clear_bounds();
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("AccessibilityNode(role={:?})", self.role()?))
    }
}
