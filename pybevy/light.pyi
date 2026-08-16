import math
from typing import ClassVar, Literal

import numpy as np

from pybevy.app import App, Plugin
from pybevy.assets import Asset, Handle
from pybevy.camera import CubemapLayout
from pybevy.collections import LiveList
from pybevy.color import Color
from pybevy.ecs import Batchable, Component, Resource
from pybevy.gizmos import GizmoConfigGroup
from pybevy.image import Image
from pybevy.math import Mat4, Quat, UVec2, Vec3

class LightPlugin(Plugin):
    """Light plugin for ambient and directional lighting.

    Note: Lighting is included in PbrPlugin. This plugin is optional
    and only adds default ambient light if not present.
    """
    def __init__(self) -> None: ...
    def build(self, app: App) -> None: ...

class LightGizmoColor:
    """Selects how a light gizmo is colored."""

    class Manual(LightGizmoColor):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: Color
        def __init__(self, value: Color) -> None: ...

    class Varied(LightGizmoColor):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class MatchLightColor(LightGizmoColor):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class ByLightType(LightGizmoColor):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

class LightGizmoConfigGroup(GizmoConfigGroup):
    """Configuration and selector for Bevy's light gizmo group."""

    def __init__(
        self,
        draw_all: bool = False,
        color: LightGizmoColor = LightGizmoColor.MatchLightColor(),
        point_light_color: Color = ...,
        spot_light_color: Color = ...,
        directional_light_color: Color = ...,
        rect_light_color: Color = ...,
    ) -> None: ...
    draw_all: bool
    color: LightGizmoColor
    point_light_color: Color
    spot_light_color: Color
    directional_light_color: Color
    rect_light_color: Color

class ShowLightGizmo(Component):
    """Draw a gizmo for light components on this entity."""

    def __init__(self, color: LightGizmoColor | None = None) -> None: ...
    color: LightGizmoColor | None

class NotShadowCaster(Component):
    """Prevents a mesh from casting shadows."""
    def __init__(self) -> None: ...
    def __eq__(self, other: NotShadowCaster) -> bool: ...  # type: ignore[override]

class NotShadowReceiver(Component):
    """Prevents a mesh from receiving shadows."""
    def __init__(self) -> None: ...

class TransmittedShadowReceiver(Component):
    """Enables shadows on the backside (transmission lobe) of a mesh with diffuse transmission."""
    def __init__(self) -> None: ...

class VolumetricLight(Component):
    """Marks a light source as volumetric, causing it to illuminate fog volumes."""
    def __init__(self) -> None: ...

class ShadowFilteringMethod(Component):
    """Controls the shadow filtering algorithm used for a camera.

    Use as a component on camera entities.
    """
    def __init__(self) -> None: ...

    HARDWARE_2X2: ShadowFilteringMethod
    """Fast but poor quality shadow filtering."""

    GAUSSIAN: ShadowFilteringMethod
    """Approximates a fixed Gaussian blur. Good when TAA isn't in use. (Default)"""

    TEMPORAL: ShadowFilteringMethod
    """Randomized filter that varies over time. Best when TAA is enabled."""

    def __eq__(self, other: ShadowFilteringMethod) -> bool: ...  # type: ignore[override]

class PointLightShadowMap(Resource):
    """Controls the shadow map size for point lights.

    Example:
        app.insert_resource(PointLightShadowMap(size=2048))
    """
    def __init__(self, size: int = 1024) -> None: ...
    size: int

class DirectionalLightShadowMap(Resource):
    """Controls the shadow map size for directional lights.

    Example:
        app.insert_resource(DirectionalLightShadowMap(size=4096))
    """
    def __init__(self, size: int = 2048) -> None: ...
    size: int

