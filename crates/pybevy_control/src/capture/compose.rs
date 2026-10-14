use std::io::Cursor;

use base64::Engine;
use bevy::prelude::*;
use image::{ImageFormat, Rgb, RgbImage};

use super::timeline::ActiveTimeline;
use crate::protocol::ControlError;

/// Composite timeline captures into a contact sheet grid image.
pub(super) fn composite_contact_sheet(
    timeline: &mut ActiveTimeline,
) -> Result<serde_json::Value, ControlError> {
    // Sort by capture index
    timeline.collected.sort_by_key(|(idx, _)| *idx);

    let cols = timeline.columns.max(1) as usize;
    let count = timeline.collected.len();
    let rows = count.div_ceil(cols);

    if count == 0 {
        return Err(ControlError::internal("No captures collected"));
    }

    // Use the first image's dimensions as cell size
    let cell_w = timeline.collected[0].1.width();
    let cell_h = timeline.collected[0].1.height();
    let bar_height = 4u32;

    let canvas_w = cell_w * cols as u32;
    let canvas_h = (cell_h + bar_height) * rows as u32;

    // Dark background
    let mut canvas = RgbImage::from_pixel(canvas_w, canvas_h, Rgb([40, 40, 40]));

    for (i, (_, frame)) in timeline.collected.iter().enumerate() {
        let col = (i % cols) as u32;
        let row = (i / cols) as u32;
        let x = col * cell_w;
        let y = row * (cell_h + bar_height);

        // Resize frame to cell size if needed
        let resized;
        let src = if frame.width() != cell_w || frame.height() != cell_h {
            resized = image::imageops::resize(
                frame,
                cell_w,
                cell_h,
                image::imageops::FilterType::Triangle,
            );
            &resized
        } else {
            frame
        };

        image::imageops::overlay(&mut canvas, src, x as i64, y as i64);

        // Colored bar at bottom (hue cycling for visual ordering)
        let hue = (i as f64 / count.max(1) as f64) * 300.0; // 0..300 degrees
        let color = hsv_to_rgb(hue, 0.8, 0.9);
        let bar_y = y + cell_h;
        for bx in x..x + cell_w {
            for by in bar_y..bar_y + bar_height {
                if bx < canvas_w && by < canvas_h {
                    canvas.put_pixel(bx, by, color);
                }
            }
        }
    }

    // Resize if max_width set
    if let Some(max_w) = timeline.max_width
        && canvas.width() > max_w
    {
        let scale = max_w as f64 / canvas.width() as f64;
        let new_height = (canvas.height() as f64 * scale).round() as u32;
        canvas = image::imageops::resize(
            &canvas,
            max_w,
            new_height,
            image::imageops::FilterType::Triangle,
        );
    }

    let width = canvas.width();
    let height = canvas.height();

    let mut buf = Vec::new();
    canvas
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| ControlError::internal(format!("Failed to encode timeline PNG: {e}")))?;

    let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);

    Ok(serde_json::json!({
        "image": b64,
        "width": width,
        "height": height,
        "format": "png",
        "encoding": "base64"
    }))
}

