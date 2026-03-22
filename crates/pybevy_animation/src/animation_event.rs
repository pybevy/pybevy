use pyo3::prelude::*;

#[pyclass(name = "AnimationEvent", eq)]
#[derive(Clone, Debug, PartialEq)]
pub struct PyAnimationEvent {
    #[pyo3(get)]
    pub name: String,

    #[pyo3(get)]
    pub data: Option<String>,
}

#[pymethods]
impl PyAnimationEvent {
    #[new]
    #[pyo3(signature = (name, data=None))]
    pub fn new(name: String, data: Option<String>) -> Self {
        Self { name, data }
    }

    pub fn __repr__(&self) -> String {
        match &self.data {
            Some(data) => format!("AnimationEvent(name='{}', data='{}')", self.name, data),
            None => format!("AnimationEvent(name='{}')", self.name),
        }
    }
}

#[pyclass(name = "AnimationEventData")]
#[derive(Debug)]
pub struct PyAnimationEventData {
    #[pyo3(get)]
    pub time: f32,

    #[pyo3(get)]
    pub event: Py<PyAnimationEvent>,
}

impl Clone for PyAnimationEventData {
    fn clone(&self) -> Self {
        Python::attach(|py| PyAnimationEventData {
            time: self.time,
            event: self.event.clone_ref(py),
        })
    }
}

#[pymethods]
impl PyAnimationEventData {
    #[new]
    pub fn new(time: f32, event: Py<PyAnimationEvent>) -> Self {
        Self { time, event }
    }

    pub fn __repr__(&self) -> String {
        Python::attach(|py| {
            let event_repr = self.event.borrow(py).__repr__();
            format!(
                "AnimationEventData(time={:.1}, event={})",
                self.time, event_repr
            )
        })
    }
}