class FogVolume(Component):
    """A volumetric fog volume that creates atmospheric effects.

    FogVolume creates localized fog effects that interact with volumetric lights.
    Unlike global fog, FogVolume is attached to an entity and affected by its transform.

    Args:
        fog_color: The color of the fog when illuminated.
        density_factor: How dark/dense the fog appears.
        density_texture: Optional 3D texture for varying density.
        density_texture_offset: UVW offset for scrolling the density texture.
        absorption: How much light is absorbed per step.
        scattering: How much light is scattered at each step.
        scattering_asymmetry: Forward vs backward scattering bias.
        light_tint: Non-physical color tint applied to light.
        light_intensity: Light intensity multiplier.
    """
    def __init__(
        self,
        fog_color: Color = Color.WHITE,
        density_factor: float = 0.1,
        density_texture: Handle[Image] | None = None,
        density_texture_offset: Vec3 = ...,
        absorption: float = 0.3,
        scattering: float = 0.3,
        scattering_asymmetry: float = 0.5,
        light_tint: Color = Color.WHITE,
        light_intensity: float = 1.0,
    ) -> None: ...

    fog_color: Color
    density_factor: float
    density_texture: Handle[Image] | None
    density_texture_offset: Vec3
    absorption: float
    scattering: float
    scattering_asymmetry: float
    light_tint: Color
    light_intensity: float

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        density_factor: np.typing.ArrayLike | None = None,
        absorption: np.typing.ArrayLike | None = None,
        scattering: np.typing.ArrayLike | None = None,
        scattering_asymmetry: np.typing.ArrayLike | None = None,
        light_intensity: np.typing.ArrayLike | None = None,
        fog_color: np.typing.ArrayLike | None = None,
        light_tint: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class GlobalAmbientLight(Resource):
    """Global ambient light resource.

    In Bevy 0.18, the global ambient light is a Resource that applies to all cameras
    unless overridden by an AmbientLight component on the camera entity.
    """
    def __init__(
        self,
        color: Color = Color.WHITE,
        brightness: float = 80.0,
        affects_lightmapped_meshes: bool = True,
    ) -> None: ...

    color: Color
    brightness: float
    affects_lightmapped_meshes: bool

