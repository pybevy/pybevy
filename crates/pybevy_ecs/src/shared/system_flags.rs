use bevy::ecs::system::SystemStateFlags;

/// Compute the system state flags based on parameter requirements.
///
/// - `needs_exclusive`: system has a World parameter (requires exclusive access)
/// - `needs_commands`: system has a Commands parameter (requires deferred sync points)
pub fn compute_system_flags(needs_exclusive: bool, needs_commands: bool) -> SystemStateFlags {
    if needs_exclusive {
        SystemStateFlags::EXCLUSIVE
    } else if needs_commands {
        SystemStateFlags::DEFERRED
    } else {
        SystemStateFlags::empty()
    }
}
