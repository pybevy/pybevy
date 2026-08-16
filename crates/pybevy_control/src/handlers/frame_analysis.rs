use std::{collections::VecDeque, sync::Arc};

use bevy::prelude::Resource;
use image::{Rgb, Rgb32FImage, RgbImage};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use crate::bridge::ControlError;

pub const MAX_STATS_GRID: u32 = 16;
pub const MAX_SAMPLE_POINTS: usize = 256;
pub const DEFAULT_COMPARE_EPSILON: f64 = 1.0 / 255.0;
pub const DEFAULT_RETAINED_FRAME_COUNT: usize = 8;
pub const DEFAULT_RETAINED_FRAME_BYTES: usize = 32 * 1024 * 1024;

const BLACK_LUMA_THRESHOLD: f64 = 0.01;
const CLIPPED_CHANNEL_THRESHOLD: f64 = 254.0 / 255.0;
const LOW_VARIANCE_THRESHOLD: f64 = 0.01;
const LUMA_BUCKETS: usize = 16;

pub(crate) fn resize_rgb_image_linear(rgb: RgbImage, max_width: Option<u32>) -> RgbImage {
    let Some(max_width) = max_width.filter(|max_width| rgb.width() > *max_width) else {
        return rgb;
    };
    let scale = f64::from(max_width) / f64::from(rgb.width());
    let new_height = ((f64::from(rgb.height()) * scale).round() as u32).max(1);
    let linear = Rgb32FImage::from_fn(rgb.width(), rgb.height(), |x, y| {
        let channels = rgb.get_pixel(x, y).0;
        Rgb(channels.map(|channel| srgb_to_linear(f64::from(channel) / 255.0) as f32))
    });
    let resized = image::imageops::resize(
        &linear,
        max_width,
        new_height,
        image::imageops::FilterType::Triangle,
    );
    RgbImage::from_fn(max_width, new_height, |x, y| {
        Rgb(resized.get_pixel(x, y).0.map(linear_to_srgb_u8))
    })
}

fn linear_to_srgb_u8(channel: f32) -> u8 {
    let channel = f64::from(channel).clamp(0.0, 1.0);
    let srgb = if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round() as u8
}

#[derive(Debug, Clone)]
pub struct FrameStatsOptions {
    pub grid: u32,
    pub region: Option<[i64; 4]>,
    pub sample_points: Option<Vec<[i64; 2]>>,
}

#[derive(Debug, Clone)]
pub struct CapturedFrameMetadata {
    pub kind: &'static str,
    pub max_width: Option<u32>,
    pub hot_reload_generation: Option<u32>,
}

#[derive(Debug)]
struct CapturedFrame {
    id: String,
    image: Arc<RgbImage>,
    metadata: CapturedFrameMetadata,
    byte_len: usize,
}

#[derive(Debug)]
pub struct RetentionStatus {
    pub frame_id: Option<String>,
    pub retained: bool,
    pub reason: Option<&'static str>,
}

impl RetentionStatus {
    pub fn insert_into(&self, result: &mut Map<String, Value>) {
        result.insert("frame_id".to_string(), json!(self.frame_id));
        result.insert("retained".to_string(), json!(self.retained));
        if let Some(reason) = self.reason {
            result.insert("retention_reason".to_string(), json!(reason));
        }
    }
}

#[derive(Resource, Debug)]
pub struct CapturedFrames {
    frames: VecDeque<CapturedFrame>,
    namespace: Uuid,
    next_id: u64,
    total_bytes: usize,
    max_frames: usize,
    max_bytes: usize,
}

impl Default for CapturedFrames {
    fn default() -> Self {
        Self::with_limits(DEFAULT_RETAINED_FRAME_COUNT, DEFAULT_RETAINED_FRAME_BYTES)
    }
}

impl CapturedFrames {
    fn with_limits(max_frames: usize, max_bytes: usize) -> Self {
        Self::with_namespace(max_frames, max_bytes, Uuid::new_v4())
    }

    fn with_namespace(max_frames: usize, max_bytes: usize, namespace: Uuid) -> Self {
        Self {
            frames: VecDeque::new(),
            namespace,
            next_id: 0,
            total_bytes: 0,
            max_frames,
            max_bytes,
        }
    }

