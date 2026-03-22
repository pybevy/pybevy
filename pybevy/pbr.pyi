from typing import ClassVar

import numpy as np

from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.color import Color, LinearRgba
from pybevy.ecs import Batchable, Component, Resource
from pybevy.image import Image
from pybevy.math import Affine2, Rect, UVec2, UVec3, Vec3
from pybevy.render import AlphaMode
from pybevy.wgpu import Face

class PbrPlugin(Plugin):
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class UvChannel:
    """UV channel selection for texture mapping."""

    Uv0: ClassVar[UvChannel]
    Uv1: ClassVar[UvChannel]
    UV0: ClassVar[UvChannel]
    UV1: ClassVar[UvChannel]

    def __init__(self) -> None: ...

class OpaqueRenderMethod:
    """Opaque rendering method selection."""

    Forward: ClassVar[OpaqueRenderMethod]
    Deferred: ClassVar[OpaqueRenderMethod]
    Auto: ClassVar[OpaqueRenderMethod]
    FORWARD: ClassVar[OpaqueRenderMethod]
    DEFERRED: ClassVar[OpaqueRenderMethod]
    AUTO: ClassVar[OpaqueRenderMethod]

class ParallaxMappingMethod:
    OCCLUSION: ClassVar[ParallaxMappingMethod]

    def __init__(self) -> None: ...
    @staticmethod
    def Occlusion() -> ParallaxMappingMethod: ...
    @staticmethod
    def Relief(max_steps: int) -> ParallaxMappingMethod: ...

class StandardMaterial(Asset):
    def __init__(
        self,
        base_color: Color = Color.WHITE,
        base_color_texture: Handle[Image] | None = None,
        base_color_channel: UvChannel = UvChannel.Uv0,
        emissive: Color | LinearRgba = Color.BLACK,
        emissive_exposure_weight: float = 0.0,
        emissive_channel: UvChannel = UvChannel.Uv0,
        emissive_texture: Handle[Image] | None = None,
        perceptual_roughness: float = 0.5,
        metallic: float = 0.0,
        metallic_roughness_channel: UvChannel = UvChannel.Uv0,
        metallic_roughness_texture: Handle[Image] | None = None,
        reflectance: float = 0.5,
        specular_tint: Color = Color.WHITE,
        diffuse_transmission: float = 0.0,
        specular_transmission: float = 0.0,
        thickness: float = 0.0,
        ior: float = 1.5,
        attenuation_distance: float = float("inf"),
        attenuation_color: Color = Color.WHITE,
        normal_map_channel: UvChannel = UvChannel.Uv0,
        normal_map_texture: Handle[Image] | None = None,
        flip_normal_map_y: bool = False,
        occlusion_channel: UvChannel = UvChannel.Uv0,
        occlusion_texture: Handle[Image] | None = None,
        anisotropy_strength: float = 0.0,
        anisotropy_rotation: float = 0.0,
        double_sided: bool = False,
        cull_mode: Face | None = Face.Back,
        unlit: bool = False,
        fog_enabled: bool = True,
        alpha_mode: AlphaMode = ...,
        depth_bias: float = 0.0,
        depth_map: Handle[Image] | None = None,
        parallax_depth_scale: float = 0.1,
        parallax_mapping_method: ParallaxMappingMethod = ParallaxMappingMethod.Occlusion(),
        max_parallax_layer_count: float = 16.0,
        lightmap_exposure: float = 1.0,
        opaque_render_method: OpaqueRenderMethod = OpaqueRenderMethod.Auto,
        deferred_lighting_pass_id: int = 1,
        uv_transform: Affine2 = Affine2.IDENTITY,
        clearcoat: float = 0.0,
        clearcoat_perceptual_roughness: float = 0.5,
    ) -> None: ...
    @staticmethod
    def from_color(color: Color) -> StandardMaterial: ...
    base_color: Color
    base_color_texture: Handle[Image] | None
    base_color_channel: UvChannel
    emissive: Color | LinearRgba
    emissive_exposure_weight: float
    emissive_channel: UvChannel
    emissive_texture: Handle[Image] | None
    perceptual_roughness: float
    metallic: float
    metallic_roughness_channel: UvChannel
    metallic_roughness_texture: Handle[Image] | None
    reflectance: float
    specular_tint: Color
    diffuse_transmission: float
    specular_transmission: float
    thickness: float
    ior: float
    attenuation_distance: float
    attenuation_color: Color
    normal_map_channel: UvChannel
    normal_map_texture: Handle[Image] | None
    flip_normal_map_y: bool
    occlusion_channel: UvChannel
    occlusion_texture: Handle[Image] | None
    anisotropy_strength: float
    anisotropy_rotation: float
    double_sided: bool
    cull_mode: Face | None
    unlit: bool
    fog_enabled: bool
    alpha_mode: AlphaMode
    depth_bias: float
    depth_map: Handle[Image] | None
    parallax_depth_scale: float
    parallax_mapping_method: ParallaxMappingMethod
    max_parallax_layer_count: float
    lightmap_exposure: float
    opaque_render_method: OpaqueRenderMethod
    deferred_lighting_pass_id: int
    uv_transform: Affine2
    clearcoat: float
    clearcoat_perceptual_roughness: float

    def flip(self, horizontal: bool, vertical: bool) -> None:
        """Flip the texture coordinates of the material in place."""

    def flipped(self, horizontal: bool, vertical: bool) -> StandardMaterial:
        """Return the material with flipped texture coordinates."""

