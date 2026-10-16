use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::{DynamicImage, GenericImageView, ImageFormat, codecs::jpeg::JpegEncoder};
use serde_json::{Map, Value};

pub(crate) const MCP_IMAGE_DELIVERY_HEADER: &str = "x-pybevy-image-delivery";
pub(crate) const MCP_IMAGE_DELIVERY_VALUE: &str = "mcp";

const MAX_INLINE_IMAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_INLINE_DECODED_BYTES: usize = 8 * 1024 * 1024;
const PREVIEW_MAX_DIMENSION: u32 = 1280;

#[derive(Debug)]
pub(crate) enum ImageDelivery {
    Original,
    Preview {
        bytes: Vec<u8>,
        width: u32,
        height: u32,
    },
    Omitted {
        reason: String,
    },
}

#[derive(Debug)]
pub(crate) struct PreparedMcpImage {
    pub source_dimensions: Option<(u32, u32)>,
    pub delivery: ImageDelivery,
}

/// Rewrite every screenshot payload in an engine response for MCP delivery.
///
/// This runs inside the engine's Rust HTTP server, before response JSON crosses
/// into the Python stdio bridge. Ordinary REST responses never call this path.
pub(crate) fn prepare_mcp_response_images(response: &mut Value) {
    let Some(response) = response.as_object_mut() else {
        return;
    };
    prepare_response_object(response);

    let Some(results) = response.get_mut("results").and_then(Value::as_array_mut) else {
        return;
    };
    for action in results {
        let Some(action) = action.as_object_mut() else {
            continue;
        };
        let is_image_tool = action
            .get("tool")
            .and_then(Value::as_str)
            .is_some_and(|tool| {
                matches!(
                    tool,
                    "capture_screenshot"
                        | "capture_timeline"
                        | "capture_turnaround"
                        | "capture_depth"
                        | "reload_and_capture"
                )
            });
        if !is_image_tool {
            continue;
        }
        if let Some(result) = action.get_mut("result").and_then(Value::as_object_mut) {
            prepare_response_object(result);
        }
    }
}

fn prepare_response_object(object: &mut Map<String, Value>) {
    if object.contains_key("image_delivery") {
        return;
    }

    let Some((image_key, encoded)) = ["image", "screenshot"].into_iter().find_map(|key| {
        object
            .get(key)
            .and_then(Value::as_str)
            .map(|encoded| (key, encoded.to_owned()))
    }) else {
        return;
    };

    let png_bytes = match STANDARD.decode(&encoded) {
        Ok(bytes) => bytes,
        Err(error) => {
            object.insert(image_key.to_owned(), Value::Null);
            object.insert(
                "image_delivery".to_owned(),
                serde_json::json!({
                    "inline_mime_type": null,
                    "inline_image_omitted": format!("invalid base64: {error}"),
                }),
            );
            return;
        }
    };

    let prepared = prepare_mcp_image(
        &png_bytes,
        MAX_INLINE_IMAGE_BYTES,
        MAX_INLINE_DECODED_BYTES,
        PREVIEW_MAX_DIMENSION,
    );
    let mut metadata = Map::new();
    metadata.insert(
        "full_resolution_bytes".to_owned(),
        Value::from(png_bytes.len() as u64),
    );
    if let Some((width, height)) = prepared.source_dimensions {
        metadata.insert("full_resolution_width".to_owned(), Value::from(width));
        metadata.insert("full_resolution_height".to_owned(), Value::from(height));
    }

    match prepared.delivery {
        ImageDelivery::Original => {
            metadata.insert(
                "inline_mime_type".to_owned(),
                Value::String("image/png".to_owned()),
            );
        }
        ImageDelivery::Preview {
            bytes,
            width,
            height,
        } => {
            object.insert(image_key.to_owned(), Value::String(STANDARD.encode(&bytes)));
            if object.contains_key("format") {
                object.insert("format".to_owned(), Value::String("jpeg".to_owned()));
            }
            metadata.insert(
                "inline_mime_type".to_owned(),
                Value::String("image/jpeg".to_owned()),
            );
            metadata.insert("inline_preview".to_owned(), Value::Bool(true));
            metadata.insert(
                "inline_preview_bytes".to_owned(),
                Value::from(bytes.len() as u64),
            );
            metadata.insert("inline_preview_width".to_owned(), Value::from(width));
            metadata.insert("inline_preview_height".to_owned(), Value::from(height));
        }
        ImageDelivery::Omitted { reason } => {
            object.insert(image_key.to_owned(), Value::Null);
            metadata.insert("inline_mime_type".to_owned(), Value::Null);
            metadata.insert("inline_image_omitted".to_owned(), Value::String(reason));
        }
    }
    object.insert("image_delivery".to_owned(), Value::Object(metadata));
}