    pub fn retain(
        &mut self,
        image: Arc<RgbImage>,
        metadata: CapturedFrameMetadata,
    ) -> RetentionStatus {
        let byte_len = image.as_raw().len();
        if self.max_frames == 0 || byte_len > self.max_bytes {
            return RetentionStatus {
                frame_id: None,
                retained: false,
                reason: Some("captured frame exceeds the retention limit"),
            };
        }

        while self.frames.len() >= self.max_frames
            || self
                .total_bytes
                .checked_add(byte_len)
                .is_none_or(|total| total > self.max_bytes)
        {
            let Some(evicted) = self.frames.pop_front() else {
                break;
            };
            self.total_bytes = self.total_bytes.saturating_sub(evicted.byte_len);
        }

        let id = format!("f_{}_{:016x}", self.namespace.simple(), self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        self.total_bytes = self
            .total_bytes
            .checked_add(byte_len)
            .expect("retained frame bytes were bounded before insertion");
        self.frames.push_back(CapturedFrame {
            id: id.clone(),
            image,
            metadata,
            byte_len,
        });

        RetentionStatus {
            frame_id: Some(id),
            retained: true,
            reason: None,
        }
    }

    pub fn compare(&self, a: &str, b: &str, epsilon: f64) -> Result<Value, ControlError> {
        if !epsilon.is_finite() || !(0.0..=1.0).contains(&epsilon) {
            return Err(ControlError::invalid_params(
                "epsilon must be a finite number in [0, 1]",
            ));
        }

        let a_frame = self.find(a)?;
        let b_frame = self.find(b)?;
        if a_frame.image.dimensions() != b_frame.image.dimensions() {
            return Err(ControlError::invalid_params(format!(
                "Cannot compare frames with different dimensions: '{}' is {}x{}, '{}' is {}x{}",
                a,
                a_frame.image.width(),
                a_frame.image.height(),
                b,
                b_frame.image.width(),
                b_frame.image.height(),
            )));
        }

        let width = a_frame.image.width();
        let height = a_frame.image.height();
        let a_bytes = a_frame.image.as_raw();
        let b_bytes = b_frame.image.as_raw();
        if a_bytes.is_empty() {
            return Err(ControlError::internal(
                "Cannot compare empty captured frames",
            ));
        }

        let mut total_abs_diff = 0.0_f64;
        let mut max_diff = 0.0_f64;
        let mut changed_count = 0_u64;
        let mut min_x = u32::MAX;
        let mut min_y = u32::MAX;
        let mut max_x = 0_u32;
        let mut max_y = 0_u32;
        let mut changed_x_sum = 0.0_f64;
        let mut changed_y_sum = 0.0_f64;

        for (pixel_index, (a_pixel, b_pixel)) in a_bytes
            .chunks_exact(3)
            .zip(b_bytes.chunks_exact(3))
            .enumerate()
        {
            let mut pixel_max = 0.0_f64;
            for channel in 0..3 {
                let diff =
                    (f64::from(a_pixel[channel]) - f64::from(b_pixel[channel])).abs() / 255.0;
                total_abs_diff += diff;
                pixel_max = pixel_max.max(diff);
                max_diff = max_diff.max(diff);
            }

            if pixel_max > epsilon {
                let index = u64::try_from(pixel_index)
                    .map_err(|_| ControlError::internal("Captured frame is too large"))?;
                let x = u32::try_from(index % u64::from(width))
                    .map_err(|_| ControlError::internal("Captured frame width overflowed"))?;
                let y = u32::try_from(index / u64::from(width))
                    .map_err(|_| ControlError::internal("Captured frame height overflowed"))?;
                changed_count += 1;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                changed_x_sum += f64::from(x);
                changed_y_sum += f64::from(y);
            }
        }

        let pixel_count = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or_else(|| ControlError::internal("Captured frame dimensions overflowed"))?;
        let channel_count = pixel_count
            .checked_mul(3)
            .ok_or_else(|| ControlError::internal("Captured frame channel count overflowed"))?;
        let (changed_bbox, changed_centroid) = if changed_count == 0 {
            (Value::Null, Value::Null)
        } else {
            (
                json!([min_x, min_y, max_x - min_x + 1, max_y - min_y + 1]),
                json!([
                    changed_x_sum / changed_count as f64,
                    changed_y_sum / changed_count as f64
                ]),
            )
        };

        Ok(json!({
            "a": a,
            "b": b,
            "width": width,
            "height": height,
            "epsilon": epsilon,
            "identical": changed_count == 0,
            "mean_abs_diff": total_abs_diff / channel_count as f64,
            "max_diff": max_diff,
            "pct_pixels_changed": 100.0 * changed_count as f64 / pixel_count as f64,
            "changed_bbox": changed_bbox,
            "changed_centroid": changed_centroid,
            "a_metadata": metadata_json(&a_frame.metadata),
            "b_metadata": metadata_json(&b_frame.metadata),
        }))
    }

    fn find(&self, id: &str) -> Result<&CapturedFrame, ControlError> {
        self.frames
            .iter()
            .find(|frame| frame.id == id)
            .ok_or_else(|| ControlError::not_found(format!("Frame '{id}' not found or evicted")))
    }
}

fn metadata_json(metadata: &CapturedFrameMetadata) -> Value {
    json!({
        "kind": metadata.kind,
        "max_width": metadata.max_width,
        "hot_reload_generation": metadata.hot_reload_generation,
    })
}

#[derive(Default)]
struct PixelAccumulator {
    count: u64,
    luma_sum: f64,
    luma_square_sum: f64,
    luma_min: f64,
    luma_max: f64,
    rgb_sum: [f64; 3],
    histogram: [u64; LUMA_BUCKETS],
    black_count: u64,
    clipped_count: u64,
}

impl PixelAccumulator {
    fn add(&mut self, pixel: &[u8; 3]) {
        let rgb = normalized_rgb(pixel);
        let luma = luma(rgb);
        if self.count == 0 {
            self.luma_min = luma;
            self.luma_max = luma;
        } else {
            self.luma_min = self.luma_min.min(luma);
            self.luma_max = self.luma_max.max(luma);
        }
        self.count += 1;
        self.luma_sum += luma;
        self.luma_square_sum += luma * luma;
        for (sum, value) in self.rgb_sum.iter_mut().zip(rgb) {
            *sum += value;
        }
        let bucket = ((luma * LUMA_BUCKETS as f64).floor() as usize).min(LUMA_BUCKETS - 1);
        self.histogram[bucket] += 1;
        if luma <= BLACK_LUMA_THRESHOLD {
            self.black_count += 1;
        }
        if rgb
            .iter()
            .any(|channel| *channel >= CLIPPED_CHANNEL_THRESHOLD)
        {
            self.clipped_count += 1;
        }
    }