class FogFalloff:
    """Fog falloff mode controlling how fog density changes with distance.

    Determines the mathematical relationship between distance and fog density.
    """

    REVISED_KOSCHMIEDER_CONTRAST_THRESHOLD: float

    @staticmethod
    def Linear(start: float, end: float) -> FogFalloff:
        """Linear fog falloff between start and end distances."""

    @staticmethod
    def Exponential(density: float) -> FogFalloff:
        """Exponential fog falloff with given density."""

    @staticmethod
    def ExponentialSquared(density: float) -> FogFalloff:
        """Exponential squared fog falloff with given density."""

    @staticmethod
    def Atmospheric(extinction: Vec3, inscattering: Vec3) -> FogFalloff:
        """Atmospheric fog with separate extinction and inscattering colors."""

    @staticmethod
    def from_visibility(visibility: float) -> FogFalloff:
        """Create exponential fog from visibility distance."""

    @staticmethod
    def from_visibility_squared(visibility: float) -> FogFalloff:
        """Create exponential squared fog from visibility distance."""

    @staticmethod
    def from_visibility_color(visibility: float, extinction_inscattering_color: Color) -> FogFalloff:
        """Create atmospheric fog from visibility and color."""

    @staticmethod
    def from_visibility_colors(visibility: float, extinction_color: Color, inscattering_color: Color) -> FogFalloff:
        """Create atmospheric fog from visibility and separate extinction/inscattering colors."""

    @staticmethod
    def from_visibility_contrast(visibility: float, contrast_threshold: float) -> FogFalloff:
        """Create exponential fog from visibility and contrast threshold."""

    @staticmethod
    def from_visibility_contrast_squared(visibility: float, contrast_threshold: float) -> FogFalloff:
        """Create exponential squared fog from visibility and contrast threshold."""

    @staticmethod
    def from_visibility_contrast_color(visibility: float, contrast_threshold: float, extinction_inscattering_color: Color) -> FogFalloff:
        """Create atmospheric fog from visibility, contrast, and color."""

    @staticmethod
    def from_visibility_contrast_colors(visibility: float, contrast_threshold: float, extinction_color: Color, inscattering_color: Color) -> FogFalloff:
        """Create atmospheric fog from visibility, contrast, and separate colors."""

    @staticmethod
    def koschmieder(v: float, c_t: float) -> float:
        """Calculate fog density using Koschmieder's equation.

        Args:
            v: Visibility distance
            c_t: Contrast threshold

        Returns:
            Fog density coefficient
        """

