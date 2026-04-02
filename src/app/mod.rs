use bevy::{
    app::{
        First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate, Last, Main,
        PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, Update,
    },
    ecs::{
        schedule::{InternedScheduleLabel, ScheduleLabel},
        world::World,
    },
};
use pyo3::prelude::*;

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

/// Custom schedule label for RL simulation workloads.
/// Runs between PreUpdate and Update in the frame lifecycle.
/// Sim kernels go in SimTick, visual sync goes in Update.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimTick;

#[pyclass(name = "Stage", frozen)]
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
        match self {
            PyStage::Startup => world.run_schedule(Startup),
            PyStage::Update => world.run_schedule(Update),
            PyStage::Last => world.run_schedule(Last),
            PyStage::FixedUpdate => world.run_schedule(FixedUpdate),
            PyStage::Main => world.run_schedule(Main),
            PyStage::First => world.run_schedule(First),
            PyStage::PreUpdate => world.run_schedule(PreUpdate),
            PyStage::PostUpdate => world.run_schedule(PostUpdate),
            PyStage::PreStartup => world.run_schedule(PreStartup),
            PyStage::PostStartup => world.run_schedule(PostStartup),
            PyStage::FixedFirst => world.run_schedule(FixedFirst),
            PyStage::FixedPreUpdate => world.run_schedule(FixedPreUpdate),
            PyStage::FixedPostUpdate => world.run_schedule(FixedPostUpdate),
            PyStage::FixedLast => world.run_schedule(FixedLast),
            PyStage::SimTick => world.run_schedule(SimTick),
        }
    }

    /// Return the interned Bevy schedule label for this stage.
    pub fn intern_label(self) -> InternedScheduleLabel {
        match self {
            PyStage::Startup => Startup.intern(),
            PyStage::PreStartup => PreStartup.intern(),
            PyStage::PostStartup => PostStartup.intern(),
            PyStage::Main => Main.intern(),
            PyStage::First => First.intern(),
            PyStage::PreUpdate => PreUpdate.intern(),
            PyStage::Update => Update.intern(),
            PyStage::PostUpdate => PostUpdate.intern(),
            PyStage::Last => Last.intern(),
            PyStage::FixedFirst => FixedFirst.intern(),
            PyStage::FixedPreUpdate => FixedPreUpdate.intern(),
            PyStage::FixedUpdate => FixedUpdate.intern(),
            PyStage::FixedPostUpdate => FixedPostUpdate.intern(),
            PyStage::FixedLast => FixedLast.intern(),
            PyStage::SimTick => SimTick.intern(),
        }
    }

    /// Whether this stage is a Startup variant (Startup, PreStartup, PostStartup).
    pub fn is_startup(self) -> bool {
        matches!(
            self,
            PyStage::Startup | PyStage::PreStartup | PyStage::PostStartup
        )
    }
}

pub(crate) fn add_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    let app = PyModule::new(m.py(), "app")?;
    app.add_class::<app::PyApp>()?;
    app.add_class::<PyStage>()?;
    app.add_class::<app_exit::PyAppExit>()?;
    app.add_class::<chained_systems::PyChainedSystems>()?;
    app.add_class::<hot_reload::PyAppReloadState>()?;
    app.add_class::<hot_reload::PyHotReloadControl>()?;
    app.add_class::<hot_reload::PyHotReloadPlugin>()?;
    app.add_class::<plugins::PyDefaultPlugins>()?;
    app.add_class::<plugins::PyPluginGroupBuilder>()?;
    app.add_class::<plugins::PyMinimalPlugins>()?;
    // Re-export PyPlugin from the app module so Python can import it from pybevy.app
    // The actual class is registered at root level from pybevy_core to ensure single type identity
    app.add_class::<plugin::PyPlugin>()?;
    app.add_class::<plugin::PyPluginGroup>()?;
    app.add_class::<schedule_runner::PyScheduleRunnerPlugin>()?;
    app.add_class::<schedule_runner::PyRunMode>()?;
    app.add_class::<task_pool::PyTaskPoolPlugin>()?;
    app.add_function(wrap_pyfunction!(chained_systems::chain, &app)?)?;

    // Test-only functions for verifying atexit cleanup behavior
    app.add_function(wrap_pyfunction!(app::_test_get_app_count, &app)?)?;
    app.add_function(wrap_pyfunction!(app::_test_force_cleanup, &app)?)?;

    // Add schedule label constants for convenient imports
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
