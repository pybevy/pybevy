use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

/// Global frame buffer: entity bits → latest frame bytes.
/// Written by Rust `collect_readback_frames` system, read by Python `poll_readback_frame`.
static READBACK_FRAMES: OnceLock<Mutex<HashMap<u64, Vec<u8>>>> = OnceLock::new();

fn frames_map() -> &'static Mutex<HashMap<u64, Vec<u8>>> {
    READBACK_FRAMES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Latest frame with dimensions, for headless screenshot support.
/// Written by `collect_readback_frames`, read by `HeadlessFrameProvider`.
static LATEST_FRAME: OnceLock<Mutex<Option<(Vec<u8>, u32, u32)>>> = OnceLock::new();

fn latest_frame_slot() -> &'static Mutex<Option<(Vec<u8>, u32, u32)>> {
    LATEST_FRAME.get_or_init(|| Mutex::new(None))
}

/// Poll the latest readback frame with dimensions. Used by the control server
/// for headless screenshots.
pub(crate) fn poll_latest_frame() -> Option<(Vec<u8>, u32, u32)> {
    latest_frame_slot().lock().ok()?.clone()
}

/// Called from Python to get the latest readback frame for a camera entity.
/// Returns None if no frame is available yet.
pub(crate) fn poll_frame(entity_bits: u64) -> Option<Vec<u8>> {
    let mut map = frames_map().lock().ok()?;
    map.remove(&entity_bits)
}

/// List all entity bits that currently have readback frames available.
pub(crate) fn list_entities() -> Vec<u64> {
    let map = frames_map().lock().unwrap_or_else(|e| e.into_inner());
    map.keys().copied().collect()
}

///! GPU Readback System for JupyBevy
///!
///! Adapted from Bevy's headless_renderer example.
///! Architecture:
///! 1. Render to texture (Camera → GPU Image)
///! 2. Copy texture to buffer (ImageCopyDriver node)
///! 3. Map buffer and read pixels (RenderWorld)
///! 4. Send via channel to MainWorld
///! 5. Python extracts pixels via HeadlessRenderer.extract_frame()
use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        Extract, Render, RenderApp, RenderSystems,
        render_asset::RenderAssets,
        render_graph::{self, NodeRunError, RenderGraph, RenderGraphContext, RenderLabel},
        render_resource::{
            Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, MapMode, PollType,
            TexelCopyBufferInfo, TexelCopyBufferLayout,
        },
        renderer::{RenderContext, RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};
use crossbeam_channel::{Receiver, Sender};

