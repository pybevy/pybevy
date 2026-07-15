use bevy::ecs::{
    message::{Message, Messages},
    world::World,
};

/// Ensure a native Bevy message buffer exists without taking ownership away
/// from its plugin.
///
/// Backends run their built-in fallback set in `PreStartup`, after all plugins
/// have built. A plugin-owned buffer is therefore preserved (including the
/// plugin's normal message-update system); only a genuinely absent buffer gets
/// an empty fallback so native reader systems can validate in headless/minimal
/// plugin compositions.
pub fn ensure_message_resource<M: Message>(world: &mut World) -> bool {
    if world.contains_resource::<Messages<M>>() {
        return false;
    }
    world.insert_resource(Messages::<M>::default());
    true
}

#[cfg(test)]
mod tests {
    use bevy::ecs::message::Message;

    use super::*;

    #[derive(Message)]
    struct TestMessage;

    #[test]
    fn fallback_preserves_existing_plugin_owned_buffer() {
        let mut world = World::new();
        assert!(ensure_message_resource::<TestMessage>(&mut world));
        world
            .resource_mut::<Messages<TestMessage>>()
            .write(TestMessage);

        assert!(!ensure_message_resource::<TestMessage>(&mut world));
        assert_eq!(
            world
                .resource::<Messages<TestMessage>>()
                .iter_current_update_messages()
                .count(),
            1
        );
    }
}
