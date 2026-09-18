use bevy::app::{App, TaskPoolOptions, TaskPoolPlugin, TaskPoolThreadAssignmentPolicy};
use pybevy_core::{PluginBuild, PyPlugin, public_error};
use pybevy_macros::pyplugin;
use pyo3::{PyTraverseError, PyVisit, exceptions::PyValueError, prelude::*};

use crate::app::app::PyApp;

fn validate_policy(min_threads: usize, max_threads: usize, percent: f32) -> PyResult<()> {
    if min_threads > max_threads {
        return Err(PyValueError::new_err(
            public_error::task_pool_thread_bounds(min_threads, max_threads),
        ));
    }
    if percent.is_nan() || percent < 0.0 {
        return Err(PyValueError::new_err(
            public_error::TASK_POOL_PERCENT_NONNEGATIVE,
        ));
    }
    Ok(())
}

fn validate_total_threads(min_total_threads: usize, max_total_threads: usize) -> PyResult<()> {
    if min_total_threads > max_total_threads {
        return Err(PyValueError::new_err(
            public_error::task_pool_total_thread_bounds(min_total_threads, max_total_threads),
        ));
    }
    Ok(())
}

#[pyclass(
    name = "TaskPoolThreadAssignmentPolicy",
    module = "pybevy.app",
    skip_from_py_object
)]
#[derive(Clone, Debug)]
pub struct PyTaskPoolThreadAssignmentPolicy {
    policy: TaskPoolThreadAssignmentPolicy,
}

impl PyTaskPoolThreadAssignmentPolicy {
    fn from_bevy(policy: TaskPoolThreadAssignmentPolicy) -> Self {
        Self { policy }
    }

    fn to_bevy(&self) -> TaskPoolThreadAssignmentPolicy {
        self.policy.clone()
    }
}

#[pymethods]
impl PyTaskPoolThreadAssignmentPolicy {
    #[new]
    #[pyo3(signature = (*, min_threads, max_threads, percent))]
    pub fn new(min_threads: usize, max_threads: usize, percent: f32) -> PyResult<Self> {
        validate_policy(min_threads, max_threads, percent)?;
        Ok(Self {
            policy: TaskPoolThreadAssignmentPolicy {
                min_threads,
                max_threads,
                percent,
                on_thread_spawn: None,
                on_thread_destroy: None,
            },
        })
    }

    #[getter]
    pub fn min_threads(&self) -> usize {
        self.policy.min_threads
    }

    #[setter]
    pub fn set_min_threads(&mut self, min_threads: usize) -> PyResult<()> {
        validate_policy(min_threads, self.policy.max_threads, self.policy.percent)?;
        self.policy.min_threads = min_threads;
        Ok(())
    }

    #[getter]
    pub fn max_threads(&self) -> usize {
        self.policy.max_threads
    }

    #[setter]
    pub fn set_max_threads(&mut self, max_threads: usize) -> PyResult<()> {
        validate_policy(self.policy.min_threads, max_threads, self.policy.percent)?;
        self.policy.max_threads = max_threads;
        Ok(())
    }

    #[getter]
    pub fn percent(&self) -> f32 {
        self.policy.percent
    }

    #[setter]
    pub fn set_percent(&mut self, percent: f32) -> PyResult<()> {
        validate_policy(self.policy.min_threads, self.policy.max_threads, percent)?;
        self.policy.percent = percent;
        Ok(())
    }
}

#[pyclass(name = "TaskPoolOptions", module = "pybevy.app", skip_from_py_object)]
pub struct PyTaskPoolOptions {
    min_total_threads: usize,
    max_total_threads: usize,
    io: Py<PyTaskPoolThreadAssignmentPolicy>,
    async_compute: Py<PyTaskPoolThreadAssignmentPolicy>,
    compute: Py<PyTaskPoolThreadAssignmentPolicy>,
}

impl Clone for PyTaskPoolOptions {
    fn clone(&self) -> Self {
        Python::attach(|py| Self {
            min_total_threads: self.min_total_threads,
            max_total_threads: self.max_total_threads,
            io: self.io.clone_ref(py),
            async_compute: self.async_compute.clone_ref(py),
            compute: self.compute.clone_ref(py),
        })
    }
}

impl PyTaskPoolOptions {
    fn from_bevy(py: Python<'_>, options: TaskPoolOptions) -> PyResult<Self> {
        Ok(Self {
            min_total_threads: options.min_total_threads,
            max_total_threads: options.max_total_threads,
            io: Py::new(py, PyTaskPoolThreadAssignmentPolicy::from_bevy(options.io))?,
            async_compute: Py::new(
                py,
                PyTaskPoolThreadAssignmentPolicy::from_bevy(options.async_compute),
            )?,
            compute: Py::new(
                py,
                PyTaskPoolThreadAssignmentPolicy::from_bevy(options.compute),
            )?,
        })
    }