class DistanceFog(Component):
    """Distance-based fog component for atmospheric effects.

    Adds fog that increases with distance from the camera, creating depth
    and atmospheric perspective in 3D scenes.
    """

    def __init__(
        self,
        color: Color = ...,
        falloff: FogFalloff = ...,
        directional_light_color: Color = ...,
        directional_light_exponent: float = 8.0,
    ) -> None: ...

    @property
    def color(self) -> Color:
        """Base color of the fog."""
    @color.setter
    def color(self, value: Color) -> None: ...

    @property
    def falloff(self) -> FogFalloff:
        """Fog falloff mode controlling density vs distance."""
    @falloff.setter
    def falloff(self, value: FogFalloff) -> None: ...

    @property
    def directional_light_color(self) -> Color:
        """Color tint for directional light passing through fog."""
    @directional_light_color.setter
    def directional_light_color(self, value: Color) -> None: ...

    @property
    def directional_light_exponent(self) -> float:
        """Exponent controlling directional light scattering intensity."""
    @directional_light_exponent.setter
    def directional_light_exponent(self, value: float) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        directional_light_exponent: np.ndarray | None = None,
        color: np.ndarray | None = None,
        directional_light_color: np.ndarray | None = None,
    ) -> Batchable: ...

class ScreenSpaceAmbientOcclusionQualityLevel:
    """Quality level for Screen Space Ambient Occlusion (SSAO).

    Higher quality levels use more samples and produce better results
    at the cost of performance.
    """

    LOW: ClassVar[ScreenSpaceAmbientOcclusionQualityLevel]
    MEDIUM: ClassVar[ScreenSpaceAmbientOcclusionQualityLevel]
    HIGH: ClassVar[ScreenSpaceAmbientOcclusionQualityLevel]
    ULTRA: ClassVar[ScreenSpaceAmbientOcclusionQualityLevel]

    @staticmethod
    def Low() -> ScreenSpaceAmbientOcclusionQualityLevel: ...
    @staticmethod
    def Medium() -> ScreenSpaceAmbientOcclusionQualityLevel: ...
    @staticmethod
    def High() -> ScreenSpaceAmbientOcclusionQualityLevel: ...
    @staticmethod
    def Ultra() -> ScreenSpaceAmbientOcclusionQualityLevel: ...
    @staticmethod
    def Custom(samples_per_slice_side: int, slice_count: int) -> ScreenSpaceAmbientOcclusionQualityLevel: ...

class ScreenSpaceAmbientOcclusion(Component):
    """Screen Space Ambient Occlusion (SSAO) component.

    SSAO is a rendering technique that approximates ambient occlusion
    in real-time by darkening creases, holes, and surfaces that are
    close to each other in screen space.
    """

    def __init__(
        self,
        quality_level: ScreenSpaceAmbientOcclusionQualityLevel = ...,
        constant_object_thickness: float = 0.0,
    ) -> None: ...

    def __eq__(self, other: object) -> bool: ...

    @property
    def quality_level(self) -> ScreenSpaceAmbientOcclusionQualityLevel:
        """Quality level for SSAO computation."""
    @quality_level.setter
    def quality_level(self, value: ScreenSpaceAmbientOcclusionQualityLevel) -> None: ...

    @property
    def constant_object_thickness(self) -> float:
        """Constant object thickness assumption for SSAO."""
    @constant_object_thickness.setter
    def constant_object_thickness(self, value: float) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        constant_object_thickness: np.ndarray | None = None,
    ) -> Batchable: ...

class Wireframe(Component):
    """Wireframe rendering marker component.

    Entities with this component will be rendered in wireframe mode,
    showing only the edges of their geometry. Useful for debugging
    and visualization.
    """

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class WireframeColor(Component):
    """Wireframe color override component.

    When attached to an entity with the Wireframe component,
    this specifies a custom color for the wireframe rendering.
    """

    def __init__(self, color: Color = ...) -> None: ...

    @property
    def color(self) -> Color:
        """Color to use for wireframe rendering."""
    @color.setter
    def color(self, value: Color) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        color: np.ndarray | None = None,
    ) -> Batchable: ...