    fn to_json(&self, include_distribution: bool) -> Result<Value, ControlError> {
        if self.count == 0 {
            return Err(ControlError::invalid_params(
                "Analysis region and grid cells must contain at least one pixel",
            ));
        }
        let count = self.count as f64;
        let mean = self.luma_sum / count;
        let variance = (self.luma_square_sum / count - mean * mean).max(0.0);
        let mut result = json!({
            "luma_mean": mean,
            "luma_std": variance.sqrt(),
            "luma_min": self.luma_min,
            "luma_max": self.luma_max,
            "rgb_mean": [
                self.rgb_sum[0] / count,
                self.rgb_sum[1] / count,
                self.rgb_sum[2] / count,
            ],
        });
        if include_distribution {
            result["histogram"] = json!(self.histogram);
            result["pct_black"] = json!(self.black_count as f64 / count);
            result["pct_clipped"] = json!(self.clipped_count as f64 / count);
        }
        Ok(result)
    }
}

pub fn analyze_frame(image: &RgbImage, options: &FrameStatsOptions) -> Result<Value, ControlError> {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return Err(ControlError::internal(
            "Cannot analyze an empty captured frame",
        ));
    }
    validate_frame_stats_options(options)?;

    let region = validate_region(options.region, width, height)?;
    if options.grid > region[2] || options.grid > region[3] {
        return Err(ControlError::invalid_params(
            "grid must not exceed the analysis region width or height",
        ));
    }