    fn to_bevy(&self, py: Python<'_>) -> PyResult<TaskPoolOptions> {
        Ok(TaskPoolOptions {
            min_total_threads: self.min_total_threads,
            max_total_threads: self.max_total_threads,
            io: self.io.bind(py).try_borrow()?.to_bevy(),
            async_compute: self.async_compute.bind(py).try_borrow()?.to_bevy(),
            compute: self.compute.bind(py).try_borrow()?.to_bevy(),
        })
    }
}

#[pymethods]
impl PyTaskPoolOptions {
    #[new]
    #[pyo3(signature = (
        *,
        min_total_threads = 1,
        max_total_threads = usize::MAX,
        io = None,
        async_compute = None,
        compute = None,
    ))]
    pub fn new(
        py: Python<'_>,
        min_total_threads: usize,
        max_total_threads: usize,
        io: Option<Py<PyTaskPoolThreadAssignmentPolicy>>,
        async_compute: Option<Py<PyTaskPoolThreadAssignmentPolicy>>,
        compute: Option<Py<PyTaskPoolThreadAssignmentPolicy>>,
    ) -> PyResult<Self> {
        validate_total_threads(min_total_threads, max_total_threads)?;
        let defaults = TaskPoolOptions::default();
        Ok(Self {
            min_total_threads,
            max_total_threads,
            io: match io {
                Some(io) => io,
                None => Py::new(py, PyTaskPoolThreadAssignmentPolicy::from_bevy(defaults.io))?,
            },
            async_compute: match async_compute {
                Some(async_compute) => async_compute,
                None => Py::new(
                    py,
                    PyTaskPoolThreadAssignmentPolicy::from_bevy(defaults.async_compute),
                )?,
            },
            compute: match compute {
                Some(compute) => compute,
                None => Py::new(
                    py,
                    PyTaskPoolThreadAssignmentPolicy::from_bevy(defaults.compute),
                )?,
            },
        })
    }

    #[staticmethod]
    pub fn with_num_threads(py: Python<'_>, thread_count: usize) -> PyResult<Self> {
        Self::from_bevy(py, TaskPoolOptions::with_num_threads(thread_count))
    }

    #[getter]
    pub fn min_total_threads(&self) -> usize {
        self.min_total_threads
    }

    #[setter]
    pub fn set_min_total_threads(&mut self, min_total_threads: usize) -> PyResult<()> {
        validate_total_threads(min_total_threads, self.max_total_threads)?;
        self.min_total_threads = min_total_threads;
        Ok(())
    }

    #[getter]
    pub fn max_total_threads(&self) -> usize {
        self.max_total_threads
    }

    #[setter]
    pub fn set_max_total_threads(&mut self, max_total_threads: usize) -> PyResult<()> {
        validate_total_threads(self.min_total_threads, max_total_threads)?;
        self.max_total_threads = max_total_threads;
        Ok(())
    }

    #[getter]
    pub fn io(&self, py: Python<'_>) -> Py<PyTaskPoolThreadAssignmentPolicy> {
        self.io.clone_ref(py)
    }

    #[setter]
    pub fn set_io(&mut self, io: Py<PyTaskPoolThreadAssignmentPolicy>) {
        self.io = io;
    }

    #[getter]
    pub fn async_compute(&self, py: Python<'_>) -> Py<PyTaskPoolThreadAssignmentPolicy> {
        self.async_compute.clone_ref(py)
    }

    #[setter]
    pub fn set_async_compute(&mut self, async_compute: Py<PyTaskPoolThreadAssignmentPolicy>) {
        self.async_compute = async_compute;
    }

    #[getter]
    pub fn compute(&self, py: Python<'_>) -> Py<PyTaskPoolThreadAssignmentPolicy> {
        self.compute.clone_ref(py)
    }

    #[setter]
    pub fn set_compute(&mut self, compute: Py<PyTaskPoolThreadAssignmentPolicy>) {
        self.compute = compute;
    }

    fn __traverse__(&self, visit: PyVisit<'_>) -> Result<(), PyTraverseError> {
        visit.call(&self.io)?;
        visit.call(&self.async_compute)?;
        visit.call(&self.compute)
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
            task_pool_options: self.task_pool_options.bind(py).try_borrow()?.to_bevy(py)?,
        })
    }
}

#[pymethods]
impl PyTaskPoolPlugin {
    #[new]
    #[pyo3(signature = (*, task_pool_options = None))]
    pub fn new(
        py: Python<'_>,
        task_pool_options: Option<Py<PyTaskPoolOptions>>,
    ) -> PyResult<PyClassInitializer<Self>> {
        Ok((
            Self {
                task_pool_options: match task_pool_options {
                    Some(options) => options,
                    None => Py::new(
                        py,
                        PyTaskPoolOptions::from_bevy(py, TaskPoolOptions::default())?,
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