class AmbientLight(Component):
    """Per-camera ambient light component.

    In Bevy 0.18, AmbientLight is a Component that can be added to camera entities
    to override the GlobalAmbientLight resource for that specific camera.
    """
    def __init__(
        self,
        color: Color = Color.WHITE,
        brightness: float = 80.0,
        affects_lightmapped_meshes: bool = True,
    ) -> None: ...

    color: Color
    brightness: float
    affects_lightmapped_meshes: bool

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        brightness: np.typing.ArrayLike | None = None,
        affects_lightmapped_meshes: np.typing.ArrayLike | None = None,
        color: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class PointLight(Component):
    def __init__(
        self,
        color: Color = Color.WHITE,
        intensity: float = 1_000_000.0,
        range: float = 20.0,
        radius: float = 0.0,
        shadow_maps_enabled: bool = False,
        contact_shadows_enabled: bool = False,
        affects_lightmapped_mesh_diffuse: bool = True,
        shadow_depth_bias: float = 0.08,
        shadow_normal_bias: float = 0.6,
        shadow_map_near_z: float = 0.1,
    ) -> None: ...

    color: Color
    intensity: float
    range: float
    radius: float
    shadow_maps_enabled: bool
    contact_shadows_enabled: bool
    affects_lightmapped_mesh_diffuse: bool
    shadow_depth_bias: float
    shadow_normal_bias: float
    shadow_map_near_z: float

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        range: np.typing.ArrayLike | None = None,
        radius: np.typing.ArrayLike | None = None,
        shadow_depth_bias: np.typing.ArrayLike | None = None,
        shadow_normal_bias: np.typing.ArrayLike | None = None,
        shadow_map_near_z: np.typing.ArrayLike | None = None,
        shadow_maps_enabled: np.typing.ArrayLike | None = None,
        contact_shadows_enabled: np.typing.ArrayLike | None = None,
        affects_lightmapped_mesh_diffuse: np.typing.ArrayLike | None = None,
        color: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class SpotLight(Component):
    def __init__(
        self,
        color: Color = Color.WHITE,
        intensity: float = 1_000_000.0,
        range: float = 20.0,
        radius: float = 0.0,
        shadow_maps_enabled: bool = False,
        contact_shadows_enabled: bool = False,
        affects_lightmapped_mesh_diffuse: bool = True,
        shadow_depth_bias: float = 0.02,
        shadow_normal_bias: float = 1.8,
        shadow_map_near_z: float = 0.1,
        outer_angle: float = math.pi / 4,
        inner_angle: float = 0.0,
    ) -> None: ...

    color: Color
    intensity: float
    range: float
    radius: float
    shadow_maps_enabled: bool
    contact_shadows_enabled: bool
    affects_lightmapped_mesh_diffuse: bool
    shadow_depth_bias: float
    shadow_normal_bias: float
    shadow_map_near_z: float
    outer_angle: float
    inner_angle: float

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        range: np.typing.ArrayLike | None = None,
        radius: np.typing.ArrayLike | None = None,
        shadow_depth_bias: np.typing.ArrayLike | None = None,
        shadow_normal_bias: np.typing.ArrayLike | None = None,
        shadow_map_near_z: np.typing.ArrayLike | None = None,
        outer_angle: np.typing.ArrayLike | None = None,
        inner_angle: np.typing.ArrayLike | None = None,
        shadow_maps_enabled: np.typing.ArrayLike | None = None,
        contact_shadows_enabled: np.typing.ArrayLike | None = None,
        affects_lightmapped_mesh_diffuse: np.typing.ArrayLike | None = None,
        color: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class DirectionalLight(Component):
    def __init__(
        self,
        color: Color = Color.WHITE,
        illuminance: float = 10_000.0,
        shadow_maps_enabled: bool = False,
        contact_shadows_enabled: bool = False,
        affects_lightmapped_mesh_diffuse: bool = True,
        shadow_depth_bias: float = 0.02,
        shadow_normal_bias: float = 1.8,
    ) -> None: ...

    color: Color
    illuminance: float
    shadow_maps_enabled: bool
    contact_shadows_enabled: bool
    affects_lightmapped_mesh_diffuse: bool
    shadow_depth_bias: float
    shadow_normal_bias: float

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        illuminance: np.typing.ArrayLike | None = None,
        shadow_depth_bias: np.typing.ArrayLike | None = None,
        shadow_normal_bias: np.typing.ArrayLike | None = None,
        shadow_maps_enabled: np.typing.ArrayLike | None = None,
        contact_shadows_enabled: np.typing.ArrayLike | None = None,
        affects_lightmapped_mesh_diffuse: np.typing.ArrayLike | None = None,
        color: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class RectLight(Component):
    """Rectangular area light.

    The rectangle lies in the entity's local XY plane (sized by ``width`` and
    ``height``) and emits along local -Z, so aim it with ``looking_at`` like a
    spotlight. Objects it illuminates do not cast shadows (no shadow-map
    support upstream yet). Rendering uses the engine's ``area_light_luts``
    feature, which pybevy builds enable.
    """

    def __init__(
        self,
        color: Color = Color.WHITE,
        intensity: float = 1_000_000.0,
        range: float = 20.0,
        width: float = 1.0,
        height: float = 1.0,
    ) -> None: ...

    color: Color
    intensity: float
    range: float
    width: float
    height: float

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        range: np.typing.ArrayLike | None = None,
        width: np.typing.ArrayLike | None = None,
        height: np.typing.ArrayLike | None = None,
        color: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class ParallaxCorrection(Component):
    """Parallax correction mode for reflection (light) probes."""

    class None_(ParallaxCorrection):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Auto(ParallaxCorrection):
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Custom(ParallaxCorrection):
        __match_args__: ClassVar[tuple[Literal["value"]]]
        value: Vec3
        def __init__(self, value: Vec3) -> None: ...