class NoWireframe(Component):
    """Marker component to disable wireframe rendering.

    Entities with this component will not be rendered in wireframe mode,
    even if global wireframe rendering is enabled via WireframeConfig.
    """

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class WireframeConfig(Resource):
    """Global wireframe configuration resource.

    Controls global wireframe rendering settings including whether
    wireframe rendering is enabled globally and the default color
    for wireframes that don't have a WireframeColor component.
    """

    def __init__(
        self,
        global_: bool = False,
        default_color: Color = ...,
    ) -> None: ...

    @property
    def global_(self) -> bool:
        """Enable wireframe rendering globally for all entities."""
    @global_.setter
    def global_(self, value: bool) -> None: ...

    @property
    def default_color(self) -> Color:
        """Default color for wireframe rendering."""
    @default_color.setter
    def default_color(self, value: Color) -> None: ...

class WireframeMaterial(Asset):
    """Material asset for wireframe rendering.

    A specialized material used for rendering meshes in wireframe mode.
    """

    def __init__(self, color: Color = ...) -> None: ...
    @property
    def color(self) -> Color: ...

class Mesh3dWireframe(Component):
    """Component holding a wireframe material handle.

    Specifies a custom wireframe material to use for rendering
    a specific entity in wireframe mode.
    """

    def __init__(self, handle: Handle) -> None: ...

    def handle(self) -> Handle:
        """Get the wireframe material handle."""

    def __eq__(self, other: object) -> bool: ...

class ScreenSpaceReflections(Component):
    """Screen Space Reflections (SSR) component.

    Enables real-time reflections on surfaces using screen-space ray marching.
    Provides high-quality reflections for smooth, reflective surfaces.
    """

    def __init__(
        self,
        perceptual_roughness_threshold: float = 0.1,
        thickness: float = 0.25,
        linear_steps: int = 16,
        linear_march_exponent: float = 1.0,
        bisection_steps: int = 4,
        use_secant: bool = True,
    ) -> None: ...

    @property
    def perceptual_roughness_threshold(self) -> float:
        """Roughness threshold above which SSR is disabled."""
    @perceptual_roughness_threshold.setter
    def perceptual_roughness_threshold(self, value: float) -> None: ...

    @property
    def thickness(self) -> float:
        """Thickness assumption for ray marching."""
    @thickness.setter
    def thickness(self, value: float) -> None: ...

    @property
    def linear_steps(self) -> int:
        """Number of linear steps in ray march."""
    @linear_steps.setter
    def linear_steps(self, value: int) -> None: ...

    @property
    def linear_march_exponent(self) -> float:
        """Exponent for linear march step distribution."""
    @linear_march_exponent.setter
    def linear_march_exponent(self, value: float) -> None: ...

    @property
    def bisection_steps(self) -> int:
        """Number of bisection refinement steps."""
    @bisection_steps.setter
    def bisection_steps(self, value: int) -> None: ...

    @property
    def use_secant(self) -> bool:
        """Whether to use secant method for refinement."""
    @use_secant.setter
    def use_secant(self, value: bool) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        perceptual_roughness_threshold: np.ndarray | None = None,
        thickness: np.ndarray | None = None,
        linear_steps: np.ndarray | None = None,
        linear_march_exponent: np.ndarray | None = None,
        bisection_steps: np.ndarray | None = None,
        use_secant: np.ndarray | None = None,
    ) -> Batchable: ...

class Lightmap(Component):
    """Lightmap component for pre-baked lighting.

    Applies a pre-computed lightmap texture to an entity for static lighting.
    Useful for baked global illumination and ambient occlusion.
    """

    def __init__(
        self,
        image: Handle[Image] | None = None,
        uv_rect: Rect | None = None,
        bicubic_sampling: bool = False,
    ) -> None: ...

    @property
    def image(self) -> Handle[Image]:
        """Handle to the lightmap image."""
    @image.setter
    def image(self, value: Handle[Image]) -> None: ...

    @property
    def uv_rect(self) -> Rect:
        """UV rectangle within the lightmap atlas."""
    @uv_rect.setter
    def uv_rect(self, value: Rect) -> None: ...

    @property
    def bicubic_sampling(self) -> bool:
        """Whether to use bicubic sampling for higher quality."""
    @bicubic_sampling.setter
    def bicubic_sampling(self, value: bool) -> None: ...

