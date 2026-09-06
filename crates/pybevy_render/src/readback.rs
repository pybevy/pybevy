use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

/// Global frame buffer: entity bits → latest frame bytes.
/// Written by Rust `collect_readback_frames` system, read by Python `poll_readback_frame`.
static READBACK_FRAMES: OnceLock<Mutex<HashMap<u64, Arc<Vec<u8>>>>> = OnceLock::new();

fn frames_map() -> &'static Mutex<HashMap<u64, Arc<Vec<u8>>>> {
    READBACK_FRAMES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Latest frame with dimensions, for headless screenshot support.
/// Written by `collect_readback_frames`, read by `HeadlessFrameProvider`.
static LATEST_FRAME: OnceLock<Mutex<Option<(Arc<Vec<u8>>, u32, u32, u64)>>> = OnceLock::new();
static LATEST_FRAME_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn latest_frame_slot() -> &'static Mutex<Option<(Arc<Vec<u8>>, u32, u32, u64)>> {
    LATEST_FRAME.get_or_init(|| Mutex::new(None))
}

/// Poll the latest readback frame with dimensions. Used by the control server
/// for headless screenshots.
pub fn poll_latest_frame() -> Option<(Vec<u8>, u32, u32)> {
    let (frame, width, height, _) = latest_frame_slot().lock().ok()?.clone()?;
    Some((unwrap_frame(frame), width, height))
}

/// Poll the latest readback frame and the sequence assigned when it arrived.
pub fn poll_latest_frame_with_sequence() -> Option<(Vec<u8>, u32, u32, u64)> {
    let (frame, width, height, sequence) = latest_frame_slot().lock().ok()?.clone()?;
    Some((unwrap_frame(frame), width, height, sequence))
}

/// Take the bytes without copying when this is the last reference.
fn unwrap_frame(frame: Arc<Vec<u8>>) -> Vec<u8> {
    Arc::try_unwrap(frame).unwrap_or_else(|shared| shared.as_ref().clone())
}

/// Called from Python to get the latest readback frame for a camera entity.
/// Returns None if no frame is available yet.
pub fn poll_frame(entity_bits: u64) -> Option<Vec<u8>> {
    let mut map = frames_map().lock().ok()?;
    map.remove(&entity_bits).map(unwrap_frame)
}

/// List all entity bits that currently have readback frames available.
pub fn list_entities() -> Vec<u64> {
    let map = frames_map().lock().unwrap_or_else(|e| e.into_inner());
    map.keys().copied().collect()
}

// GPU Readback System for JupyBevy
//
// Adapted from Bevy's headless_renderer example.
// Architecture:
// 1. Render to texture (Camera → GPU Image)
// 2. Copy texture to buffer (ImageCopyDriver node)
// 3. Map buffer and read pixels (RenderWorld)
// 4. Send via channel to MainWorld
// 5. Python extracts pixels via HeadlessRenderer.extract_frame()
use bevy::{
    camera::RenderTarget,
    prelude::*,
    render::{
        Extract, Render, RenderApp, RenderSystems,
        render_asset::RenderAssets,
        render_resource::{
            Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, MapMode, PollType,
            TexelCopyBufferInfo, TexelCopyBufferLayout, TextureUsages,
        },
        renderer::{RenderContext, RenderDevice, RenderGraph, RenderQueue},
        texture::GpuImage,
    },
};
use crossbeam_channel::{Receiver, Sender};
use tracing::{debug, error, warn};

/// Component on camera entities: receives pixel data from render world
/// Attached to cameras with RenderToBuffer component to enable frame extraction
#[derive(Component, Clone)]
pub struct FrameReceiver {
    pub(crate) receiver: Receiver<Vec<u8>>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bytes_per_pixel: u32,
}

impl FrameReceiver {
    pub fn new(width: u32, height: u32) -> (Self, Sender<Vec<u8>>) {
        Self::with_bytes_per_pixel(width, height, 4)
    }