class EnvironmentMapLight(Component):
    def __init__(
        self,
        diffuse_map: Handle[Image] | None = None,
        specular_map: Handle[Image] | None = None,
        intensity: float = 0.0,
        rotation: Quat = ...,
        affects_lightmapped_mesh_diffuse: bool = True,
    ) -> None: ...

    diffuse_map: Handle[Image]
    specular_map: Handle[Image]
    intensity: float
    rotation: Quat
    affects_lightmapped_mesh_diffuse: bool

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        affects_lightmapped_mesh_diffuse: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class VolumetricFog(Component):
    def __init__(
        self,
        ambient_color: Color = Color.WHITE,
        ambient_intensity: float = 0.1,
        step_count: int = 64,
        jitter: float = 0.0,
    ) -> None: ...

    ambient_color: Color
    ambient_intensity: float
    step_count: int
    jitter: float

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        ambient_intensity: np.typing.ArrayLike | None = None,
        step_count: np.typing.ArrayLike | None = None,
        jitter: np.typing.ArrayLike | None = None,
        ambient_color: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class CascadeShadowConfig(Component):
    """
    Controls cascaded shadow mapping for directional lights.

    Cascaded shadow maps divide the view frustum into multiple shadow maps,
    with near cascades having higher resolution than far cascades.
    This significantly improves shadow quality by reducing perspective aliasing.
    """
    def __init__(
        self,
        bounds: list[float] = ...,
        overlap_proportion: float = 0.2,
        minimum_distance: float = 0.1,
    ) -> None: ...

    @property
    def bounds(self) -> LiveList[float]: ...
    @bounds.setter
    def bounds(self, value: list[float]) -> None: ...
    overlap_proportion: float
    minimum_distance: float

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        overlap_proportion: np.typing.ArrayLike | None = None,
        minimum_distance: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class Cascade:
    @property
    def world_from_cascade(self) -> Mat4: ...
    @property
    def clip_from_cascade(self) -> Mat4: ...
    @property
    def clip_from_world(self) -> Mat4: ...
    @property
    def texel_size(self) -> float: ...

class Cascades(Component):
    """Engine-managed component containing shadow cascades (read-only, cannot be constructed)."""

    @property
    def cascades(self) -> dict[int, list[Cascade]]: ...

class LightProbe(Component):
    """A component for a light probe.

    A light probe is a cuboid region that provides global illumination to all
    fragments inside it. Requires Transform and Visibility components.

    Has no effect unless paired with EnvironmentMapLight or IrradianceVolume.
    """
    def __init__(self, falloff: Vec3 = ...) -> None: ...

    @property
    def falloff(self) -> Vec3:
        """Falloff applied at the edges of the light probe's region."""
    @falloff.setter
    def falloff(self, value: Vec3) -> None: ...

class IrradianceVolume(Component):
    """A light probe using irradiance volumes for diffuse global illumination.

    Irradiance volumes use a 3D texture of ambient cubes to provide efficient
    diffuse global illumination. Requires LightProbe component.

    Args:
        voxels: The 3D texture representing ambient cubes (optional, defaults to invalid handle).
        intensity: Light intensity multiplier.
        affects_lightmapped_meshes: Whether to affect lightmapped meshes.
    """
    def __init__(
        self,
        voxels: Handle[Image] | None = None,
        intensity: float = 0.0,
        affects_lightmapped_meshes: bool = True,
    ) -> None: ...

    voxels: Handle[Image]
    intensity: float
    affects_lightmapped_meshes: bool

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        affects_lightmapped_meshes: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class SunDisk(Component):
    """Controls the visible solar disk appearance for DirectionalLight.

    When added to a DirectionalLight entity, renders a visible sun disk in
    the sky. Requires an Atmosphere component on the Camera3d for rendering.

    Args:
        angular_size: The angular diameter of the sun disk in radians.
        intensity: Brightness multiplier (0.0 disables, 1.0 is physical default).

    Example:
        commands.spawn(DirectionalLight(), SunDisk.EARTH)
    """
    def __init__(
        self,
        angular_size: float = 0.00930842,
        intensity: float = 1.0,
    ) -> None: ...

    EARTH: ClassVar[SunDisk]
    """Earth's sun disk with realistic angular size."""

    OFF: ClassVar[SunDisk]
    """Disabled sun disk (zero size and intensity)."""

    angular_size: float
    intensity: float

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        angular_size: np.typing.ArrayLike | None = None,
        intensity: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class DirectionalLightTexture(Component):
    """Adds a texture mask effect to a DirectionalLight.

    The texture modulates the light's intensity, simulating patterns like
    window shadows, gobo/cookie effects, or soft falloffs.

    Args:
        image: The texture image. Only the R channel is read.
        tiled: Whether to tile the image infinitely.
    """
    def __init__(
        self,
        image: Handle[Image],
        tiled: bool,
    ) -> None: ...

    image: Handle[Image]
    tiled: bool