class DefaultOpaqueRendererMethod(Resource):
    """Default opaque rendering method resource.

    Controls the default rendering method (forward or deferred)
    used for opaque materials that specify OpaqueRenderMethod.Auto.
    """

    def __init__(self, method: OpaqueRenderMethod = ...) -> None: ...

    @staticmethod
    def forward() -> DefaultOpaqueRendererMethod:
        """Create a resource configured for forward rendering."""

    @staticmethod
    def deferred() -> DefaultOpaqueRendererMethod:
        """Create a resource configured for deferred rendering."""

    def set_to_forward(self) -> None:
        """Set rendering method to forward."""

    def set_to_deferred(self) -> None:
        """Set rendering method to deferred."""

class AtmosphereMode:
    """Rendering mode for atmospheric scattering.

    Controls how the atmosphere is rendered - either using lookup textures
    for performance or raymarching for accuracy.
    """

    LookupTexture: ClassVar[AtmosphereMode]
    """High-performance mode using lookup textures."""
    Raymarched: ClassVar[AtmosphereMode]
    """More accurate raymarching mode."""
    LOOKUP_TEXTURE: ClassVar[AtmosphereMode]
    RAYMARCHED: ClassVar[AtmosphereMode]

class ScatteringMedium(Asset):
    """Asset defining how a material scatters light.

    In Bevy 0.18, atmospheric scattering parameters (rayleigh, mie, ozone)
    moved from Atmosphere into ScatteringMedium assets. Atmosphere now
    references a Handle[ScatteringMedium].

    Example:
        >>> from pybevy.pbr import ScatteringMedium
        >>> medium = ScatteringMedium()  # default (earth-like) medium
        >>> earth_medium = ScatteringMedium.earthlike()
    """

    def __init__(self) -> None: ...

    @staticmethod
    def earthlike(
        falloff_resolution: int = 256,
        phase_resolution: int = 256,
    ) -> ScatteringMedium:
        """Create an earth-like scattering medium preset."""

class Atmosphere(Component):
    """Atmospheric scattering component for realistic sky rendering.

    When added to a camera, enables procedural atmospheric scattering that
    simulates realistic sky colors, sunsets, and aerial perspective. Based on
    Hillaire's 2020 paper on real-time atmospheric scattering.

    In Bevy 0.18, scattering parameters (rayleigh, mie, ozone) moved to
    ScatteringMedium assets. Atmosphere now references a Handle to a
    ScatteringMedium.

    Example:
        >>> from pybevy.pbr import Atmosphere
        >>> # Create with a ScatteringMedium handle
        >>> atmo = Atmosphere(medium=medium_handle)
        >>> # Or use earthlike preset
        >>> atmo = Atmosphere.earthlike(medium_handle)
    """

    def __init__(
        self,
        medium: Handle[ScatteringMedium],
        bottom_radius: float,
        top_radius: float,
        ground_albedo: Vec3,
    ) -> None: ...

    @staticmethod
    def earthlike(medium: Handle[ScatteringMedium]) -> Atmosphere:
        """Create an earth-like atmosphere with the given ScatteringMedium handle."""

    @property
    def medium(self) -> Handle[ScatteringMedium]:
        """Handle to the ScatteringMedium asset."""
    @medium.setter
    def medium(self, value: Handle[ScatteringMedium]) -> None: ...

    @property
    def bottom_radius(self) -> float:
        """Radius of the planet in meters."""
    @bottom_radius.setter
    def bottom_radius(self, value: float) -> None: ...

    @property
    def top_radius(self) -> float:
        """Radius at which atmosphere ends in meters (from planet center)."""
    @top_radius.setter
    def top_radius(self, value: float) -> None: ...

    @property
    def ground_albedo(self) -> Vec3:
        """Average surface albedo for multiscattering calculations."""
    @ground_albedo.setter
    def ground_albedo(self, value: Vec3) -> None: ...

