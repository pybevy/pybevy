use bevy::{ecs::world::World, prelude::*};
use image::{Rgb, RgbImage};
use pybevy_ecs::shared::system_runtime::HotReloadGeneration;

use super::screenshot::complete_rgb_capture;
use crate::{
    capture::{analysis::CapturedFrames, queue::CaptureResponseKind},
    protocol::ControlError,
};

/// Resource storing the latest GPU readback frame for headless screenshots.
///
/// Updated each frame by a system registered by `ImageCopyPlugin`.
/// Read by screenshot/timeline/turnaround handlers when no primary window exists.
#[derive(Resource, Default)]
pub struct HeadlessFrameBuffer {
    pub latest: Option<(Vec<u8>, u32, u32)>,
    pub sequence: u64,
}

impl HeadlessFrameBuffer {
    /// Mirror the renderer's latest readback frame, dropping it once no camera produces one.
    pub fn sync(&mut self, latest: Option<(Vec<u8>, u32, u32, u64)>) {
        match latest {
            Some((bytes, width, height, sequence)) if sequence > self.sequence => {
                self.latest = Some((bytes, width, height));
                self.sequence = sequence;
            }
            Some(_) => {}
            None => self.latest = None,
        }
    }
}

pub(crate) fn headless_frame_sequence(world: &World) -> Option<u64> {
    world
        .get_resource::<HeadlessFrameBuffer>()
        .and_then(|buffer| buffer.latest.as_ref().map(|_| buffer.sequence))
}

/// Read the latest frame from `HeadlessFrameBuffer` and convert to an RgbImage.
pub fn read_headless_frame(world: &World) -> Result<RgbImage, ControlError> {
    let buffer = world.get_resource::<HeadlessFrameBuffer>().ok_or_else(|| {
        ControlError::internal(
            "No primary window and no HeadlessFrameBuffer registered. \
             Add ImageCopyPlugin and a camera with RenderTarget.Image(ImageRenderTarget(...)) for headless screenshots."
                .to_string(),
        )
    })?;

    let (rgba_bytes, w, h) = buffer.latest.as_ref().ok_or_else(|| {
        ControlError::internal(
            "HeadlessFrameBuffer has no frame yet. \
             The render pipeline may not have produced a frame yet."
                .to_string(),
        )
    })?;

    let mut rgb = RgbImage::new(*w, *h);
    for y in 0..*h {
        for x in 0..*w {
            let i = ((y * *w + x) * 4) as usize;
            if i + 2 < rgba_bytes.len() {
                rgb.put_pixel(
                    x,
                    y,
                    Rgb([rgba_bytes[i], rgba_bytes[i + 1], rgba_bytes[i + 2]]),
                );
            }
        }
    }

    Ok(rgb)
}

/// Capture a frame from the headless readback buffer and encode it as a screenshot.
pub(super) fn capture_headless_frame(
    world: &mut World,
    max_width: Option<u32>,
    response_kind: CaptureResponseKind,
) -> Result<serde_json::Value, ControlError> {
    let rgb = read_headless_frame(world)?;
    let hot_reload_generation = world
        .get_resource::<HotReloadGeneration>()
        .map(|generation| generation.current);
    let mut captured_frames = world.get_resource_or_insert_with(CapturedFrames::default);
    complete_rgb_capture(
        rgb,
        max_width,
        response_kind,
        &mut captured_frames,
        hot_reload_generation,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{analysis::FrameStatsOptions, queue::CaptureResponseKind};

    #[test]
    fn capture_headless_frame_no_buffer() {
        let mut world = World::new();
        let result = capture_headless_frame(&mut world, None, CaptureResponseKind::Screenshot);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("HeadlessFrameBuffer"));
    }

    #[test]
    fn capture_headless_frame_no_frame_available() {
        let mut world = World::new();
        world.insert_resource(HeadlessFrameBuffer::default());
        let result = capture_headless_frame(&mut world, None, CaptureResponseKind::Screenshot);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("no frame"));
    }

    #[test]
    fn capture_headless_frame_success() {
        let mut world = World::new();
        let rgba = vec![
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ];
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((rgba, 2, 2)),
            sequence: 1,
        });
        let result =
            capture_headless_frame(&mut world, None, CaptureResponseKind::Screenshot).unwrap();
        assert_eq!(result["width"], 2);
        assert_eq!(result["height"], 2);
        assert_eq!(result["format"], "png");
        assert!(result["frame_id"].as_str().is_some());
        assert_eq!(result["retained"], true);
    }

    #[test]
    fn capture_headless_stats_returns_numbers_without_png_and_retains_frame() {
        let mut world = World::new();
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![255, 0, 0, 255, 0, 255, 0, 255], 2, 1)),
            sequence: 1,
        });
        let result = capture_headless_frame(
            &mut world,
            None,
            CaptureResponseKind::Stats(FrameStatsOptions {
                grid: 1,
                region: None,
                sample_points: Some(vec![[0, 0]]),
            }),
        )
        .unwrap();

        assert_eq!(result["width"], 2);
        assert_eq!(
            result["samples"][0]["rgb"],
            serde_json::json!([1.0, 0.0, 0.0])
        );
        assert!(result.get("image").is_none());
        assert!(result["frame_id"].as_str().is_some());
        assert_eq!(result["retained"], true);
    }

    #[test]
    fn read_headless_frame_converts_rgba_to_rgb() {
        let mut world = World::new();
        // 1x1 pixel: R=100, G=200, B=50, A=255
        world.insert_resource(HeadlessFrameBuffer {
            latest: Some((vec![100, 200, 50, 255], 1, 1)),
            sequence: 1,
        });
        let rgb = read_headless_frame(&world).unwrap();
        assert_eq!(rgb.width(), 1);
        assert_eq!(rgb.height(), 1);
        let pixel = rgb.get_pixel(0, 0);
        assert_eq!(pixel.0, [100, 200, 50]);
    }

    #[test]
    fn headless_frame_buffer_sync_drops_the_frame_but_keeps_the_sequence() {
        let mut buffer = HeadlessFrameBuffer {
            latest: Some((vec![255, 0, 0, 255], 1, 1)),
            sequence: 5,
        };

        buffer.sync(Some((vec![0, 0, 255, 255], 1, 1, 5)));
        assert_eq!(buffer.latest, Some((vec![255, 0, 0, 255], 1, 1)));

        buffer.sync(None);
        assert_eq!(buffer.latest, None);
        assert_eq!(buffer.sequence, 5);

        buffer.sync(Some((vec![0, 255, 0, 255], 1, 1, 6)));
        assert_eq!(buffer.latest, Some((vec![0, 255, 0, 255], 1, 1)));
        assert_eq!(buffer.sequence, 6);
    }

    #[test]
    fn headless_frame_buffer_default_is_none() {
        let buffer = HeadlessFrameBuffer::default();
        assert!(buffer.latest.is_none());
    }
}
