//! Root-crate integration around the shared renderer readback core.

use bevy::prelude::*;
pub use pybevy_render::readback::{FrameReceiver, ImageCopier, list_entities, poll_frame};

/// GPU image readback plus the optional control-server screenshot bridge.
pub struct ImageCopyPlugin;

impl Plugin for ImageCopyPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(pybevy_render::readback::ImageCopyPlugin);

        #[cfg(feature = "mcp")]
        {
            use pybevy_control::handlers::screenshot::HeadlessFrameBuffer;

            app.init_resource::<HeadlessFrameBuffer>();
            app.add_systems(
                Update,
                update_headless_frame_buffer
                    .after(pybevy_render::readback::collect_readback_frames),
            );
        }
    }
}

/// Copy the latest renderer frame into the control server's screenshot buffer.
#[cfg(feature = "mcp")]
fn update_headless_frame_buffer(
    mut buffer: ResMut<pybevy_control::handlers::screenshot::HeadlessFrameBuffer>,
) {
    if let Some(frame) = pybevy_render::readback::poll_latest_frame() {
        buffer.latest = Some(frame);
    }
}