    let overall = accumulate_region(image, region)?.to_json(true)?;
    let mut cells = Vec::new();
    for row in 0..options.grid {
        let y0 = grid_boundary(region[1], region[3], row, options.grid)?;
        let y1 = grid_boundary(region[1], region[3], row + 1, options.grid)?;
        for column in 0..options.grid {
            let x0 = grid_boundary(region[0], region[2], column, options.grid)?;
            let x1 = grid_boundary(region[0], region[2], column + 1, options.grid)?;
            let bounds = [x0, y0, x1 - x0, y1 - y0];
            let mut cell = accumulate_region(image, bounds)?.to_json(false)?;
            cell["bounds"] = json!(bounds);
            cells.push(cell);
        }
    }

    let mut samples = Vec::new();
    for (index, point) in options.sample_points.iter().flatten().enumerate() {
        let [x, y] = *point;
        if x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
            return Err(ControlError::invalid_params(format!(
                "sample_points[{index}] must be inside the captured {}x{} image (got [{x}, {y}])",
                width, height
            )));
        }
        let x = u32::try_from(x)
            .map_err(|_| ControlError::invalid_params("sample point x is out of range"))?;
        let y = u32::try_from(y)
            .map_err(|_| ControlError::invalid_params("sample point y is out of range"))?;
        let pixel = image.get_pixel(x, y).0;
        let rgb = normalized_rgb(&pixel);
        samples.push(json!({
            "point": [x, y],
            "rgb": rgb,
            "luma": luma(rgb),
        }));
    }

    let luma_std = overall["luma_std"]
        .as_f64()
        .ok_or_else(|| ControlError::internal("Frame luma_std was not numeric"))?;
    let pct_black = overall["pct_black"]
        .as_f64()
        .ok_or_else(|| ControlError::internal("Frame pct_black was not numeric"))?;
    let pct_clipped = overall["pct_clipped"]
        .as_f64()
        .ok_or_else(|| ControlError::internal("Frame pct_clipped was not numeric"))?;
    let mut health_hints = Vec::new();
    if pct_black >= 0.99 {
        health_hints.push("almost_all_black");
    }
    if pct_clipped >= 0.5 {
        health_hints.push("mostly_clipped");
    }
    if luma_std <= LOW_VARIANCE_THRESHOLD {
        health_hints.push("low_variance");
    }

    Ok(json!({
        "width": width,
        "height": height,
        "region": region,
        "color_space": "display-srgb",
        "analysis": {
            "luma_space": "linear",
            "luma_coefficients": [0.2126, 0.7152, 0.0722],
            "histogram_buckets": LUMA_BUCKETS,
            "black_luma_threshold": BLACK_LUMA_THRESHOLD,
            "clipped_channel_threshold": CLIPPED_CHANNEL_THRESHOLD,
            "low_variance_threshold": LOW_VARIANCE_THRESHOLD,
        },
        "overall": overall,
        "cells": cells,
        "samples": samples,
        "health_hints": health_hints,
    }))
}

pub fn validate_frame_stats_options(options: &FrameStatsOptions) -> Result<(), ControlError> {
    if !(1..=MAX_STATS_GRID).contains(&options.grid) {
        return Err(ControlError::invalid_params(format!(
            "grid must be between 1 and {MAX_STATS_GRID}"
        )));
    }
    if options
        .sample_points
        .as_ref()
        .is_some_and(|points| points.len() > MAX_SAMPLE_POINTS)
    {
        return Err(ControlError::invalid_params(format!(
            "sample_points must contain at most {MAX_SAMPLE_POINTS} entries"
        )));
    }
    if let Some([x, y, width, height]) = options.region
        && (x < 0 || y < 0 || width <= 0 || height <= 0)
    {
        return Err(ControlError::invalid_params(
            "region must be [x, y, width, height] with non-negative x/y and positive width/height",
        ));
    }
    Ok(())
}

