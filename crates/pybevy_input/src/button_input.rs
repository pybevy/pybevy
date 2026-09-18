use bevy::input::{ButtonInput, keyboard::KeyCode};
use pybevy_core::{PyResource, ResourceStorage, registry::global_registry, resource_initializer};
use pybevy_macros::pyresource;
use pyo3::{
    PyTypeInfo,
    exceptions::{PyRuntimeError, PyTypeError},
    prelude::*,
    types::{PyDict, PyTuple, PyType},
};

use crate::{
    key_code::{PyKeyCode, materialize_key_code},
    mouse_button::PyMouseButton,
    mouse_input::PyMouseInput,
};

/// Python name of the class `ButtonInput[KeyCode]` resolves to.
const KEY_CODE_SPECIALIZATION_NAME: &str = "ButtonInput[KeyCode]";

/// Attribute on `ButtonInput` holding that class.
const KEY_CODE_SPECIALIZATION_ATTR: &str = "__pybevy_button_input_key_code__";

#[pyresource(ButtonInput<KeyCode>, no_clone, bridge, "ButtonInput", default_insert)]
#[pyclass(name = "ButtonInput", module = "pybevy.input", extends = PyResource, subclass)]
pub struct PyButtonInput {
    pub(crate) storage: ResourceStorage<ButtonInput<KeyCode>>,
}

/// Build the distinct `ButtonInput[KeyCode]` class and point it at the keyboard bridge.
pub fn register_button_input_specializations(py: Python<'_>) -> PyResult<()> {
    let base = PyButtonInput::type_object(py);
    let namespace = PyDict::new(py);
    namespace.set_item("__module__", "pybevy.input")?;
    namespace.set_item("__qualname__", KEY_CODE_SPECIALIZATION_NAME)?;
    let bases = PyTuple::new(py, [&base])?;
    let specialization = py
        .get_type::<PyType>()
        .call1((KEY_CODE_SPECIALIZATION_NAME, bases, namespace))?
        .cast_into::<PyType>()?;

    if !global_registry::register_resource_bridge_alias(
        specialization.as_type_ptr(),
        base.as_type_ptr(),
    ) {
        return Err(PyRuntimeError::new_err(
            "ButtonInput[KeyCode] cannot be registered before the ButtonInput resource bridge",
        ));
    }

    base.setattr(KEY_CODE_SPECIALIZATION_ATTR, &specialization)?;
    Ok(())
}

#[pymethods]
impl PyButtonInput {
    /// Resolve `ButtonInput[T]` to the resource holding that button type.
    ///
    /// Bevy has one generic `ButtonInput<T>`; PyO3 has no generic classes, so
    /// each specialization is its own registered resource and the subscript
    /// selects between them. An unsupported key is rejected rather than
    /// quietly resolving to the keyboard resource.
    #[classmethod]
    pub fn __class_getitem__(
        cls: &Bound<'_, PyType>,
        key: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyType>> {
        let py = cls.py();
        let key_type = key.cast::<PyType>().map_err(|_| {
            PyTypeError::new_err(
                "ButtonInput[...] requires a button type, for example ButtonInput[KeyCode]",
            )
        })?;

        if key_type.is(PyKeyCode::type_object(py)) {
            return Ok(PyButtonInput::type_object(py)
                .getattr(KEY_CODE_SPECIALIZATION_ATTR)?
                .cast_into::<PyType>()?
                .unbind());
        }
        if key_type.is(PyMouseButton::type_object(py)) {
            return Ok(PyMouseInput::type_object(py).unbind());
        }

        let name = key_type
            .name()
            .map_or_else(|_| "the given type".to_string(), |name| name.to_string());
        Err(PyTypeError::new_err(format!(
            "ButtonInput[{name}] is not available; supported button types are KeyCode and \
             MouseButton. Bevy also registers ButtonInput<Key> for logical keys, which PyBevy \
             does not wrap yet."
        )))
    }

    #[new]
    pub fn new() -> PyClassInitializer<Self> {
        resource_initializer(Self {
            storage: ResourceStorage::owned(ButtonInput::default()),
        })
    }

    pub fn just_pressed(&self, input: PyKeyCode) -> PyResult<bool> {
        Ok(self.as_ref()?.just_pressed(input.to_bevy()))
    }

    pub fn just_released(&self, input: PyKeyCode) -> PyResult<bool> {
        Ok(self.as_ref()?.just_released(input.to_bevy()))
    }

    pub fn pressed(&self, input: PyKeyCode) -> PyResult<bool> {
        Ok(self.as_ref()?.pressed(input.to_bevy()))
    }

    pub fn any_just_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.any_just_pressed(bevy_keys))
    }

    pub fn any_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.any_pressed(bevy_keys))
    }

    pub fn all_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let button_input = self.as_ref()?;
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(bevy_keys.iter().all(|k| button_input.pressed(*k)))
    }

    pub fn get_just_pressed(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let input = self.as_ref()?;
        input
            .get_just_pressed()
            .map(|key| materialize_key_code(py, *key))
            .collect()
    }

    pub fn get_pressed(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let input = self.as_ref()?;
        input
            .get_pressed()
            .map(|key| materialize_key_code(py, *key))
            .collect()
    }

    pub fn get_just_released(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        let input = self.as_ref()?;
        input
            .get_just_released()
            .map(|key| materialize_key_code(py, *key))
            .collect()
    }

    pub fn any_just_released(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.any_just_released(bevy_keys))
    }

    pub fn all_just_pressed(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.all_just_pressed(bevy_keys))
    }

    pub fn all_just_released(&self, inputs: Vec<PyKeyCode>) -> PyResult<bool> {
        let bevy_keys: Vec<KeyCode> = inputs.iter().map(|k| k.to_bevy()).collect();
        Ok(self.as_ref()?.all_just_released(bevy_keys))
    }

    pub fn press(&mut self, input: PyKeyCode) -> PyResult<()> {
        self.as_mut()?.press(input.to_bevy());
        Ok(())
    }

    pub fn release(&mut self, input: PyKeyCode) -> PyResult<()> {
        self.as_mut()?.release(input.to_bevy());
        Ok(())
    }

    pub fn release_all(&mut self) -> PyResult<()> {
        self.as_mut()?.release_all();
        Ok(())
    }

    pub fn clear_just_pressed(&mut self, input: PyKeyCode) -> PyResult<bool> {
        Ok(self.as_mut()?.clear_just_pressed(input.to_bevy()))
    }

    pub fn clear_just_released(&mut self, input: PyKeyCode) -> PyResult<bool> {
        Ok(self.as_mut()?.clear_just_released(input.to_bevy()))
    }

    pub fn reset(&mut self, input: PyKeyCode) -> PyResult<()> {
        self.as_mut()?.reset(input.to_bevy());
        Ok(())
    }

    pub fn reset_all(&mut self) -> PyResult<()> {
        self.as_mut()?.reset_all();
        Ok(())
    }

    pub fn clear(&mut self) -> PyResult<()> {
        self.as_mut()?.clear();
        Ok(())
    }

    fn __repr__(&self) -> String {
        match self.as_ref() {
            Ok(input) => {
                let pressed: Vec<String> = input
                    .get_pressed()
                    .map(|key| format!("{:?}", key))
                    .collect();
                format!("ButtonInput(pressed=[{}])", pressed.join(", "))
            }
            Err(_) => "ButtonInput(<invalid>)".to_string(),
        }
    }
}