class SpotLightTexture(Component):
    """Adds a texture mask effect to a SpotLight.

    The texture modulates the light's intensity, simulating patterns like
    window shadows, gobo/cookie effects, or soft falloffs.

    Args:
        image: The texture image. Only the R channel is read.
    """
    def __init__(self, image: Handle[Image]) -> None: ...

    image: Handle[Image]

class PointLightTexture(Component):
    """Adds a texture mask effect to a PointLight.

    The texture modulates the light's intensity, simulating patterns like
    window shadows, gobo/cookie effects, or soft falloffs.

    Args:
        image: The texture image. Only the R channel is read.
        cubemap_layout: The layout of the cubemap texture.
    """
    def __init__(
        self,
        image: Handle[Image],
        cubemap_layout: CubemapLayout,
    ) -> None: ...

    image: Handle[Image]
    cubemap_layout: CubemapLayout

class GeneratedEnvironmentMapLight(Component):
    """A generated environment map that is filtered at runtime.

    Use this when you have an HDR cubemap that needs GPU filtering
    to generate specular and diffuse maps.

    The source cubemap must be square, power-of-two, and at most 8192 wide.
    Bevy checks this in a system on the frame the image finishes loading, so a
    violation panics and terminates the app rather than raising at the spawn
    call. Catching the panic does not help: the check re-runs every frame.

    Args:
        environment_map: Source cubemap to filter (square, power-of-two, <= 8192).
        intensity: Light intensity in cd/m².
        rotation: World-space rotation applied to the cubemap.
        affects_lightmapped_mesh_diffuse: Whether to affect lightmapped meshes.
    """
    def __init__(
        self,
        environment_map: Handle[Image] | None = None,
        intensity: float = 0.0,
        rotation: Quat = ...,
        affects_lightmapped_mesh_diffuse: bool = True,
    ) -> None: ...

    environment_map: Handle[Image]
    intensity: float
    rotation: Quat
    affects_lightmapped_mesh_diffuse: bool

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        affects_lightmapped_mesh_diffuse: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class AtmosphereEnvironmentMapLight(Component):
    """Uses the atmosphere to generate environment lighting.

    Attach to a Camera3d to light the entire view, or to a LightProbe
    to light only a specific region. Generates an environment map from
    the atmosphere for image-based lighting.

    Args:
        intensity: Controls how bright the atmosphere's environment lighting is.
        affects_lightmapped_mesh_diffuse: Whether diffuse affects lightmapped meshes.
        size: Cubemap resolution in pixels (must be power-of-two).
    """
    def __init__(
        self,
        intensity: float = 1.0,
        affects_lightmapped_mesh_diffuse: bool = True,
        size: UVec2 = ...,
    ) -> None: ...

    intensity: float
    affects_lightmapped_mesh_diffuse: bool
    size: UVec2

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        intensity: np.typing.ArrayLike | None = None,
        affects_lightmapped_mesh_diffuse: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class ClusteredDecal(Component):
    """A clustered decal that projects an image onto surfaces.

    Decals are rendered using the clustered forward rendering system
    and are blended with the underlying surface.

    Args:
        base_color_texture: The base color image to project.
        normal_map_texture: The optional normal map image.
        metallic_roughness_texture: The optional metallic-roughness image.
        emissive_texture: The optional emissive image.
        tag: An application-specific tag for custom filtering.
    """
    def __init__(
        self,
        base_color_texture: Handle[Image] | None = None,
        normal_map_texture: Handle[Image] | None = None,
        metallic_roughness_texture: Handle[Image] | None = None,
        emissive_texture: Handle[Image] | None = None,
        tag: int = 0,
    ) -> None: ...

    @property
    def base_color_texture(self) -> Handle[Image] | None:
        """The base color texture to project."""
    @base_color_texture.setter
    def base_color_texture(self, value: Handle[Image] | None) -> None: ...

    @property
    def normal_map_texture(self) -> Handle[Image] | None: ...
    @normal_map_texture.setter
    def normal_map_texture(self, value: Handle[Image] | None) -> None: ...

    @property
    def metallic_roughness_texture(self) -> Handle[Image] | None: ...
    @metallic_roughness_texture.setter
    def metallic_roughness_texture(self, value: Handle[Image] | None) -> None: ...

    @property
    def emissive_texture(self) -> Handle[Image] | None: ...
    @emissive_texture.setter
    def emissive_texture(self, value: Handle[Image] | None) -> None: ...

    @property
    def tag(self) -> int:
        """Application-specific tag for custom filtering."""
    @tag.setter
    def tag(self, value: int) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        tag: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...

