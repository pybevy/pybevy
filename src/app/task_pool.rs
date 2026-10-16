use bevy::app::{App, TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy};
use pybevy_core::{FieldStorage, FromBorrowedStorage, Omittable, PluginBuild, PyPlugin};
use pybevy_macros::{pyfield, pyplugin};
use pyo3::{PyTraverseError, PyVisit, prelude::*};

use crate::app::app::PyApp;

#[pyfield(domains = [percent => [non_negative], ordered(min_threads, max_threads)])]
#[pyclass(
    name = "TaskPoolThreadAssignmentPolicy",
    module = "pybevy.app",
    skip_from_py_object
)]
#[derive(Debug)]
pub struct PyTaskPoolThreadAssignmentPolicy {
    storage: FieldStorage<TaskPoolThreadAssignmentPolicy>,
}

impl PyTaskPoolThreadAssignmentPolicy {
    fn to_bevy(&self) -> PyResult<TaskPoolThreadAssignmentPolicy> {
        Ok(self.storage.get()?)
    }
}

#[pymethods]
impl PyTaskPoolThreadAssignmentPolicy {
    #[new]
    #[pyo3(signature = (*, min_threads, max_threads, percent))]
    pub fn new(min_threads: usize, max_threads: usize, percent: f32) -> PyResult<Self> {
        Self::check_min_threads_order(min_threads, max_threads)?;
        let percent = Self::check_percent(percent)?;
        Ok(Self::from_owned(TaskPoolThreadAssignmentPolicy {
            min_threads,
            max_threads,
            percent,
            on_thread_spawn: None,
            on_thread_destroy: None,
        }))
    }

    #[getter]
    pub fn min_threads(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.min_threads)
    }

    #[setter]
    pub fn set_min_threads(&mut self, min_threads: usize) -> PyResult<()> {
        let mut policy = self.as_mut()?;
        Self::check_min_threads_order(min_threads, policy.max_threads)?;
        policy.min_threads = min_threads;
        Ok(())
    }

    #[getter]
    pub fn max_threads(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.max_threads)
    }

    #[setter]
    pub fn set_max_threads(&mut self, max_threads: usize) -> PyResult<()> {
        let mut policy = self.as_mut()?;
        Self::check_min_threads_order(policy.min_threads, max_threads)?;
        policy.max_threads = max_threads;
        Ok(())
    }

    #[getter]
    pub fn percent(&self) -> PyResult<f32> {
        Ok(self.as_ref()?.percent)
    }

    #[setter]
    pub fn set_percent(&mut self, percent: f32) -> PyResult<()> {
        let percent = Self::check_percent(percent)?;
        self.as_mut()?.percent = percent;
        Ok(())
    }
}

#[pyfield(domains = [ordered(min_total_threads, max_total_threads)])]
#[pyclass(name = "TaskPoolOptions", module = "pybevy.app", skip_from_py_object)]
#[derive(Debug)]
pub struct PyTaskPoolOptions {
    storage: FieldStorage<TaskPoolOptions>,
}

impl PyTaskPoolOptions {
    fn to_bevy(&self) -> PyResult<TaskPoolOptions> {
        Ok(self.storage.get()?)
    }
}

fn policy_or_default(
    py: Python<'_>,
    policy: Omittable<Py<PyTaskPoolThreadAssignmentPolicy>>,
    default: TaskPoolThreadAssignmentPolicy,
) -> PyResult<TaskPoolThreadAssignmentPolicy> {
    match policy.into_option() {
        Some(policy) => policy.bind(py).try_borrow()?.to_bevy(),
        None => Ok(default),
    }
}