class AtmosphereSettings(Component):
    """Configuration for atmosphere LUT (Look-Up Table) resolution and sampling.

    Controls the quality/performance tradeoff of atmospheric scattering.
    Higher resolutions and sample counts produce better quality but cost more
    to compute.
    """

    def __init__(
        self,
        transmittance_lut_size: UVec2 = ...,
        multiscattering_lut_size: UVec2 = ...,
        sky_view_lut_size: UVec2 = ...,
        aerial_view_lut_size: UVec3 = ...,
        transmittance_lut_samples: int = 40,
        multiscattering_lut_dirs: int = 64,
        multiscattering_lut_samples: int = 20,
        sky_view_lut_samples: int = 16,
        aerial_view_lut_samples: int = 10,
        aerial_view_lut_max_distance: float = 32000.0,
        scene_units_to_m: float = 1.0,
        sky_max_samples: int = 16,
        rendering_method: AtmosphereMode = ...,
    ) -> None: ...

    @property
    def transmittance_lut_size(self) -> UVec2:
        """Size of transmittance LUT texture."""
    @transmittance_lut_size.setter
    def transmittance_lut_size(self, value: UVec2) -> None: ...

    @property
    def multiscattering_lut_size(self) -> UVec2:
        """Size of multiscattering LUT texture."""
    @multiscattering_lut_size.setter
    def multiscattering_lut_size(self, value: UVec2) -> None: ...

    @property
    def sky_view_lut_size(self) -> UVec2:
        """Size of sky view LUT texture."""
    @sky_view_lut_size.setter
    def sky_view_lut_size(self, value: UVec2) -> None: ...

    @property
    def aerial_view_lut_size(self) -> UVec3:
        """Size of aerial view LUT texture (3D)."""
    @aerial_view_lut_size.setter
    def aerial_view_lut_size(self, value: UVec3) -> None: ...

    @property
    def transmittance_lut_samples(self) -> int:
        """Sample count for transmittance LUT computation."""
    @transmittance_lut_samples.setter
    def transmittance_lut_samples(self, value: int) -> None: ...

    @property
    def multiscattering_lut_dirs(self) -> int:
        """Number of ray directions for multiscattering LUT."""
    @multiscattering_lut_dirs.setter
    def multiscattering_lut_dirs(self, value: int) -> None: ...

    @property
    def multiscattering_lut_samples(self) -> int:
        """Sample count for multiscattering ray integration."""
    @multiscattering_lut_samples.setter
    def multiscattering_lut_samples(self, value: int) -> None: ...

    @property
    def sky_view_lut_samples(self) -> int:
        """Sample count for sky view LUT computation."""
    @sky_view_lut_samples.setter
    def sky_view_lut_samples(self, value: int) -> None: ...

    @property
    def aerial_view_lut_samples(self) -> int:
        """Sample count for aerial view LUT computation."""
    @aerial_view_lut_samples.setter
    def aerial_view_lut_samples(self, value: int) -> None: ...

    @property
    def aerial_view_lut_max_distance(self) -> float:
        """Maximum distance for aerial view LUT in meters."""
    @aerial_view_lut_max_distance.setter
    def aerial_view_lut_max_distance(self, value: float) -> None: ...

    @property
    def scene_units_to_m(self) -> float:
        """Conversion factor from scene units to meters."""
    @scene_units_to_m.setter
    def scene_units_to_m(self, value: float) -> None: ...

    @property
    def sky_max_samples(self) -> int:
        """Maximum samples for raymarched sky rendering."""
    @sky_max_samples.setter
    def sky_max_samples(self, value: int) -> None: ...

    @property
    def rendering_method(self) -> AtmosphereMode:
        """Rendering method: LookupTexture or Raymarched."""
    @rendering_method.setter
    def rendering_method(self, value: AtmosphereMode) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        transmittance_lut_samples: np.ndarray | None = None,
        multiscattering_lut_dirs: np.ndarray | None = None,
        multiscattering_lut_samples: np.ndarray | None = None,
        sky_view_lut_samples: np.ndarray | None = None,
        aerial_view_lut_samples: np.ndarray | None = None,
        aerial_view_lut_max_distance: np.ndarray | None = None,
        scene_units_to_m: np.ndarray | None = None,
        sky_max_samples: np.ndarray | None = None,
    ) -> Batchable: ...