class ScatteringMedium(Asset):
    """Asset defining how a material scatters light.

    Atmospheric scattering parameters (rayleigh, mie, ozone) live in
    ScatteringMedium assets; Atmosphere references a Handle[ScatteringMedium].

    Example:
        >>> from pybevy.light import ScatteringMedium
        >>> medium = ScatteringMedium()  # default (earth-like) medium
        >>> earth_medium = ScatteringMedium.earth()
        >>> custom = ScatteringMedium(terms=[ScatteringTerm()])
    """

    def __init__(
        self,
        falloff_resolution: int = 256,
        phase_resolution: int = 256,
        terms: list[ScatteringTerm] | None = None,
    ) -> None:
        """Create a medium from scattering terms (mirrors ScatteringMedium::new).

        When terms is None, returns bevy's default earth-like medium.
        """

    @property
    def label(self) -> str | None:
        """Optional label used when creating the LUTs on the GPU."""
    @label.setter
    def label(self, value: str | None) -> None: ...

    @property
    def falloff_resolution(self) -> int:
        """Resolution at which to sample each term's falloff distribution."""
    @falloff_resolution.setter
    def falloff_resolution(self, value: int) -> None: ...

    @property
    def phase_resolution(self) -> int:
        """Resolution at which to sample each term's phase function."""
    @phase_resolution.setter
    def phase_resolution(self, value: int) -> None: ...

    @property
    def terms(self) -> LiveList[ScatteringTerm]:
        """Live indexed terms for a stored medium; owned/read-only terms reject writes."""

    @terms.setter
    def terms(self, value: list[ScatteringTerm]) -> None: ...

    @staticmethod
    def earth(
        falloff_resolution: int = 256,
        phase_resolution: int = 256,
    ) -> ScatteringMedium:
        """Create an earth-like scattering medium preset."""

    @staticmethod
    def mars(
        falloff_resolution: int = 256,
        phase_resolution: int = 256,
        *,
        dust_phase: Handle[Image],
    ) -> ScatteringMedium:
        """Create a mars-like scattering medium preset (requires a dust-phase image)."""

class Atmosphere(Component):
    """Atmosphere of a planet, for physically-based sky rendering.

    Spawn it on its OWN entity: the entity's GlobalTransform is the planet
    center, and left at default it is placed inner_radius below the origin so
    the scene sits on the planet surface (mirroring Bevy's examples). Cameras
    opt in by adding pybevy.pbr.AtmosphereSettings; the nearest Atmosphere is
    used. Without AtmosphereSettings the sky silently does not render.

    Scattering parameters (rayleigh, mie, ozone) live in ScatteringMedium
    assets; Atmosphere references a Handle to a ScatteringMedium. Based on
    Hillaire's 2020 paper on real-time atmospheric scattering.

    Example:
        >>> from pybevy.light import Atmosphere
        >>> from pybevy.pbr import AtmosphereSettings
        >>> commands.spawn(Atmosphere.earth(medium_handle))  # planet entity
        >>> commands.spawn(Camera3d(), AtmosphereSettings())
    """

    def __init__(
        self,
        inner_radius: float,
        outer_radius: float,
        ground_albedo: Vec3,
        medium: Handle[ScatteringMedium],
    ) -> None: ...

    @staticmethod
    def earth(medium: Handle[ScatteringMedium]) -> Atmosphere:
        """Create an earth-like atmosphere with the given ScatteringMedium handle."""

    @staticmethod
    def mars(medium: Handle[ScatteringMedium]) -> Atmosphere:
        """Create a mars-like atmosphere with the given ScatteringMedium handle."""

    @property
    def medium(self) -> Handle[ScatteringMedium]:
        """Handle to the ScatteringMedium asset."""
    @medium.setter
    def medium(self, value: Handle[ScatteringMedium]) -> None: ...

    @property
    def inner_radius(self) -> float:
        """Radius of the planet in meters."""
    @inner_radius.setter
    def inner_radius(self, value: float) -> None: ...

    @property
    def outer_radius(self) -> float:
        """Radius at which atmosphere ends in meters (from planet center)."""
    @outer_radius.setter
    def outer_radius(self, value: float) -> None: ...

    @property
    def ground_albedo(self) -> Vec3:
        """Average surface albedo for multiscattering calculations."""
    @ground_albedo.setter
    def ground_albedo(self, value: Vec3) -> None: ...