#[pymethods]
impl PyTaskPoolOptions {
    #[new]
    #[pyo3(signature = (
        *,
        min_total_threads = 1,
        max_total_threads = usize::MAX,
        io = Omittable::default(),
        async_compute = Omittable::default(),
        compute = Omittable::default(),
    ))]
    pub fn new(
        py: Python<'_>,
        min_total_threads: usize,
        max_total_threads: usize,
        io: Omittable<Py<PyTaskPoolThreadAssignmentPolicy>>,
        async_compute: Omittable<Py<PyTaskPoolThreadAssignmentPolicy>>,
        compute: Omittable<Py<PyTaskPoolThreadAssignmentPolicy>>,
    ) -> PyResult<Self> {
        Self::check_min_total_threads_order(min_total_threads, max_total_threads)?;
        let defaults = TaskPoolOptions::default();
        Ok(Self::from_owned(TaskPoolOptions {
            min_total_threads,
            max_total_threads,
            io: policy_or_default(py, io, defaults.io)?,
            async_compute: policy_or_default(py, async_compute, defaults.async_compute)?,
            compute: policy_or_default(py, compute, defaults.compute)?,
        }))
    }

    #[staticmethod]
    pub fn with_num_threads(thread_count: usize) -> Self {
        Self::from_owned(TaskPoolOptions::with_num_threads(thread_count))
    }

    #[getter]
    pub fn min_total_threads(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.min_total_threads)
    }

    #[setter]
    pub fn set_min_total_threads(&mut self, min_total_threads: usize) -> PyResult<()> {
        let mut options = self.as_mut()?;
        Self::check_min_total_threads_order(min_total_threads, options.max_total_threads)?;
        options.min_total_threads = min_total_threads;
        Ok(())
    }

    #[getter]
    pub fn max_total_threads(&self) -> PyResult<usize> {
        Ok(self.as_ref()?.max_total_threads)
    }

    #[setter]
    pub fn set_max_total_threads(&mut self, max_total_threads: usize) -> PyResult<()> {
        let mut options = self.as_mut()?;
        Self::check_min_total_threads_order(options.min_total_threads, max_total_threads)?;
        options.max_total_threads = max_total_threads;
        Ok(())
    }

    #[getter]
    pub fn io(&self) -> PyResult<PyTaskPoolThreadAssignmentPolicy> {
        Ok(self.storage.borrow_field_as(|options| &options.io)?)
    }

    #[setter]
    pub fn set_io(&mut self, io: PyRef<'_, PyTaskPoolThreadAssignmentPolicy>) -> PyResult<()> {
        self.as_mut()?.io = io.to_bevy()?;
        Ok(())
    }

    #[getter]
    pub fn async_compute(&self) -> PyResult<PyTaskPoolThreadAssignmentPolicy> {
        Ok(self
            .storage
            .borrow_field_as(|options| &options.async_compute)?)
    }

    #[setter]
    pub fn set_async_compute(
        &mut self,
        async_compute: PyRef<'_, PyTaskPoolThreadAssignmentPolicy>,
    ) -> PyResult<()> {
        self.as_mut()?.async_compute = async_compute.to_bevy()?;
        Ok(())
    }

    #[getter]
    pub fn compute(&self) -> PyResult<PyTaskPoolThreadAssignmentPolicy> {
        Ok(self.storage.borrow_field_as(|options| &options.compute)?)
    }

    #[setter]
    pub fn set_compute(
        &mut self,
        compute: PyRef<'_, PyTaskPoolThreadAssignmentPolicy>,
    ) -> PyResult<()> {
        self.as_mut()?.compute = compute.to_bevy()?;
        Ok(())
    }
}

#[pyplugin(TaskPoolPlugin, default_plugin = TaskPool)]
#[pyclass(
    name = "TaskPoolPlugin",
    module = "pybevy.app",
    extends = PyPlugin,
    skip_from_py_object
)]
pub struct PyTaskPoolPlugin {
    task_pool_options: Py<PyTaskPoolOptions>,
}

impl Clone for PyTaskPoolPlugin {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            task_pool_options: self.task_pool_options.clone_ref(py),
        })
    }
}

impl PyTaskPoolPlugin {
    pub(crate) fn to_bevy_plugin(&self, py: Python<'_>) -> PyResult<TaskPoolPlugin> {
        Ok(TaskPoolPlugin {
            task_pool_options: self.task_pool_options.bind(py).try_borrow()?.to_bevy()?,
        })
    }
}

#[pymethods]
impl PyTaskPoolPlugin {
    #[new]
    #[pyo3(signature = (*, task_pool_options = Omittable::default()))]
    pub fn new(
        py: Python<'_>,
        task_pool_options: Omittable<Py<PyTaskPoolOptions>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok((
            Self {
                task_pool_options: match task_pool_options.into_option() {
                    Some(options) => options,
                    None => Py::new(
                        py,
                        PyTaskPoolOptions::from_owned(TaskPoolOptions::default()),
                    )?,
                },
            },
            PyPlugin,
        )
            .into())
    }

    #[getter]
    pub fn task_pool_options(&self, py: Python<'_>) -> Py<PyTaskPoolOptions> {
        self.task_pool_options.clone_ref(py)
    }

    #[setter]
    pub fn set_task_pool_options(&mut self, options: Py<PyTaskPoolOptions>) {
        self.task_pool_options = options;
    }

    pub fn build(&self, app: Bound<'_, PyApp>) -> PyResult<()> {
        let plugin = self.to_bevy_plugin(app.py())?;
        app.borrow().with_bevy_app(|bevy_app| {
            bevy_app.add_plugins(plugin);
            Ok(())
        })
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.task_pool_options)
    }
}

impl PluginBuild for PyTaskPoolPlugin {
    fn build(py_plugin: &Bound<'_, PyAny>, app: &mut App) -> PyResult<()> {
        let config = py_plugin.cast::<Self>()?.try_borrow()?;
        app.add_plugins(config.to_bevy_plugin(py_plugin.py())?);
        Ok(())
    }
}