    pub fn with_bytes_per_pixel(
        width: u32,
        height: u32,
        bytes_per_pixel: u32,
    ) -> (Self, Sender<Vec<u8>>) {
        let (sender, receiver) = crossbeam_channel::unbounded();
        (
            FrameReceiver {
                receiver,
                bytes_per_pixel,
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
    source_entity: Option<Entity>,
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
        Self::with_bytes_per_pixel(src_image, width, height, 4, render_device, sender)
    }

    pub fn with_bytes_per_pixel(
        src_image: Handle<Image>,
        width: u32,
        height: u32,
        bytes_per_pixel: u32,
        render_device: &RenderDevice,
        sender: Sender<Vec<u8>>,
    ) -> Self {
        let padded_bytes_per_row =
            RenderDevice::align_copy_bytes_per_row((width * bytes_per_pixel) as usize);

        let buffer = render_device.create_buffer(&BufferDescriptor {
            label: Some("readback_buffer"),
            size: padded_bytes_per_row as u64 * height as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        ImageCopier {
            buffer,
            src_image,
            source_entity: None,
            enabled: Arc::new(AtomicBool::new(true)),
            sender,
        }
    }

    /// Associate the copier with the camera that owns it for diagnostics.
    pub fn with_source_entity(mut self, entity: Entity) -> Self {
        self.source_entity = Some(entity);
        self
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn disable(&self) -> bool {
        self.enabled.swap(false, Ordering::AcqRel)
    }
}

/// Attach ImageCopier and FrameReceiver to an entity (Rust-native version)
///
/// This is the core implementation used by both Rust and Python bindings.
/// Call this after app initialization when RenderDevice is available.
pub fn attach_image_copier(
    commands: &mut Commands,
    entity: Entity,
    image_handle: Handle<Image>,
    width: u32,
    height: u32,
    bytes_per_pixel: u32,
    render_device: &RenderDevice,
) -> FrameReceiver {
    let (frame_receiver, sender) =
        FrameReceiver::with_bytes_per_pixel(width, height, bytes_per_pixel);

    let copier = ImageCopier::with_bytes_per_pixel(
        image_handle,
        width,
        height,
        bytes_per_pixel,
        render_device,
        sender,
    )
    .with_source_entity(entity);

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
    fn build(&self, _app: &mut App) {
        debug!("[ImageCopyPlugin] build() called - readback wiring deferred to finish()");
    }

    // All wiring happens in finish(), which Bevy runs after every plugin's
    // build(): a RenderPlugin added later in the plugin list (ImageCopyPlugin
    // before DefaultPlugins) has created the RenderApp by then, so plugin
    // order cannot silently disable readback.
    fn finish(&self, app: &mut App) {
        // A scene on MinimalPlugins, or with RenderPlugin disabled, has no
        // RenderApp and no `Assets<Image>`. Readback cannot work there, so add
        // nothing at all rather than panicking or leaving systems that will.
        if app.get_sub_app(RenderApp).is_none() {
            warn!("[ImageCopyPlugin] no RenderApp: GPU readback and screenshots are unavailable");
            return;
        }

        // Main-world systems:
        // 1. Auto-attach readback to render-target cameras
        // 2. Collect frames from FrameReceivers into global buffer for Python
        app.add_systems(Update, (auto_attach_readback, collect_readback_frames));

        let render_app = app.sub_app_mut(RenderApp);

        // Render passes are systems; the image-copy pass runs in the top-level
        // `RenderGraph` schedule.
        render_app.add_systems(RenderGraph, image_copy_driver);
        debug!("[ImageCopyPlugin] Added image copy driver system");

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
            let format = image.texture_descriptor.format;
            // The copy buffer and the row-padding strip must both use the real
            // texel size; a block-compressed or planar target has none.
            let Some(bytes_per_pixel) = format
                .block_copy_size(None)
                .filter(|_| format.block_dimensions() == (1, 1))
            else {
                warn!(
                    "[auto_attach_readback] Skipping entity {:?}: render target format {:?} \
                     has no single texel size, so its frames cannot be read back",
                    entity, format
                );
                continue;
            };

            debug!(
                "[auto_attach_readback] Attaching readback to entity {:?} ({}x{}, {} bytes/pixel)",
                entity, width, height, bytes_per_pixel
            );
            attach_image_copier(
                &mut commands,
                entity,
                handle.clone(),
                width,
                height,
                bytes_per_pixel,
                &render_device,
            );
        }
    }
}

/// Collect frames from FrameReceivers, strip row padding, and store in the
/// global frame buffer. Python receives clean W×H×4 RGBA bytes.
pub fn collect_readback_frames(receivers: Query<(Entity, &FrameReceiver)>) {
    let Ok(mut map) = frames_map().lock() else {
        return;
    };
    if !map.is_empty() {
        let live: HashSet<u64> = receivers
            .iter()
            .map(|(entity, _)| entity.to_bits())
            .collect();
        map.retain(|entity_bits, _| live.contains(entity_bits));
    }
    let mut headline: Option<(u64, Arc<Vec<u8>>, u32, u32)> = None;
    for (entity, receiver) in receivers.iter() {
        if let Some(raw) = receiver.try_recv() {
            let w = receiver.width as usize;
            let h = receiver.height as usize;
            let unpadded_row = w * receiver.bytes_per_pixel as usize;
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
            let stripped = Arc::new(stripped);
            let bits = entity.to_bits();
            if headline
                .as_ref()
                .is_none_or(|(current, ..)| bits < *current)
            {
                headline = Some((bits, Arc::clone(&stripped), receiver.width, receiver.height));
            }
            map.insert(bits, stripped);
        }
    }

    if let Some((_, frame, width, height)) = headline
        && let Ok(mut latest) = latest_frame_slot().lock()
    {
        let sequence = LATEST_FRAME_SEQUENCE.fetch_add(1, Ordering::AcqRel) + 1;
        *latest = Some((frame, width, height, sequence));
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

/// Render-graph-schedule system that copies a GPU texture to a CPU buffer.
///
/// Render passes are systems; this runs in the top-level `RenderGraph` schedule.
fn image_copy_driver(
    render_context: RenderContext,
    image_copiers: Res<ImageCopiers>,
    render_queue: Res<RenderQueue>,
    gpu_images: Res<RenderAssets<GpuImage>>,
) {
    debug!(
        "[image_copy_driver] called - found {} ImageCopiers",
        image_copiers.len()
    );

    for image_copier in image_copiers.iter() {
        if !image_copier.enabled() {
            debug!("[image_copy_driver] Skipping disabled ImageCopier");
            continue;
        }

        debug!("[image_copy_driver] Copying texture to buffer");
        let Some(src_image) = gpu_images.get(&image_copier.src_image) else {
            debug!("[image_copy_driver] Source image not yet loaded, skipping");
            continue;
        };

        let usage = src_image.texture_descriptor.usage;
        if !usage.contains(TextureUsages::COPY_SRC) {
            if image_copier.disable() {
                let image_name = image_copier
                    .src_image
                    .path()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| format!("{:?}", image_copier.src_image.id()));
                let camera_name = image_copier
                    .source_entity
                    .map(|entity| format!("{entity:?}"))
                    .unwrap_or_else(|| "unknown".to_string());
                error!(
                    "[image_copy_driver] GPU readback disabled for camera {camera_name}: image \
                     {image_name} has texture usage {usage:?}, missing required texture usage \
                     COPY_SRC; use Image.new_render_target() for camera targets that need readback"
                );
            }
            continue;
        }

        let mut encoder = render_context
            .render_device()
            .create_command_encoder(&CommandEncoderDescriptor::default());

        let block_dimensions = src_image.texture_descriptor.format.block_dimensions();
        let Some(block_size) = src_image.texture_descriptor.format.block_copy_size(None) else {
            debug!("[image_copy_driver] Unsupported texture format, skipping");
            continue;
        };

        // Calculate padded bytes per row (wgpu alignment)
        let padded_bytes_per_row = RenderDevice::align_copy_bytes_per_row(
            (src_image.texture_descriptor.size.width as usize / block_dimensions.0 as usize)
                * block_size as usize,
        );

        let Some(bytes_per_row) = std::num::NonZero::<u32>::new(padded_bytes_per_row as u32) else {
            debug!("[image_copy_driver] Zero bytes per row, skipping");
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
            src_image.texture_descriptor.size,
        );

        render_queue.submit(std::iter::once(encoder.finish()));
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