class Falloff:
    """Falloff mode controlling intensity decay over distance.

    Bevy's Curve variant holds a Rust callback and cannot be constructed from Python.
    """

    class Linear(Falloff):
        """Linear falloff."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Exponential(Falloff):
        """Exponential falloff with given scale."""
        __match_args__: ClassVar[tuple[Literal["scale"]]]
        scale: float
        def __init__(self, scale: float) -> None: ...

    class Tent(Falloff):
        """Tent-shaped falloff with given center and width."""
        __match_args__: ClassVar[tuple[Literal["center"], Literal["width"]]]
        center: float
        width: float
        def __init__(self, center: float, width: float) -> None: ...

class PhaseFunction:
    """Phase function describing how a ScatteringTerm scatters light in different directions.

    Bevy's Curve and ChromaticCurve variants hold Rust closures and cannot be
    constructed from Python.
    """

    class Isotropic(PhaseFunction):
        """Scatters light evenly in all directions."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Rayleigh(PhaseFunction):
        """Rayleigh scattering (particles much smaller than visible wavelengths)."""
        __match_args__: ClassVar[tuple[()]]
        def __init__(self) -> None: ...

    class Mie(PhaseFunction):
        """Henyey-Greenstein approximation of Mie scattering."""
        __match_args__: ClassVar[tuple[Literal["asymmetry"]]]
        asymmetry: float
        def __init__(self, asymmetry: float) -> None: ...

    class ChromaticTexture(PhaseFunction):
        """Chromatic phase function sampled from an Nx1 Rgba32Float texture."""
        __match_args__: ClassVar[tuple[Literal["image"]]]
        image: Handle[Image]
        def __init__(self, image: Handle[Image]) -> None: ...

class ScatteringTerm:
    """An individual element of a ScatteringMedium."""

    def __init__(
        self,
        absorption: Vec3 = ...,
        scattering: Vec3 = ...,
        falloff: Falloff = ...,
        phase: PhaseFunction = ...,
    ) -> None:
        """Defaults mirror bevy: zero densities, linear falloff, Mie(asymmetry=0.8)."""

    @property
    def absorption(self) -> Vec3:
        """Optical absorption density per meter."""
    @absorption.setter
    def absorption(self, value: Vec3) -> None: ...

    @property
    def scattering(self) -> Vec3:
        """Optical scattering density per meter."""
    @scattering.setter
    def scattering(self, value: Vec3) -> None: ...

    @property
    def falloff(self) -> Falloff:
        """Falloff distribution of this term."""
    @falloff.setter
    def falloff(self, value: Falloff) -> None: ...

    @property
    def phase(self) -> PhaseFunction:
        """Phase function of this term."""
    @phase.setter
    def phase(self, value: PhaseFunction) -> None: ...

class Skybox(Component):
    """Skybox component that displays an environment map as the background.

    When ``image`` is None (the default), the skybox is not rendered.
    """

    image: Handle[Image] | None
    brightness: float
    rotation: Quat

    def __init__(
        self,
        image: Handle[Image] | None = None,
        brightness: float = 0.0,
        rotation: Quat = ...,
    ) -> None: ...

    @staticmethod
    def from_numpy(  # type: ignore[override]
        *,
        brightness: np.typing.ArrayLike | None = None,
    ) -> Batchable: ...
