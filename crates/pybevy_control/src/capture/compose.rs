use std::io::Cursor;

use base64::{Engine, engine::general_purpose::STANDARD};
use image::{
    ImageFormat, Rgb, RgbImage,
    imageops::{FilterType, overlay, resize},
};

use super::timeline::ActiveTimeline;
use crate::protocol::ControlError;

/// How a contact sheet chooses its column count.
pub(super) enum ColumnPolicy {
    /// Exactly the requested columns.
    Exact,
    /// At most the requested columns, shrunk to a divisor of the frame count
    /// when one exists so the last row is full; never below two columns.
    FillRows,
}

/// Layout and colors of a contact sheet.
pub(super) struct SheetStyle {
    pub(super) name: &'static str,
    pub(super) empty_error: &'static str,
    pub(super) bar_height: u32,
    pub(super) background: Rgb<u8>,
    pub(super) bar_saturation: f64,
    pub(super) bar_value: f64,
    pub(super) columns: ColumnPolicy,
}

pub(super) const TIMELINE_SHEET: SheetStyle = SheetStyle {
    name: "timeline",
    empty_error: "No captures collected",
    bar_height: 4,
    background: Rgb([40, 40, 40]),
    bar_saturation: 0.8,
    bar_value: 0.9,
    columns: ColumnPolicy::Exact,
};

pub(super) const TURNAROUND_SHEET: SheetStyle = SheetStyle {
    name: "turnaround",
    empty_error: "No turnaround captures collected",
    bar_height: 20,
    background: Rgb([30, 30, 30]),
    bar_saturation: 0.7,
    bar_value: 0.85,
    columns: ColumnPolicy::FillRows,
};

/// A composed contact sheet as a base64 PNG.
pub(super) struct ContactSheet {
    pub(super) image: String,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) frame_count: usize,
}

/// Composes frames, sorted by their index, into a grid with a hue-cycled bar
/// under each frame, scaled down to `max_width`.
pub(super) fn compose_contact_sheet<K: Ord + Copy>(
    frames: &mut [(K, RgbImage)],
    style: &SheetStyle,
    columns: u32,
    max_width: Option<u32>,
) -> Result<ContactSheet, ControlError> {
    frames.sort_by_key(|(index, _)| *index);
    let count = frames.len();
    if count == 0 {
        return Err(ControlError::internal(style.empty_error));
    }
    let cols = match style.columns {
        ColumnPolicy::Exact => columns.max(1) as usize,
        ColumnPolicy::FillRows => {
            let preferred = (columns.max(1) as usize).min(count);
            (preferred.min(2)..=preferred)
                .rev()
                .find(|&cols| count.is_multiple_of(cols))
                .unwrap_or(preferred)
        }
    };
    let rows = count.div_ceil(cols);

    let cell_w = frames[0].1.width();
    let cell_h = frames[0].1.height();
    let bar_height = style.bar_height;
    let canvas_w = cell_w * cols as u32;
    let canvas_h = (cell_h + bar_height) * rows as u32;
    let mut canvas = RgbImage::from_pixel(canvas_w, canvas_h, style.background);

    for (i, (_, frame)) in frames.iter().enumerate() {
        let x = (i % cols) as u32 * cell_w;
        let y = (i / cols) as u32 * (cell_h + bar_height);

        let resized;
        let src = if frame.width() != cell_w || frame.height() != cell_h {
            resized = resize(frame, cell_w, cell_h, FilterType::Triangle);
            &resized
        } else {
            frame
        };
        overlay(&mut canvas, src, x as i64, y as i64);

        let hue = (i as f64 / count as f64) * 300.0;
        let color = hsv_to_rgb(hue, style.bar_saturation, style.bar_value);
        let bar_y = y + cell_h;
        for bx in x..x + cell_w {
            for by in bar_y..bar_y + bar_height {
                if bx < canvas_w && by < canvas_h {
                    canvas.put_pixel(bx, by, color);
                }
            }
        }
    }

    if let Some(max_w) = max_width
        && canvas.width() > max_w
    {
        let scale = max_w as f64 / canvas.width() as f64;
        let new_height = (canvas.height() as f64 * scale).round() as u32;
        canvas = resize(&canvas, max_w, new_height, FilterType::Triangle);
    }

    let mut buf = Vec::new();
    canvas
        .write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
        .map_err(|e| ControlError::internal(format!("Failed to encode {} PNG: {e}", style.name)))?;
    Ok(ContactSheet {
        image: STANDARD.encode(&buf),
        width: canvas.width(),
        height: canvas.height(),
        frame_count: count,
    })
}

/// Composite timeline captures into a contact sheet grid image.
pub(super) fn composite_contact_sheet(
    timeline: &mut ActiveTimeline,
) -> Result<serde_json::Value, ControlError> {
    let sheet = compose_contact_sheet(
        &mut timeline.collected,
        &TIMELINE_SHEET,
        timeline.columns,
        timeline.max_width,
    )?;
    Ok(serde_json::json!({
        "image": sheet.image,
        "width": sheet.width,
        "height": sheet.height,
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
            hold: None,
            schedule: VecDeque::new(),
            total_captures: 1,
            next_capture_index: 0,
            collected: vec![(0, img)],
            hide_ui: true,
            with_gizmos: false,
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
            hold: None,
            schedule: VecDeque::new(),
            total_captures: 4,
            next_capture_index: 0,
            collected,
            hide_ui: true,
            with_gizmos: false,
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
            hold: None,
            schedule: VecDeque::new(),
            total_captures: 0,
            next_capture_index: 0,
            collected: vec![],
            hide_ui: true,
            with_gizmos: false,
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
            hold: None,
            schedule: VecDeque::new(),
            total_captures: 1,
            next_capture_index: 0,
            collected: vec![(0, img)],
            hide_ui: true,
            with_gizmos: false,
            headless_sequence: None,
            stall_frames: 0,
        };

        let result = composite_contact_sheet(&mut timeline).unwrap();
        assert_eq!(result["width"], 50);
    }
}
