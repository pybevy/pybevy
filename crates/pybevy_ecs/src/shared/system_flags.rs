use bevy::ecs::system::SystemStateFlags;

/// Compute the system state flags based on parameter requirements.
///
/// - `needs_exclusive`: system has a World parameter (requires exclusive access)
/// - `needs_commands`: system has a Commands parameter (requires deferred sync points)
/// - `needs_main_thread`: system has a `NonSendMarker` parameter
///
/// Exclusive systems are NON_SEND | EXCLUSIVE, exactly like Bevy's
/// ExclusiveFunctionSystem. A Send exclusive system is a state vanilla Bevy
/// cannot construct, and MultiThreadedExecutor's local-thread accounting
/// assumes the pairing: exclusive spawn sets `local_thread_running`
/// unconditionally but only non-Send completion clears it, so a bare
/// EXCLUSIVE system leaks the flag and permanently blocks every non-Send
/// system still in the queue (`ready_systems.is_clear()` assertion at scope
/// end).
///
/// A main-thread system is NON_SEND without EXCLUSIVE, as Bevy's
/// `NonSendMarker::init_access` sets it; it may also be DEFERRED.
pub fn compute_system_flags(
    needs_exclusive: bool,
    needs_commands: bool,
    needs_main_thread: bool,
) -> SystemStateFlags {
    if needs_exclusive {
        return SystemStateFlags::NON_SEND | SystemStateFlags::EXCLUSIVE;
    }
    let mut flags = SystemStateFlags::empty();
    if needs_main_thread {
        flags |= SystemStateFlags::NON_SEND;
    }
    if needs_commands {
        flags |= SystemStateFlags::DEFERRED;
    }
    flags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_takes_priority_over_commands() {
        let flags = compute_system_flags(true, true, false);
        assert!(flags.contains(SystemStateFlags::EXCLUSIVE));
        assert!(flags.contains(SystemStateFlags::NON_SEND));
    }

    #[test]
    fn exclusive_without_commands() {
        let flags = compute_system_flags(true, false, false);
        assert!(flags.contains(SystemStateFlags::EXCLUSIVE));
        assert!(flags.contains(SystemStateFlags::NON_SEND));
    }

    #[test]
    fn commands_only() {
        let flags = compute_system_flags(false, true, false);
        assert!(flags.contains(SystemStateFlags::DEFERRED));
        assert!(!flags.contains(SystemStateFlags::EXCLUSIVE));
    }

    #[test]
    fn neither_exclusive_nor_commands() {
        let flags = compute_system_flags(false, false, false);
        assert!(flags.is_empty());
    }

    #[test]
    fn main_thread_is_non_send_without_being_exclusive() {
        let flags = compute_system_flags(false, false, true);
        assert!(flags == SystemStateFlags::NON_SEND);
    }

    #[test]
    fn main_thread_keeps_deferred_commands() {
        let flags = compute_system_flags(false, true, true);
        assert!(flags == SystemStateFlags::NON_SEND | SystemStateFlags::DEFERRED);
    }

    #[test]
    fn exclusive_is_already_main_thread() {
        assert!(
            compute_system_flags(true, false, true) == compute_system_flags(true, false, false)
        );
    }
}