class ShaderMaterialPlugin(Plugin):
    """Plugin that registers the ShaderMaterial type.

    Must be added before creating ShaderMaterial assets.

    Example:
        ```python
        app.add_plugins(ShaderMaterialPlugin())
        ```
    """

    def __init__(self) -> None: ...

class ShaderMaterial(Asset):
    """A material that extends StandardMaterial with a per-instance custom shader.

    Each ShaderMaterial instance can specify its own fragment/vertex shader,
    plus uniform parameters accessible from WGSL via a flat data buffer.
    Supports up to 4 texture slots and 32 shader def bits.

    Typically used indirectly via the ``@material`` decorator, which provides
    typed field access. Direct use is for advanced cases.

    Args:
        base: The StandardMaterial providing PBR base properties.
        fragment_shader: Asset path to a custom fragment shader.
        vertex_shader: Asset path to a custom vertex shader.
        data: Flat list of up to 256 floats for the uniform buffer at binding 100.
        shader_defs: Bitmask of active shader defs (up to 32 bits).
        shader_def_names: Names for each bit (bit 0 = names[0], etc.).
        textures: List of up to 4 texture handles (or None per slot).

    Example:
        ```python
        # Prefer using @material decorator instead of this directly:
        @material(fragment_shader="shaders/hologram.wgsl")
        class HologramMaterial:
            energy: float = 1.0

        # Direct use:
        mat = ShaderMaterial(
            base=StandardMaterial(base_color=Color.srgb(1.0, 0.0, 0.0)),
            fragment_shader="shaders/hologram.wgsl",
            data=[1.5, 0.0, 0.0, 0.7],
        )
        ```
    """

    def __init__(
        self,
        base: StandardMaterial,
        fragment_shader: str | None = None,
        vertex_shader: str | None = None,
        data: list[float] | None = None,
        shader_defs: int = 0,
        shader_def_names: list[str] | None = None,
        textures: list[Handle[Image] | None] | None = None,
        bindings_wgsl: str | None = None,
    ) -> None: ...

    def set_data_float(self, index: int, value: float) -> None:
        """Set a single float in the uniform data buffer (index 0-255)."""

    def get_data_float(self, index: int) -> float:
        """Get a single float from the uniform data buffer (index 0-255)."""

    def set_data_floats(self, start: int, values: list[float]) -> None:
        """Set multiple floats starting at the given index."""

    def get_data_floats(self, start: int, count: int) -> list[float]:
        """Get multiple floats starting at the given index."""

    def get_shader_defs(self) -> int:
        """Get the shader defs bitmask."""

    def set_shader_defs(self, defs: int) -> None:
        """Set the shader defs bitmask."""

    def set_texture(self, slot: int, handle: Handle[Image]) -> None:
        """Set a texture handle for a given slot (0-3)."""

    def clear_texture(self, slot: int) -> None:
        """Clear a texture slot (set to None)."""

class ForwardDecal(Component):
    """Marker component for forward-rendered decals."""

    def __init__(self) -> None: ...
    def __eq__(self, other: object) -> bool: ...

class Falloff:
    """Falloff mode controlling intensity decay over distance."""

    @staticmethod
    def linear() -> Falloff:
        """Linear falloff."""

    @staticmethod
    def exponential(scale: float) -> Falloff:
        """Exponential falloff with given scale."""

    @staticmethod
    def tent(center: float, width: float) -> Falloff:
        """Tent-shaped falloff with given center and width."""

class MeshMaterial3dShader(Component):
    """Component that assigns a ShaderMaterial to a mesh entity.

    Usually not used directly -- ``MeshMaterial3d[YourMaterial]`` resolves
    to this automatically for ``@material``-decorated classes.

    Example:
        ```python
        # Preferred (via @material):
        commands.spawn(Mesh3d(mesh), MeshMaterial3d[HologramMaterial](handle))

        # Direct:
        commands.spawn(Mesh3d(mesh), MeshMaterial3dShader(handle))
        ```
    """

    def __init__(self, handle: Handle[ShaderMaterial]) -> None: ...

    def handle(self) -> Handle[ShaderMaterial]:
        """Get the shader material handle."""

    def __eq__(self, other: object) -> bool: ...