/// Convert HSV (h: 0..360, s: 0..1, v: 0..1) to RGB.
pub(super) fn hsv_to_rgb(h: f64, s: f64, v: f64) -> Rgb<u8> {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Rgb([
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ])
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use image::{Rgb, RgbImage};
    use tokio::sync::oneshot;

    use super::*;
    use crate::capture::hold::GizmoEnabledRestore;

    #[test]
    fn hsv_to_rgb_red() {
        assert_eq!(hsv_to_rgb(0.0, 1.0, 1.0), Rgb([255, 0, 0]));
    }

    #[test]
    fn hsv_to_rgb_green() {
        assert_eq!(hsv_to_rgb(120.0, 1.0, 1.0), Rgb([0, 255, 0]));
    }

    #[test]
    fn hsv_to_rgb_blue() {
        assert_eq!(hsv_to_rgb(240.0, 1.0, 1.0), Rgb([0, 0, 255]));
    }

    #[test]
    fn hsv_to_rgb_white() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 1.0), Rgb([255, 255, 255]));
    }

    #[test]
    fn hsv_to_rgb_black() {
        assert_eq!(hsv_to_rgb(0.0, 0.0, 0.0), Rgb([0, 0, 0]));
    }

    #[test]
    fn hsv_to_rgb_yellow() {
        assert_eq!(hsv_to_rgb(60.0, 1.0, 1.0), Rgb([255, 255, 0]));
    }

    #[test]
    fn hsv_to_rgb_cyan() {
        assert_eq!(hsv_to_rgb(180.0, 1.0, 1.0), Rgb([0, 255, 255]));
    }

    #[test]
    fn hsv_to_rgb_gray() {
        let result = hsv_to_rgb(0.0, 0.0, 0.5);
        // 0.5 * 255 = 127.5, rounds to 128
        assert!((result.0[0] as i32 - 128).abs() <= 1);
        assert!((result.0[1] as i32 - 128).abs() <= 1);
        assert!((result.0[2] as i32 - 128).abs() <= 1);
    }

    #[test]
    fn hsv_to_rgb_half_saturation() {
        let color = hsv_to_rgb(0.0, 0.5, 1.0);
        assert_eq!(color.0[0], 255);
        assert!((color.0[1] as i16 - 128).abs() <= 1);
        assert!((color.0[2] as i16 - 128).abs() <= 1);
    }

    #[test]
    fn composite_contact_sheet_single_frame() {
        let img = RgbImage::from_pixel(4, 4, Rgb([100, 100, 100]));
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            debug_camera: None,
            response_tx: Some(tx),
            max_width: None,
            columns: 3,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 1,
            next_capture_index: 0,
            collected: vec![(0, img)],
            overlay_suppressed: false,
            hide_ui: true,
            with_gizmos: false,
            ui_restore: None,
            gizmo_restore: GizmoEnabledRestore::new(),
            headless_sequence: None,
            stall_frames: 0,
        };

        let result = composite_contact_sheet(&mut timeline);
        assert!(result.is_ok());
        let val = result.unwrap();
        assert!(val.get("image").is_some());
        assert!(!val["image"].as_str().unwrap().is_empty());
        assert!(val.get("width").is_some());
        assert!(val.get("height").is_some());
        assert_eq!(val["format"], "png");
    }

    #[test]
    fn composite_contact_sheet_grid_layout() {
        let mut collected = Vec::new();
        for i in 0..4u32 {
            collected.push((i, RgbImage::from_pixel(4, 4, Rgb([100, 100, 100]))));
        }
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            debug_camera: None,
            response_tx: Some(tx),
            max_width: None,
            columns: 2,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 4,
            next_capture_index: 0,
            collected,
            overlay_suppressed: false,
            hide_ui: true,
            with_gizmos: false,
            ui_restore: None,
            gizmo_restore: GizmoEnabledRestore::new(),
            headless_sequence: None,
            stall_frames: 0,
        };

        let result = composite_contact_sheet(&mut timeline).unwrap();
        let bar_height = 4u32;
        // 2 columns, 4 images → 2 rows
        assert_eq!(result["width"], 2 * 4); // cols * cell_w
        assert_eq!(result["height"], (4 + bar_height) * 2); // (cell_h + bar) * rows
    }

    #[test]
    fn composite_contact_sheet_empty_captures() {
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            debug_camera: None,
            response_tx: Some(tx),
            max_width: None,
            columns: 3,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 0,
            next_capture_index: 0,
            collected: vec![],
            overlay_suppressed: false,
            hide_ui: true,
            with_gizmos: false,
            ui_restore: None,
            gizmo_restore: GizmoEnabledRestore::new(),
            headless_sequence: None,
            stall_frames: 0,
        };

        let result = composite_contact_sheet(&mut timeline);
        assert!(result.is_err());
    }

    #[test]
    fn composite_contact_sheet_max_width_resize() {
        let img = RgbImage::from_pixel(100, 100, Rgb([100, 100, 100]));
        let (tx, _rx) = oneshot::channel();
        let mut timeline = ActiveTimeline {
            debug_camera: None,
            response_tx: Some(tx),
            max_width: Some(50),
            columns: 3,
            debug_cleanup: None,
            schedule: VecDeque::new(),
            total_captures: 1,
            next_capture_index: 0,
            collected: vec![(0, img)],
            overlay_suppressed: false,
            hide_ui: true,
            with_gizmos: false,
            ui_restore: None,
            gizmo_restore: GizmoEnabledRestore::new(),
            headless_sequence: None,
            stall_frames: 0,
        };

        let result = composite_contact_sheet(&mut timeline).unwrap();
        assert_eq!(result["width"], 50);
    }
}
