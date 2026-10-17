use bevy::input_focus::tab_navigation::{TabGroup, TabIndex};
use pybevy_core::{ComponentStorage, PyComponent};
use pybevy_macros::pycomponent;
use pyo3::prelude::*;

#[pycomponent(TabIndex, bridge, view_fields = [0 as value])]
#[pyclass(name = "TabIndex", module = "pybevy.input_focus", extends = PyComponent, skip_from_py_object)]
#[derive(Debug)]
pub struct PyTabIndex {
    pub(crate) storage: ComponentStorage<TabIndex>,
}

#[pymethods]
impl PyTabIndex {
    #[new]
    #[pyo3(signature = (value = 0))]
    pub fn new(value: i32) -> PyClassInitializer<Self> {
        Self::from_owned(TabIndex(value)).into()
    }

    #[getter]
    pub fn value(&self) -> PyResult<i32> {
        Ok(self.as_ref()?.0)
    }

    #[setter]
    pub fn set_value(&mut self, value: i32) -> PyResult<()> {
        self.as_mut()?.0 = value;
        Ok(())
    }

    pub fn __eq__(&self, other: &Self) -> PyResult<bool> {
        Ok(self.as_ref()? == other.as_ref()?)
    }

    pub fn __repr__(&self) -> PyResult<String> {
        Ok(format!("TabIndex({})", self.as_ref()?.0))
    }
}

#[pycomponent(TabGroup, bridge, view_fields = [order, modal])]
#[pyclass(name = "TabGroup", module = "pybevy.input_focus", extends = PyComponent, skip_from_py_object)]
#[derive(Debug)]
pub struct PyTabGroup {
    pub(crate) storage: ComponentStorage<TabGroup>,
}

#[pymethods]
impl PyTabGroup {
    #[new]
    #[pyo3(signature = (order = 0, *, modal = false))]
    pub fn new(order: i32, modal: bool) -> PyClassInitializer<Self> {
        Self::from_owned(TabGroup { order, modal }).into()
    }

    #[getter]
    pub fn order(&self) -> PyResult<i32> {
        Ok(self.as_ref()?.order)
    }

    #[setter]
    pub fn set_order(&mut self, order: i32) -> PyResult<()> {
        self.as_mut()?.order = order;
        Ok(())
    }

    #[getter]
    pub fn modal(&self) -> PyResult<bool> {
        Ok(self.as_ref()?.modal)
    }

    #[setter]
    pub fn set_modal(&mut self, modal: bool) -> PyResult<()> {
        self.as_mut()?.modal = modal;
        Ok(())
    }

    pub fn __repr__(&self) -> PyResult<String> {
        let group = self.as_ref()?;
        let modal = if group.modal { "True" } else { "False" };
        Ok(format!("TabGroup(order={}, modal={modal})", group.order))
    }
}
