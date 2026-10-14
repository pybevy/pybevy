use bevy::tasks::{AsyncComputeTaskPool, ComputeTaskPool, IoTaskPool};
use pyo3::{prelude::*, types::PyDict};

fn policy<'py>(app_module: &Bound<'py, PyAny>, threads: usize) -> PyResult<Bound<'py, PyAny>> {
    let kwargs = PyDict::new(app_module.py());
    kwargs.set_item("min_threads", threads)?;
    kwargs.set_item("max_threads", threads)?;
    kwargs.set_item("percent", 1.0)?;
    app_module
        .getattr("TaskPoolThreadAssignmentPolicy")?
        .call((), Some(&kwargs))
}

#[test]
fn configured_options_initialize_the_real_global_pools() -> PyResult<()> {
    Python::attach(|py| {
        let module = PyModule::new(py, "_pybevy")?;
        pybevy::init_module(&module)?;
        let app_module = module.getattr("app")?;

        let options_kwargs = PyDict::new(py);
        options_kwargs.set_item("min_total_threads", 6)?;
        options_kwargs.set_item("max_total_threads", 6)?;
        options_kwargs.set_item("io", policy(&app_module, 1)?)?;
        options_kwargs.set_item("async_compute", policy(&app_module, 3)?)?;
        options_kwargs.set_item("compute", policy(&app_module, 2)?)?;
        let options = app_module
            .getattr("TaskPoolOptions")?
            .call((), Some(&options_kwargs))?;

        let plugin_kwargs = PyDict::new(py);
        plugin_kwargs.set_item("task_pool_options", options)?;
        let plugin = app_module
            .getattr("TaskPoolPlugin")?
            .call((), Some(&plugin_kwargs))?;
        let retained_options = plugin.getattr("task_pool_options")?;
        retained_options.setattr("compute", policy(&app_module, 4)?)?;
        let app = app_module.getattr("App")?.call0()?;
        app.call_method1("add_plugins", (plugin,))?;
        Ok::<(), PyErr>(())
    })?;

    assert_eq!(ComputeTaskPool::get().thread_num(), 4);
    assert_eq!(IoTaskPool::get().thread_num(), 1);
    assert_eq!(AsyncComputeTaskPool::get().thread_num(), 3);
    Ok(())
}