/// Component on camera entities: receives pixel data from render world
/// Attached to cameras with RenderToBuffer component to enable frame extraction
#[derive(Component, Clone)]
pub struct FrameReceiver {
    pub(crate) receiver: Receiver<Vec<u8>>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl FrameReceiver {
    pub fn new(width: u32, height: u32) -> (Self, Sender<Vec<u8>>) {
        let (sender, receiver) = crossbeam_channel::unbounded();
        (
            FrameReceiver {
                receiver,
                width,
                height,
            },
            sender,
        )
    }

    /// Try to receive the newest frame without blocking
    ///
    /// This drains the channel and returns the most recent frame.
    /// Important: Early frames may contain uninitialized data if captured
    /// before the camera has rendered. Always use this to get the latest frame.
    pub fn try_recv(&self) -> Option<Vec<u8>> {
        let mut latest = None;
        // Drain channel, keeping only the newest frame
        while let Ok(frame) = self.receiver.try_recv() {
            latest = Some(frame);
        }
        latest
    }
}

/// Component that marks an image as a readback target
/// Spawned with the render target image handle
#[derive(Clone, Component)]
pub struct ImageCopier {
    buffer: Buffer,
    enabled: Arc<AtomicBool>,
    pub src_image: Handle<Image>,
    /// Sender for this specific camera's frame data
    sender: Sender<Vec<u8>>,
}

impl ImageCopier {
    pub fn new(
        src_image: Handle<Image>,
        width: u32,
        height: u32,
        render_device: &RenderDevice,
        sender: Sender<Vec<u8>>,
    ) -> Self {
        // Calculate padded bytes per row (wgpu requires 256-byte alignment)
        // Multiply by 4 (bytes per pixel for RGBA) BEFORE alignment
        let padded_bytes_per_row = RenderDevice::align_copy_bytes_per_row((width * 4) as usize);

        let buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("readback_buffer"),
            size: padded_bytes_per_row as u64 * height as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        ImageCopier {
            buffer,
            src_image,
            enabled: Arc::new(AtomicBool::new(true)),
            sender,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }
}

/// Attach ImageCopier and FrameReceiver to an entity (Rust-native version)
///
/// This is the core implementation used by both Rust and Python bindings.
/// Call this after app initialization when RenderDevice is available.
///
/// # Arguments
/// * `commands` - Bevy Commands for entity modification
/// * `entity` - The camera entity to attach copier to
/// * `image_handle` - Handle to the Image that the camera renders to
/// * `width` - Width of the render target
/// * `height` - Height of the render target
/// * `render_device` - Bevy RenderDevice resource
///
/// # Returns
/// The FrameReceiver that can be used to retrieve captured frames
pub fn attach_image_copier(
    commands: &mut Commands,
    entity: Entity,
    image_handle: Handle<Image>,
    width: u32,
    height: u32,
    render_device: &RenderDevice,
) -> FrameReceiver {
    // Create FrameReceiver and get its sender
    let (frame_receiver, sender) = FrameReceiver::new(width, height);

    // Create ImageCopier with the sender
    let copier = ImageCopier::new(image_handle, width, height, render_device, sender);

    // Add both components to the entity
    commands
        .entity(entity)
        .insert((copier, frame_receiver.clone()));

    frame_receiver
}

/// Aggregator resource in RenderWorld
#[derive(Clone, Default, Resource, Deref, DerefMut)]
struct ImageCopiers(pub Vec<ImageCopier>);

/// Plugin that sets up GPU readback infrastructure.
///
/// Auto-detects cameras with `RenderTarget::Image` and attaches
/// `ImageCopier` + `FrameReceiver` so frames can be read from Python.
pub struct ImageCopyPlugin;

impl Plugin for ImageCopyPlugin {
    fn build(&self, app: &mut App) {
        debug!("[ImageCopyPlugin] build() called - setting up GPU readback");

        // Main-world systems:
        // 1. Auto-attach readback to render-target cameras
        // 2. Collect frames from FrameReceivers into global buffer for Python
        app.add_systems(Update, (auto_attach_readback, collect_readback_frames));

        // Register HeadlessFrameBuffer for the control server's screenshot fallback.
        // Updated each frame by update_headless_frame_buffer system.
        #[cfg(feature = "mcp")]
        {
            use pybevy_control::handlers::screenshot::HeadlessFrameBuffer;
            app.init_resource::<HeadlessFrameBuffer>();
            app.add_systems(
                Update,
                update_headless_frame_buffer.after(collect_readback_frames),
            );
        }

        let render_app = app.sub_app_mut(RenderApp);

        // Add render graph node
        let mut graph = render_app.world_mut().resource_mut::<RenderGraph>();
        graph.add_node(ImageCopyLabel, ImageCopyDriver);
        graph.add_node_edge(bevy::render::graph::CameraDriverLabel, ImageCopyLabel);
        debug!("[ImageCopyPlugin] Added render graph node and edge");

        render_app
            .add_systems(ExtractSchedule, image_copy_extract)
            .add_systems(
                Render,
                receive_image_from_buffer.after(RenderSystems::Render),
            );

        debug!("[ImageCopyPlugin] Setup complete");
    }
}

/// Auto-attach `ImageCopier` + `FrameReceiver` to any camera entity that has
/// `RenderTarget::Image` but no `ImageCopier` yet.
fn auto_attach_readback(
    mut commands: Commands,
    cameras: Query<(Entity, &RenderTarget), (With<Camera>, Without<ImageCopier>)>,
    images: Res<Assets<Image>>,
    render_device: Option<Res<RenderDevice>>,
) {
    let Some(render_device) = render_device else {
        return; // RenderDevice not yet available (first few frames)
    };

    for (entity, target) in cameras.iter() {
        if let RenderTarget::Image(image_target) = target {
            let handle = &image_target.handle;
            // Get image dimensions from the asset
            let Some(image) = images.get(handle) else {
                continue;
            };
            let width = image.width();
            let height = image.height();

            debug!(
                "[auto_attach_readback] Attaching readback to entity {:?} ({}x{})",
                entity, width, height
            );
            attach_image_copier(
                &mut commands,
                entity,
                handle.clone(),
                width,
                height,
                &render_device,
            );
        }
    }
}

/// Collect frames from FrameReceivers, strip row padding, and store in the
/// global frame buffer. Python receives clean W×H×4 RGBA bytes.
fn collect_readback_frames(receivers: Query<(Entity, &FrameReceiver)>) {
    let Ok(mut map) = frames_map().lock() else {
        return;
    };
    for (entity, receiver) in receivers.iter() {
        if let Some(raw) = receiver.try_recv() {
            let w = receiver.width as usize;
            let h = receiver.height as usize;
            let bpp = 4usize;
            let unpadded_row = w * bpp;
            let padded_row = RenderDevice::align_copy_bytes_per_row(unpadded_row);

            let stripped = if padded_row == unpadded_row {
                raw
            } else {
                // Strip wgpu 256-byte row alignment padding
                let mut out = Vec::with_capacity(unpadded_row * h);
                for row in 0..h {
                    let start = row * padded_row;
                    let end = start + unpadded_row;
                    if end <= raw.len() {
                        out.extend_from_slice(&raw[start..end]);
                    }
                }
                out
            };
            // Also store in LATEST_FRAME for the headless frame buffer system
            if let Ok(mut latest) = latest_frame_slot().lock() {
                *latest = Some((stripped.clone(), receiver.width, receiver.height));
            }
            map.insert(entity.to_bits(), stripped);
        }
    }
}

/// System that copies the latest readback frame into the HeadlessFrameBuffer resource.
/// Runs after collect_readback_frames in Update, so the control server's screenshot
/// handler (which runs in Last) always sees the current frame.
#[cfg(feature = "mcp")]
fn update_headless_frame_buffer(
    mut buffer: ResMut<pybevy_control::handlers::screenshot::HeadlessFrameBuffer>,
) {
    if let Some(frame) = poll_latest_frame() {
        buffer.latest = Some(frame);
    }
}

/// Extract ImageCopiers into render world
fn image_copy_extract(mut commands: Commands, image_copiers: Extract<Query<&ImageCopier>>) {
    let count = image_copiers.iter().count();
    debug!(
        "[image_copy_extract] Extracting {} ImageCopiers to render world",
        count
    );
    commands.insert_resource(ImageCopiers(
        image_copiers.iter().cloned().collect::<Vec<ImageCopier>>(),
    ));
}

/// Render graph label
#[derive(Debug, PartialEq, Eq, Clone, Hash, RenderLabel)]
struct ImageCopyLabel;

/// Render graph node that copies GPU texture to CPU buffer
#[derive(Default)]
struct ImageCopyDriver;

impl render_graph::Node for ImageCopyDriver {
    fn run(
        &self,
        _graph: &mut RenderGraphContext,
        render_context: &mut RenderContext,
        world: &World,
    ) -> Result<(), NodeRunError> {
        debug!("[ImageCopyDriver] run() called - starting texture copy");

        // SAFETY: ImageCopiers is inserted by our plugin; RenderAssets<GpuImage> is
        // inserted by Bevy's render pipeline. Both are guaranteed present when this
        // render graph node runs.
        let image_copiers = world.get_resource::<ImageCopiers>().unwrap();
        let gpu_images = world.get_resource::<RenderAssets<GpuImage>>().unwrap();

        debug!(
            "[ImageCopyDriver] Found {} ImageCopiers",
            image_copiers.len()
        );

        for image_copier in image_copiers.iter() {
            if !image_copier.enabled() {
                debug!("[ImageCopyDriver] Skipping disabled ImageCopier");
                continue;
            }

            debug!("[ImageCopyDriver] Copying texture to buffer");
            let Some(src_image) = gpu_images.get(&image_copier.src_image) else {
                debug!("[ImageCopyDriver] Source image not yet loaded, skipping");
                continue;
            };

            let mut encoder = render_context
                .render_device()
                .create_command_encoder(&CommandEncoderDescriptor::default());

            let block_dimensions = src_image.texture_format.block_dimensions();
            let Some(block_size) = src_image.texture_format.block_copy_size(None) else {
                debug!("[ImageCopyDriver] Unsupported texture format, skipping");
                continue;
            };

            // Calculate padded bytes per row (wgpu alignment)
            let padded_bytes_per_row = RenderDevice::align_copy_bytes_per_row(
                (src_image.size.width as usize / block_dimensions.0 as usize) * block_size as usize,
            );

            let Some(bytes_per_row) = std::num::NonZero::<u32>::new(padded_bytes_per_row as u32)
            else {
                debug!("[ImageCopyDriver] Zero bytes per row, skipping");
                continue;
            };

            encoder.copy_texture_to_buffer(
                src_image.texture.as_image_copy(),
                TexelCopyBufferInfo {
                    buffer: &image_copier.buffer,
                    layout: TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row.into()),
                        rows_per_image: None,
                    },
                },
                src_image.size,
            );