/// Bound an MCP screenshot without depending on an interpreter image library.
///
/// Small opaque payloads retain the bridge's compatibility behavior and pass
/// through unchanged. Payloads over either transport or decoded-size limit
/// must decode as PNG and are converted into a bounded JPEG preview.
pub(crate) fn prepare_mcp_image(
    png_bytes: &[u8],
    max_inline_bytes: usize,
    max_decoded_bytes: usize,
    preview_max_dimension: u32,
) -> PreparedMcpImage {
    let mut needs_preview = png_bytes.len() > max_inline_bytes;
    let source = match image::load_from_memory_with_format(png_bytes, ImageFormat::Png) {
        Ok(source) => source,
        Err(_) if !needs_preview => {
            return PreparedMcpImage {
                source_dimensions: None,
                delivery: ImageDelivery::Original,
            };
        }
        Err(error) => {
            return PreparedMcpImage {
                source_dimensions: None,
                delivery: ImageDelivery::Omitted {
                    reason: format!("preview generation failed: {error}"),
                },
            };
        }
    };

    let (width, height) = source.dimensions();
    let decoded_bytes = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4);
    needs_preview |= decoded_bytes > max_decoded_bytes as u64;

    if !needs_preview {
        return PreparedMcpImage {
            source_dimensions: Some((width, height)),
            delivery: ImageDelivery::Original,
        };
    }

    let max_dimension = preview_max_dimension.max(1);
    let rgb = DynamicImage::ImageRgb8(source.to_rgb8());
    let preview = if width > max_dimension || height > max_dimension {
        rgb.resize(
            max_dimension,
            max_dimension,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        rgb
    };
    let (preview_width, preview_height) = preview.dimensions();
    let mut preview_bytes = Vec::new();
    if let Err(error) = JpegEncoder::new_with_quality(&mut preview_bytes, 80).encode_image(&preview)
    {
        return PreparedMcpImage {
            source_dimensions: Some((width, height)),
            delivery: ImageDelivery::Omitted {
                reason: format!("preview generation failed: {error}"),
            },
        };
    }

    let delivery = if preview_bytes.len() > max_inline_bytes {
        ImageDelivery::Omitted {
            reason: "generated preview still exceeds inline limit".to_string(),
        }
    } else {
        ImageDelivery::Preview {
            bytes: preview_bytes,
            width: preview_width,
            height: preview_height,
        }
    };
    PreparedMcpImage {
        source_dimensions: Some((width, height)),
        delivery,
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    use super::{ImageDelivery, prepare_mcp_image, prepare_mcp_response_images};

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, Rgba([20, 40, 60, 255]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    #[test]
    fn small_png_keeps_original_payload() {
        let bytes = png(320, 200);
        let prepared = prepare_mcp_image(&bytes, 2 * 1024 * 1024, 8 * 1024 * 1024, 1280);

        assert_eq!(prepared.source_dimensions, Some((320, 200)));
        assert!(matches!(prepared.delivery, ImageDelivery::Original));
    }

    #[test]
    fn decoded_size_limit_produces_bounded_jpeg() {
        let bytes = png(2400, 1600);
        let prepared = prepare_mcp_image(&bytes, 2 * 1024 * 1024, 8 * 1024 * 1024, 1280);

        assert_eq!(prepared.source_dimensions, Some((2400, 1600)));
        let ImageDelivery::Preview {
            bytes,
            width,
            height,
        } = prepared.delivery
        else {
            panic!("expected a preview");
        };
        assert!(bytes.starts_with(&[0xff, 0xd8]));
        assert!(bytes.len() < 2 * 1024 * 1024);
        assert_eq!((width, height), (1280, 853));
    }

    #[test]
    fn oversized_invalid_payload_is_omitted() {
        let prepared = prepare_mcp_image(b"not an image", 1, usize::MAX, 1280);

        let ImageDelivery::Omitted { reason } = prepared.delivery else {
            panic!("expected an omission");
        };
        assert!(reason.starts_with("preview generation failed:"));
    }

    #[test]
    fn preview_over_transport_limit_is_omitted() {
        let bytes = png(2400, 1600);
        let prepared = prepare_mcp_image(&bytes, 1, 8 * 1024 * 1024, 1280);

        assert!(matches!(
            prepared.delivery,
            ImageDelivery::Omitted {
                ref reason,
            } if reason == "generated preview still exceeds inline limit"
        ));
    }

    #[test]
    fn response_preview_is_generated_before_http_delivery() {
        let source = png(2400, 1600);
        let encoded = STANDARD.encode(&source);
        let mut response = serde_json::json!({
            "image": encoded,
            "width": 2400,
            "height": 1600,
            "format": "png",
            "encoding": "base64",
        });

        prepare_mcp_response_images(&mut response);

        let preview = STANDARD
            .decode(response["image"].as_str().unwrap())
            .unwrap();
        assert!(preview.starts_with(&[0xff, 0xd8]));
        assert_ne!(preview, source);
        assert_eq!(response["format"], "jpeg");
        assert_eq!(response["image_delivery"]["inline_mime_type"], "image/jpeg");
        assert_eq!(response["image_delivery"]["inline_preview"], true);
        assert_eq!(response["image_delivery"]["full_resolution_width"], 2400);
        assert_eq!(response["image_delivery"]["inline_preview_width"], 1280);
    }

    #[test]
    fn nested_scheduled_images_use_the_same_delivery_policy() {
        let source = png(2400, 1600);
        let mut response = serde_json::json!({
            "results": [{
                "index": 0,
                "tool": "capture_screenshot",
                "result": {"screenshot": STANDARD.encode(source)},
            }],
        });

        prepare_mcp_response_images(&mut response);

        let result = &response["results"][0]["result"];
        assert_eq!(result["image_delivery"]["inline_mime_type"], "image/jpeg");
        assert_eq!(result["image_delivery"]["inline_preview"], true);
        assert!(result["screenshot"].as_str().is_some());
    }

    #[test]
    fn scheduled_non_image_tools_keep_semantic_image_fields_untouched() {
        let image_value = "this is application data, not a screenshot";
        let mut response = serde_json::json!({
            "results": [{
                "index": 0,
                "tool": "run_code",
                "result": {"image": image_value},
            }],
        });

        prepare_mcp_response_images(&mut response);

        let result = &response["results"][0]["result"];
        assert_eq!(result["image"], image_value);
        assert!(result.get("image_delivery").is_none());
    }

    #[test]
    fn invalid_base64_is_omitted_without_crossing_the_bridge() {
        let mut response = serde_json::json!({"image": "not base64!"});

        prepare_mcp_response_images(&mut response);

        assert!(response["image"].is_null());
        assert!(response["image_delivery"]["inline_mime_type"].is_null());
        assert!(
            response["image_delivery"]["inline_image_omitted"]
                .as_str()
                .unwrap()
                .starts_with("invalid base64:")
        );
    }
}