fn validate_region(
    region: Option<[i64; 4]>,
    width: u32,
    height: u32,
) -> Result<[u32; 4], ControlError> {
    let Some([x, y, region_width, region_height]) = region else {
        return Ok([0, 0, width, height]);
    };
    if x < 0 || y < 0 || region_width <= 0 || region_height <= 0 {
        return Err(ControlError::invalid_params(
            "region must be [x, y, width, height] with non-negative x/y and positive width/height",
        ));
    }
    let x2 = x
        .checked_add(region_width)
        .ok_or_else(|| ControlError::invalid_params("region x + width overflowed"))?;
    let y2 = y
        .checked_add(region_height)
        .ok_or_else(|| ControlError::invalid_params("region y + height overflowed"))?;
    if x2 > i64::from(width) || y2 > i64::from(height) {
        return Err(ControlError::invalid_params(format!(
            "region [{x}, {y}, {region_width}, {region_height}] exceeds the captured {width}x{height} image"
        )));
    }
    Ok([
        u32::try_from(x).map_err(|_| ControlError::invalid_params("region x is out of range"))?,
        u32::try_from(y).map_err(|_| ControlError::invalid_params("region y is out of range"))?,
        u32::try_from(region_width)
            .map_err(|_| ControlError::invalid_params("region width is out of range"))?,
        u32::try_from(region_height)
            .map_err(|_| ControlError::invalid_params("region height is out of range"))?,
    ])
}

fn grid_boundary(start: u32, length: u32, index: u32, grid: u32) -> Result<u32, ControlError> {
    let offset = u64::from(index)
        .checked_mul(u64::from(length))
        .ok_or_else(|| ControlError::internal("Grid boundary overflowed"))?
        / u64::from(grid);
    let offset = u32::try_from(offset)
        .map_err(|_| ControlError::internal("Grid boundary exceeded image coordinates"))?;
    start
        .checked_add(offset)
        .ok_or_else(|| ControlError::internal("Grid boundary exceeded image coordinates"))
}

fn accumulate_region(image: &RgbImage, region: [u32; 4]) -> Result<PixelAccumulator, ControlError> {
    let mut accumulator = PixelAccumulator::default();
    let x_end = region[0]
        .checked_add(region[2])
        .ok_or_else(|| ControlError::internal("Analysis region x extent overflowed"))?;
    let y_end = region[1]
        .checked_add(region[3])
        .ok_or_else(|| ControlError::internal("Analysis region y extent overflowed"))?;
    for y in region[1]..y_end {
        for x in region[0]..x_end {
            accumulator.add(&image.get_pixel(x, y).0);
        }
    }
    Ok(accumulator)
}

fn normalized_rgb(pixel: &[u8; 3]) -> [f64; 3] {
    [
        f64::from(pixel[0]) / 255.0,
        f64::from(pixel[1]) / 255.0,
        f64::from(pixel[2]) / 255.0,
    ]
}