            // SAFETY: RenderQueue is always present when the render graph runs.
            let render_queue = world.get_resource::<RenderQueue>().unwrap();
            render_queue.submit(std::iter::once(encoder.finish()));
        }

        Ok(())
    }
}

/// System that runs after rendering to read pixels and send via channel
fn receive_image_from_buffer(image_copiers: Res<ImageCopiers>, render_device: Res<RenderDevice>) {
    debug!(
        "[receive_image_from_buffer] Called with {} copiers",
        image_copiers.0.len()
    );

    for image_copier in image_copiers.0.iter() {
        if !image_copier.enabled() {
            debug!("[receive_image_from_buffer] Skipping disabled copier");
            continue;
        }

        debug!("[receive_image_from_buffer] Reading buffer and sending to channel");
        let buffer_slice = image_copier.buffer.slice(..);

        // Channel for buffer mapping completion
        let (s, r) = crossbeam_channel::bounded(1);

        // Map buffer asynchronously
        buffer_slice.map_async(MapMode::Read, move |result| match result {
            Ok(r) => s.send(r).expect("Failed to send map update"),
            Err(err) => panic!("Failed to map buffer: {err}"),
        });

        // Poll device to complete the mapping (blocks on native, non-blocking on Web)
        render_device
            .poll(PollType::wait_indefinitely())
            .expect("Failed to poll device for map_async");

        // Wait for mapping to complete
        r.recv().expect("Failed to receive map_async message");

        // Send pixel data to this copier's specific channel
        // Ignore errors (can happen during app shutdown)
        let _ = image_copier
            .sender
            .send(buffer_slice.get_mapped_range().to_vec());

        // Unmap so buffer can be reused next frame
        image_copier.buffer.unmap();
    }
}
