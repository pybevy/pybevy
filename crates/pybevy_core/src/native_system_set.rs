use bevy::ecs::schedule::InternedSystemSet;

/// Interpreter-neutral metadata for one public native Bevy system-set value.
///
/// Feature crates publish registrations through `inventory`; each interpreter
/// adapter materializes its own Python object from the same native label.
pub struct NativeSystemSetRegistration {
    /// Public PyBevy submodule, without the `pybevy.` prefix.
    pub module: &'static str,
    /// Public Python name of the system-set value.
    pub name: &'static str,
    /// Intern the concrete Bevy system-set label.
    pub intern: fn() -> InternedSystemSet,
}

inventory::collect!(NativeSystemSetRegistration);

/// Publish a public native Bevy system-set value to every interpreter adapter.
#[macro_export]
macro_rules! register_native_system_set {
    ($factory:ident, $set:expr, module = $module:literal, name = $name:literal) => {
        fn $factory() -> bevy::ecs::schedule::InternedSystemSet {
            use bevy::ecs::schedule::SystemSet as _;
            ($set).intern()
        }

        $crate::inventory::submit!($crate::NativeSystemSetRegistration {
            module: $module,
            name: $name,
            intern: $factory,
        });
    };
}