fn luma(rgb: [f64; 3]) -> f64 {
    let [red, green, blue] = rgb.map(srgb_to_linear);
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

fn srgb_to_linear(channel: f64) -> f64 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use image::{Rgb, RgbImage};

    use super::*;
    use crate::bridge::ErrorCode;

    fn options() -> FrameStatsOptions {
        FrameStatsOptions {
            grid: 1,
            region: None,
            sample_points: None,
        }
    }

    #[test]
    fn black_frame_has_documented_statistics_and_hints() {
        let image = RgbImage::from_pixel(2, 2, Rgb([0, 0, 0]));
        let result = analyze_frame(&image, &options()).unwrap();

        assert_eq!(result["overall"]["luma_mean"], 0.0);
        assert_eq!(result["overall"]["luma_std"], 0.0);
        assert_eq!(result["overall"]["pct_black"], 1.0);
        assert_eq!(result["overall"]["pct_clipped"], 0.0);
        assert_eq!(result["overall"]["histogram"][0], 4);
        assert_eq!(
            result["health_hints"],
            json!(["almost_all_black", "low_variance"])
        );
    }

    #[test]
    fn white_frame_is_clipped_and_uses_linear_luma() {
        let image = RgbImage::from_pixel(1, 1, Rgb([255, 255, 255]));
        let result = analyze_frame(&image, &options()).unwrap();

        assert!((result["overall"]["luma_mean"].as_f64().unwrap() - 1.0).abs() < 1e-12);
        assert_eq!(result["overall"]["pct_clipped"], 1.0);
        assert_eq!(result["overall"]["histogram"][15], 1);
        assert_eq!(
            result["health_hints"],
            json!(["mostly_clipped", "low_variance"])
        );
    }

    #[test]
    fn resizing_averages_light_in_linear_space() {
        let mut image = RgbImage::new(2, 1);
        image.put_pixel(0, 0, Rgb([0, 0, 0]));
        image.put_pixel(1, 0, Rgb([255, 255, 255]));
        let source_mean = analyze_frame(&image, &options()).unwrap()["overall"]["luma_mean"]
            .as_f64()
            .unwrap();

        let resized = resize_rgb_image_linear(image, Some(1));
        let resized_mean = analyze_frame(&resized, &options()).unwrap()["overall"]["luma_mean"]
            .as_f64()
            .unwrap();

        assert_eq!(resized.dimensions(), (1, 1));
        assert_eq!(resized.get_pixel(0, 0).0, [188, 188, 188]);
        assert!((resized_mean - source_mean).abs() < 0.01);
    }

    #[test]
    fn resizing_preserves_at_least_one_output_row() {
        let image = RgbImage::from_pixel(100, 1, Rgb([128, 128, 128]));
        assert_eq!(resize_rgb_image_linear(image, Some(1)).dimensions(), (1, 1));
    }

    #[test]
    fn grid_partitions_odd_dimensions_without_losing_pixels() {
        let image = RgbImage::from_pixel(5, 3, Rgb([64, 128, 192]));
        let mut options = options();
        options.grid = 2;
        let result = analyze_frame(&image, &options).unwrap();
        let cells = result["cells"].as_array().unwrap();

        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0]["bounds"], json!([0, 0, 2, 1]));
        assert_eq!(cells[1]["bounds"], json!([2, 0, 3, 1]));
        assert_eq!(cells[2]["bounds"], json!([0, 1, 2, 2]));
        assert_eq!(cells[3]["bounds"], json!([2, 1, 3, 2]));
    }

    #[test]
    fn region_and_sample_points_use_output_pixel_coordinates() {
        let mut image = RgbImage::from_pixel(4, 3, Rgb([0, 0, 0]));
        image.put_pixel(2, 1, Rgb([255, 0, 0]));
        let mut options = options();
        options.region = Some([2, 1, 1, 1]);
        options.sample_points = Some(vec![[2, 1]]);
        let result = analyze_frame(&image, &options).unwrap();

        assert_eq!(result["region"], json!([2, 1, 1, 1]));
        assert_eq!(result["samples"][0]["rgb"], json!([1.0, 0.0, 0.0]));
        assert!((result["samples"][0]["luma"].as_f64().unwrap() - 0.2126).abs() < 1e-12);
    }

    #[test]
    fn invalid_analysis_bounds_return_client_errors() {
        let image = RgbImage::from_pixel(4, 4, Rgb([0, 0, 0]));
        let mut invalid_grid = options();
        invalid_grid.grid = 17;
        assert!(analyze_frame(&image, &invalid_grid).is_err());

        let mut invalid_region = options();
        invalid_region.region = Some([3, 3, 2, 2]);
        assert!(analyze_frame(&image, &invalid_region).is_err());

        let mut invalid_sample = options();
        invalid_sample.sample_points = Some(vec![[4, 0]]);
        assert!(analyze_frame(&image, &invalid_sample).is_err());
    }

    #[test]
    fn comparison_reports_exact_changed_region() {
        let original = Arc::new(RgbImage::from_pixel(3, 2, Rgb([0, 0, 0])));
        let mut changed = RgbImage::from_pixel(3, 2, Rgb([0, 0, 0]));
        changed.put_pixel(1, 0, Rgb([255, 0, 0]));
        changed.put_pixel(2, 1, Rgb([0, 128, 0]));

        let mut frames = CapturedFrames::default();
        let a = frames
            .retain(
                original,
                CapturedFrameMetadata {
                    kind: "screenshot",
                    max_width: None,
                    hot_reload_generation: Some(1),
                },
            )
            .frame_id
            .unwrap();
        let b = frames
            .retain(
                Arc::new(changed),
                CapturedFrameMetadata {
                    kind: "stats",
                    max_width: None,
                    hot_reload_generation: Some(2),
                },
            )
            .frame_id
            .unwrap();
        let result = frames.compare(&a, &b, DEFAULT_COMPARE_EPSILON).unwrap();

        assert_eq!(result["identical"], false);
        assert_eq!(result["pct_pixels_changed"], 100.0 / 3.0);
        assert_eq!(result["changed_bbox"], json!([1, 0, 2, 2]));
        assert_eq!(result["changed_centroid"], json!([1.5, 0.5]));
        assert_eq!(result["a_metadata"]["hot_reload_generation"], 1);
        assert_eq!(result["b_metadata"]["kind"], "stats");
    }

    #[test]
    fn comparison_honors_epsilon_and_rejects_invalid_inputs() {
        let mut frames = CapturedFrames::default();
        let a = frames
            .retain(
                Arc::new(RgbImage::from_pixel(1, 1, Rgb([0, 0, 0]))),
                CapturedFrameMetadata {
                    kind: "screenshot",
                    max_width: None,
                    hot_reload_generation: None,
                },
            )
            .frame_id
            .unwrap();
        let b = frames
            .retain(
                Arc::new(RgbImage::from_pixel(1, 1, Rgb([1, 0, 0]))),
                CapturedFrameMetadata {
                    kind: "screenshot",
                    max_width: None,
                    hot_reload_generation: None,
                },
            )
            .frame_id
            .unwrap();

        assert_eq!(
            frames.compare(&a, &b, 1.0 / 255.0).unwrap()["pct_pixels_changed"],
            0.0
        );
        assert_eq!(
            frames.compare(&a, &b, 1.0 / 255.0).unwrap()["identical"],
            true
        );
        assert_eq!(frames.compare(&a, &b, 0.0).unwrap()["identical"], false);
        assert!(frames.compare(&a, &b, f64::NAN).is_err());
        assert!(frames.compare("missing", &b, 0.0).is_err());
    }

    #[test]
    fn comparison_rejects_dimension_mismatches() {
        let mut frames = CapturedFrames::default();
        let metadata = || CapturedFrameMetadata {
            kind: "stats",
            max_width: None,
            hot_reload_generation: None,
        };
        let a = frames
            .retain(
                Arc::new(RgbImage::from_pixel(1, 1, Rgb([0, 0, 0]))),
                metadata(),
            )
            .frame_id
            .unwrap();
        let b = frames
            .retain(
                Arc::new(RgbImage::from_pixel(2, 1, Rgb([0, 0, 0]))),
                metadata(),
            )
            .frame_id
            .unwrap();

        let error = frames.compare(&a, &b, 0.0).unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        assert!(error.message.contains("different dimensions"));
    }

    #[test]
    fn retention_evicts_by_count_and_byte_limit() {
        let mut frames = CapturedFrames::with_limits(2, 9);
        let metadata = || CapturedFrameMetadata {
            kind: "stats",
            max_width: None,
            hot_reload_generation: None,
        };
        let a = frames
            .retain(
                Arc::new(RgbImage::from_pixel(1, 1, Rgb([0, 0, 0]))),
                metadata(),
            )
            .frame_id
            .unwrap();
        let b = frames
            .retain(
                Arc::new(RgbImage::from_pixel(1, 1, Rgb([1, 1, 1]))),
                metadata(),
            )
            .frame_id
            .unwrap();
        let c = frames
            .retain(
                Arc::new(RgbImage::from_pixel(1, 1, Rgb([2, 2, 2]))),
                metadata(),
            )
            .frame_id
            .unwrap();

        assert!(frames.compare(&a, &b, 0.0).is_err());
        assert!(frames.compare(&b, &c, 0.0).is_ok());

        let oversized = frames.retain(
            Arc::new(RgbImage::from_pixel(2, 2, Rgb([0, 0, 0]))),
            metadata(),
        );
        assert!(!oversized.retained);
        assert!(oversized.frame_id.is_none());
        assert_eq!(
            oversized.reason,
            Some("captured frame exceeds the retention limit")
        );
    }

    #[test]
    fn frame_ids_do_not_alias_across_process_namespaces() {
        let metadata = || CapturedFrameMetadata {
            kind: "stats",
            max_width: None,
            hot_reload_generation: None,
        };
        let image = || Arc::new(RgbImage::from_pixel(1, 1, Rgb([0, 0, 0])));
        let mut first = CapturedFrames::with_namespace(8, 1024, Uuid::from_u128(1));
        let old_id = first.retain(image(), metadata()).frame_id.unwrap();
        let mut restarted = CapturedFrames::with_namespace(8, 1024, Uuid::from_u128(2));
        let new_id = restarted.retain(image(), metadata()).frame_id.unwrap();

        assert_ne!(old_id, new_id);
        assert!(restarted.compare(&old_id, &new_id, 0.0).is_err());
    }
}
