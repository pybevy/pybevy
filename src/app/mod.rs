use bevy::ecs::{schedule::InternedScheduleLabel, world::World};
use pybevy_ecs::shared::schedule::ScheduleKind;
use pyo3::prelude::*;

pybevy_core::register_native_system_set!(
    intern_animation_systems,
    bevy::app::AnimationSystems,
    module = "app",
    name = "AnimationSystems"
);

pub mod app;
pub mod app_exit;
pub mod chained_systems;
pub mod error_messages;
pub mod hot_reload;
pub mod plugin;
pub mod plugin_config;
pub mod plugins;
pub mod schedule_runner;
pub mod task_pool;

#[pyclass(name = "Stage", module = "pybevy.app", frozen, from_py_object)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PyStage {
    Startup,
    Update,
    Last,
    FixedUpdate,
    Main,
    First,
    PreUpdate,
    PostUpdate,
    PreStartup,
    PostStartup,
    FixedFirst,
    FixedPreUpdate,
    FixedPostUpdate,
    FixedLast,
    SimTick,
}

impl PyStage {
    /// Run the corresponding Bevy schedule on the given World.
    pub fn run_on_world(self, world: &mut World) {
        ScheduleKind::from(self).run_on_world(world);
    }

    /// Return the interned Bevy schedule label for this stage.
    pub fn intern_label(self) -> InternedScheduleLabel {
        ScheduleKind::from(self).intern_label()
    }

    /// Whether this stage is a Startup variant (Startup, PreStartup, PostStartup).
    pub fn is_startup(self) -> bool {
        ScheduleKind::from(self).is_startup()
    }
}

impl From<PyStage> for ScheduleKind {
    fn from(stage: PyStage) -> Self {
        match stage {
            PyStage::Startup => Self::Startup,
            PyStage::Update => Self::Update,
            PyStage::Last => Self::Last,
            PyStage::FixedUpdate => Self::FixedUpdate,
            PyStage::Main => Self::Main,
            PyStage::First => Self::First,
            PyStage::PreUpdate => Self::PreUpdate,
            PyStage::PostUpdate => Self::PostUpdate,
            PyStage::PreStartup => Self::PreStartup,
            PyStage::PostStartup => Self::PostStartup,
            PyStage::FixedFirst => Self::FixedFirst,
            PyStage::FixedPreUpdate => Self::FixedPreUpdate,
            PyStage::FixedPostUpdate => Self::FixedPostUpdate,
            PyStage::FixedLast => Self::FixedLast,
            PyStage::SimTick => Self::SimTick,
        }
    }
}

pub(crate) fn add_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let app = PyModule::new(m.py(), "app")?;
    app.add_class::<app::PyApp>()?;
    app.add_class::<PyStage>()?;
    app.add_class::<app_exit::PyAppExit>()?;
    app_exit::register_app_exit_variants(&app)?;
    app.add_class::<chained_systems::PyChainedSystems>()?;
    app.add_class::<chained_systems::PyChainedSystemSets>()?;
    app.add_class::<hot_reload::bindings::PyAppReloadState>()?;
    app.add_class::<hot_reload::bindings::PyHotReloadControl>()?;
    app.add_class::<hot_reload::bindings::PyHotReloadPlugin>()?;
    app.add_class::<hot_reload::watcher::PyFileWatcher>()?;
    app.add_class::<plugins::PyDefaultPlugins>()?;
    app.add_class::<plugins::PyPluginGroupBuilder>()?;
    app.add_class::<plugins::PyMinimalPlugins>()?;
    app.add_class::<plugin::PyPlugin>()?;
    app.add_class::<plugin::PyPluginGroup>()?;
    app.add_class::<schedule_runner::PyScheduleRunnerPlugin>()?;
    app.add_class::<schedule_runner::PyRunMode>()?;
    app.add_class::<task_pool::PyTaskPoolPlugin>()?;
    app.add_function(wrap_pyfunction!(chained_systems::chain, &app)?)?;

    // Internal test-only functions
    app.add_function(wrap_pyfunction!(app::_test_get_app_count, &app)?)?;
    app.add_function(wrap_pyfunction!(app::_test_force_cleanup, &app)?)?;

    // Add schedule label constants for convenient imports
    // TODO: use a macro for these? auto-generated from PyStage
    app.add("Startup", PyStage::Startup)?;
    app.add("Update", PyStage::Update)?;
    app.add("Last", PyStage::Last)?;
    app.add("FixedUpdate", PyStage::FixedUpdate)?;
    app.add("Main", PyStage::Main)?;
    app.add("First", PyStage::First)?;
    app.add("PreUpdate", PyStage::PreUpdate)?;
    app.add("PostUpdate", PyStage::PostUpdate)?;
    app.add("PreStartup", PyStage::PreStartup)?;
    app.add("PostStartup", PyStage::PostStartup)?;
    app.add("FixedFirst", PyStage::FixedFirst)?;
    app.add("FixedPreUpdate", PyStage::FixedPreUpdate)?;
    app.add("FixedPostUpdate", PyStage::FixedPostUpdate)?;
    app.add("FixedLast", PyStage::FixedLast)?;
    app.add("SimTick", PyStage::SimTick)?;

    m.add_submodule(&app)
}
