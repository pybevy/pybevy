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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_takes_priority_over_commands() {
        let flags = compute_system_flags(true, true);
        assert!(flags.contains(SystemStateFlags::EXCLUSIVE));
    }

    #[test]
    fn exclusive_without_commands() {
        let flags = compute_system_flags(true, false);
        assert!(flags.contains(SystemStateFlags::EXCLUSIVE));
    }

    #[test]
    fn commands_only() {
        let flags = compute_system_flags(false, true);
        assert!(flags.contains(SystemStateFlags::DEFERRED));
        assert!(!flags.contains(SystemStateFlags::EXCLUSIVE));
    }

    #[test]
    fn neither_exclusive_nor_commands() {
        let flags = compute_system_flags(false, false);
        assert!(flags.is_empty());
    }
}
