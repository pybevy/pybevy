//! Compile-time layout assertions for View API component types
//!
//! This module ensures that our assumptions about component memory layouts
//! are validated at compile time. If Bevy changes the layout of any component
//! in a future version, the build will fail rather than causing subtle bugs.
//!
//! These assertions are critical for the View API which relies on direct
//! memory access via field offsets for performance.

use bevy::{
    light::{DirectionalLight, PointLight, SpotLight},
    transform::components::Transform,
};

/// Verify Transform has stable size (48 bytes on current Rust/Bevy)
///
/// Layout: rotation (0-15), translation (16-27), scale (28-39), padding (40-47)
/// Total: 48 bytes (16-byte aligned, likely for SIMD)
///
/// IMPORTANT: Rust reordered fields! Declaration order is translation/rotation/scale,
/// but actual memory order is rotation/translation/scale (by alignment requirement).
const _: () = {
    assert!(
        std::mem::size_of::<Transform>() == 48,
        "Transform size changed! Expected 48 bytes (16-byte aligned). \
         This affects View API direct memory access."
    );
};

/// Verify Transform field offsets remain stable
///
/// These specific offsets are enforced to catch any layout changes.
/// The View API depends on these offsets for direct memory access.
const _: () = {
    const ROTATION: usize = std::mem::offset_of!(Transform, rotation);
    const TRANSLATION: usize = std::mem::offset_of!(Transform, translation);
    const SCALE: usize = std::mem::offset_of!(Transform, scale);

    // Enforce the actual layout observed on current Rust/Bevy
    // Rust reordered fields by alignment: rotation (16-byte) first, then Vec3s
    assert!(
        ROTATION == 0,
        "Transform.rotation offset changed! Expected 0, affects View API"
    );
    assert!(
        TRANSLATION == 16,
        "Transform.translation offset changed! Expected 16, affects View API"
    );
    assert!(
        SCALE == 28,
        "Transform.scale offset changed! Expected 28, affects View API"
    );
};

const _: () = {
    const SIZE: usize = std::mem::size_of::<PointLight>();

    // Verify all field offsets are within bounds
    assert!(
        std::mem::offset_of!(PointLight, intensity) < SIZE,
        "PointLight::intensity offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(PointLight, range) < SIZE,
        "PointLight::range offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(PointLight, radius) < SIZE,
        "PointLight::radius offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(PointLight, shadow_depth_bias) < SIZE,
        "PointLight::shadow_depth_bias offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(PointLight, shadow_normal_bias) < SIZE,
        "PointLight::shadow_normal_bias offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(PointLight, shadow_map_near_z) < SIZE,
        "PointLight::shadow_map_near_z offset out of bounds"
    );
};

const _: () = {
    const SIZE: usize = std::mem::size_of::<DirectionalLight>();

    assert!(
        std::mem::offset_of!(DirectionalLight, illuminance) < SIZE,
        "DirectionalLight::illuminance offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(DirectionalLight, shadow_depth_bias) < SIZE,
        "DirectionalLight::shadow_depth_bias offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(DirectionalLight, shadow_normal_bias) < SIZE,
        "DirectionalLight::shadow_normal_bias offset out of bounds"
    );
};

const _: () = {
    const SIZE: usize = std::mem::size_of::<SpotLight>();

    assert!(
        std::mem::offset_of!(SpotLight, intensity) < SIZE,
        "SpotLight::intensity offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(SpotLight, range) < SIZE,
        "SpotLight::range offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(SpotLight, radius) < SIZE,
        "SpotLight::radius offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(SpotLight, shadow_depth_bias) < SIZE,
        "SpotLight::shadow_depth_bias offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(SpotLight, shadow_normal_bias) < SIZE,
        "SpotLight::shadow_normal_bias offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(SpotLight, shadow_map_near_z) < SIZE,
        "SpotLight::shadow_map_near_z offset out of bounds"
    );
    assert!(
        std::mem::offset_of!(SpotLight, outer_angle) < SIZE,
        "SpotLight::outer_angle offset out of bounds"
    );
};
