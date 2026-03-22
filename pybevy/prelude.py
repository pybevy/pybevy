from typing import Optional

from .animation import (
    AnimatableCurve,
    AnimatableKeyframeCurve,
    AnimatedField,
    AnimationClip,
    AnimationGraph,
    AnimationGraphHandle,
    AnimationGraphNode,
    AnimationNodeIndex,
    AnimationPlayer,
    AnimationPlugin,
    AnimationTransitions,
    VariableCurve,
    WeightsCurve,
)
from .app import (
    App,
    AppExit,
    DefaultPlugins,
    MinimalPlugins,
    Plugin,
    PluginGroup,
    Stage,
    TaskPoolPlugin,
    chain,
)
from .assets import AssetEvent, AssetPlugin, Assets, AssetServer, Handle
from .audio import (
    AudioPlayer,
    AudioSink,
    AudioSource,
    GlobalVolume,
    Pitch,
    PlaybackSettings,
    SpatialAudioSink,
    SpatialListener,
    Volume,
)
from .camera import (
    Bloom,
    BloomCompositeMode,
    BloomPrefilter,
    Camera,
    Camera2d,
    Camera3d,
    ClearColor,
    ClearColorConfig,
    ColorGrading,
    InheritedVisibility,
    OrthographicProjection,
    PerspectiveProjection,
    Projection,
    ScalingMode,
    Skybox,
    Tonemapping,
    ViewVisibility,
    Visibility,
)
from .color import (
    Color,
    Hsla,
    Hsva,
    Laba,
    Lcha,
    LinearRgba,
    Oklaba,
    Oklcha,
    Srgba,
    Xyza,
)
from .decorators import component, entrypoint, material, plugin, post_process, resource
from .ecs import (
    Added,
    Changed,
    ChildOf,
    Children,
    Commands,
    Component,
    DespawnOnExit,
    Entity,
    Has,
    Local,
    Message,
    MessageReader,
    Messages,
    MessageWriter,
    Mut,
    Name,
    NextState,
    On,
    OnAdd,
    OnEnter,
    OnExit,
    OnInsert,
    OnRemove,
    OnTransition,
    Query,
    Res,
    ResMut,
    Resource,
    Single,
    State,
    View,
    With,
    Without,
    World,
    in_state,
    run_if,
    state,
)
from .expr import FieldExpr
from .gltf import Gltf, GltfAssetLabel, GltfExtras
from .image import (
    Image,
    ImagePlugin,
    TextureAtlas,
    TextureAtlasLayout,
    TextureAtlasSources,
)
from .input import (
    ButtonInput,
    Gamepad,
    GamepadAxis,
    GamepadButton,
    GamepadSettings,
    KeyCode,
    MouseButton,
    Touches,
    TouchInput,
)
from .light import (
    AmbientLight,
    DirectionalLight,
    EnvironmentMapLight,
    GeneratedEnvironmentMapLight,
    GlobalAmbientLight,
    LightProbe,
    PointLight,
    SpotLight,
)
from .math import (
    Annulus,
    Capsule2d,
    Capsule3d,
    Circle,
    CircularSector,
    CircularSegment,
    Cone,
    Cuboid,
    Cylinder,
    Dir2,
    Dir3,
    EaseFunction,
    Ellipse,
    EulerRot,
    InfinitePlane3d,
    Isometry2d,
    Isometry3d,
    IVec2,
    JumpAt,
    Mat2,
    Mat3,
    Mat3A,
    Mat4,
    Plane3d,
    Quat,
    Ray2d,
    Ray3d,
    Rect,
    Rectangle,
    RegularPolygon,
    Rhombus,
    Sphere,
    Tetrahedron,
    Torus,
    Triangle2d,
    Triangle3d,
    URect,
    UVec2,
    UVec3,
    Vec2,
    Vec3,
    Vec3A,
    Vec4,
)
from .mesh import (
    Mesh,
    Mesh2d,
    Mesh3d,
    MeshMaterial2d,
    MeshMaterial3d,
    MorphWeights,
    PrimitiveTopology,
)
from .pbr import DistanceFog, FogFalloff, ParallaxMappingMethod, StandardMaterial
from .render import (
    AlphaMode,
    ColorGradingGlobal,
    ColorGradingSection,
    Hdr,
    Msaa,
)
from .scene import DynamicScene, DynamicSceneRoot, Scene, SceneRoot, SceneSpawner
from .shader import Shader
from .sprite import (
    ColorMaterial,
    SliceScaleMode,
    Sprite,
    SpriteImageMode,
    TextureSlicer,
)
from .text import (
    Font,
    Justify,
    LineBreak,
    Text2d,
    TextBackgroundColor,
    TextColor,
    TextFont,
    TextLayout,
    TextSpan,
)
from .time import Time, Timer, TimerMode
from .transform import GlobalTransform, Transform, TransformPlugin
from .ui import (
    AlignContent,
    AlignItems,
    AlignSelf,
    BackgroundColor,
    BorderColor,
    BorderGradient,
    BorderRadius,
    BoxSizing,
    Button,
    Display,
    FlexDirection,
    FlexWrap,
    GlobalZIndex,
    GridAutoFlow,
    GridPlacement,
    GridTrack,
    ImageNode,
    Interaction,
    IsDefaultUiCamera,
    JustifyContent,
    JustifyItems,
    JustifySelf,
    Label,
    Node,
    NodeImageMode,
    Outline,
    Overflow,
    OverflowAxis,
    OverflowClipBox,
    OverflowClipMargin,
    PositionType,
    RepeatedGridTrack,
    Text,
    UiRect,
    UiTargetCamera,
    Val,
    Val2,
    ZIndex,
)
from .window import (
    CursorEntered,
    CursorLeft,
    CursorMoved,
    FileDragAndDrop,
    Ime,
    MonitorSelection,
    VideoModeSelection,
    Window,
    WindowPlugin,
    WindowPosition,
    WindowResizeConstraints,
)

# Schedule aliases
Startup = Stage.Startup
Update = Stage.Update
Last = Stage.Last
FixedUpdate = Stage.FixedUpdate
SimTick = Stage.SimTick
First = Stage.First
PreStartup = Stage.PreStartup
PostStartup = Stage.PostStartup
PreUpdate = Stage.PreUpdate
PostUpdate = Stage.PostUpdate
Main = Stage.Main
FixedFirst = Stage.FixedFirst
FixedPreUpdate = Stage.FixedPreUpdate
FixedPostUpdate = Stage.FixedPostUpdate
FixedLast = Stage.FixedLast


# ---------------------------------------------------------------------------
# Bevy prelude items NOT YET available in PyBevy
# ---------------------------------------------------------------------------
#
# Math:
#   BVec2, BVec3, BVec4, BVec3A, BVec4A, Dir3A, IVec3, IVec4, UVec4, IRect
#
# Primitives:
#   Arc2d, ConicalFrustum, ConvexPolygon,
#   Extrusion, Line2d, Line3d, Polygon, Polyline2d, Polyline3d, Ring,
#   Segment3d
#
# ECS:
#   Allow, AnyOf, ApplyDeferred, Deferred, EntityCommands, EntityMut, EntityRef,
#   EntityWorldMut, FilteredResources, FilteredResourcesMut, If, NonSend,
#   NonSendMut, Observer, Or, ParallelCommands, ParamSet, Populated,
#   QueryBuilder, QueryState, Ref, RemovedComponents, MessageMutator
#
# App / Scheduling:
#   Schedules, SubApp, RunFixedMainLoop, RunFixedMainLoopSystems, SpawnScene,
#   TaskPoolOptions
#
# State:
#   ComputedStates, DespawnOnEnter, EnterSchedules,
#   ExitSchedules, StateTransition, StateTransitionEvent, SubStates,
#   TransitionSchedules
#
# Rendering / Camera:
#   ExtractSchedule, ManualTextureViews, MsaaWriteback
#
# UI:
#   BoxShadow, BoxShadowSamples, CalculatedClip, ComputedNode,
#   ComputedUiRenderTargetInfo, ComputedUiTargetCamera, DefaultUiCamera,
#   IgnoreScroll, LayoutConfig, OverrideClip, ResolvedBorderRadius,
#   ScrollPosition, ShadowStyle, UiAntiAlias, UiDebugOptions,
#   UiGlobalTransform, UiPosition, UiScale, ViewportNode, MaterialNode
#
# Text:
#   FontWeight, Strikethrough, StrikethroughColor, TextShadow, Underline,
#   UnderlineColor
#
# Animation:
#   AnimationGraphAssetLoader, AnimationTransition, BlendInput,
#   SerializedAnimationGraph, SerializedAnimationGraphNode,
#   ThreadedAnimationGraph, ThreadedAnimationGraphs
#
# Scene:
#   DynamicSceneBuilder, SceneFilter
#
# Assets:
#   AssetId, AssetMode, DynamicTextureAtlasBuilder, NonPathHandleError,
#   TextureAtlasBuilder, UntypedHandle
#
# Audio:
#   PlaybackMode — intentionally excluded (not in Bevy's audio prelude);
#     use `from pybevy.audio import PlaybackMode`
#
# Spawn:
#   Spawn, SpawnIter, SpawnWith, WithOneRelated, WithRelated
#
# Gizmos:
#   AabbGizmoConfigGroup, DefaultGizmoConfigGroup, Gizmo, GizmoAsset,
#   GizmoConfig, GizmoConfigStore, GizmoLineConfig, GizmoLineJoint,
#   GizmoLineStyle, Gizmos, LightGizmoColor, LightGizmoConfigGroup,
#   ShowAabbGizmo, ShowLightGizmo
#
# Picking:
#   Cancel, Click, DefaultPickingPlugins, Drag, DragDrop, DragEnd,
#   DragEnter, DragEntry, DragLeave, DragOver, DragStart, InteractionPlugin,
#   MeshPickingCamera, MeshPickingPlugin, MeshPickingSettings, MeshRayCast,
#   MeshRayCastSettings, Move, Out, Over, Pickable, PickingMessageWriters,
#   PickingPlugin, Pointer, PointerButton, PointerButtonState,
#   PointerInputPlugin, PointerState, PointerTraversal, Press,
#   RayCastBackfaces, RayCastVisibility, Release, Scroll, SpritePickingCamera,
#   SpritePickingMode, SpritePickingPlugin, SpritePickingSettings,
#   UiPickingCamera, UiPickingPlugin, UiPickingSettings
#
# Reflection:
#   AppFunctionRegistry, AppTypeRegistry, FromReflect, Function, GetField,
#   GetPath, GetTupleStructField, IntoFunction, IntoFunctionMut,
#   PartialReflect, Reflect, ReflectComponent, ReflectDefault,
#   ReflectDeserialize, ReflectEvent, ReflectFreelyMutableState,
#   ReflectFromReflect, ReflectFromWorld, ReflectPath, ReflectResource,
#   ReflectSerialize, ReflectState, Struct, TupleStruct, TypePath
#
# Curves:
#   BackInCurve, BackInOutCurve, BackOutCurve, BounceInCurve,
#   BounceInOutCurve, BounceOutCurve, ChainCurve, CircularInCurve,
#   CircularInOutCurve, CircularOutCurve, ConstantCurve, ContinuationCurve,
#   CubicBSpline, CubicBezier, CubicCardinalSpline, CubicCurve, CubicHermite,
#   CubicInCurve, CubicInOutCurve, CubicNurbs, CubicOutCurve, CubicSegment,
#   CurveReparamCurve, EasingCurve, ElasticCurve, ElasticInCurve,
#   ElasticInOutCurve, ElasticOutCurve, EvenCore, ExponentialInCurve,
#   ExponentialInOutCurve, ExponentialOutCurve, ForeverCurve, FunctionCurve,
#   GraphCurve, Interval, LinearCurve, LinearReparamCurve, MapCurve,
#   PingPongCurve, QuadraticInCurve, QuadraticInOutCurve, QuadraticOutCurve,
#   QuarticInCurve, QuarticInOutCurve, QuarticOutCurve, QuinticInCurve,
#   QuinticInOutCurve, QuinticOutCurve, RationalCurve, RationalSegment,
#   RepeatCurve, ReparamCurve, ReverseCurve, SampleAutoCurve, SampleCurve,
#   SineInCurve, SineInOutCurve, SineOutCurve, SmoothStepCurve,
#   SmoothStepInCurve, SmoothStepOutCurve, SmootherStepCurve,
#   SmootherStepInCurve, SmootherStepOutCurve, StepsCurve,
#   UnevenSampleAutoCurve, UnevenSampleCurve, ZipCurve, UnevenCore
#
# Window:
#   WindowMoved
#
# Other:
#   BevyError, DebugName, NameOrEntity, ShortName,
#   StaticTransformOptimizations, TransformHelper, TransformTreeChanged
# ---------------------------------------------------------------------------


__all__ = [
    # Animation
    "AnimatableCurve",
    "AnimatableKeyframeCurve",
    "AnimatedField",
    "AnimationClip",
    "AnimationGraph",
    "AnimationGraphHandle",
    "AnimationGraphNode",
    "AnimationNodeIndex",
    "AnimationPlayer",
    "AnimationPlugin",
    "AnimationTransitions",
    "VariableCurve",
    "WeightsCurve",
    # App
    "App",
    "AppExit",
    "DefaultPlugins",
    "MinimalPlugins",
    "Plugin",
    "PluginGroup",
    "Stage",
    "TaskPoolPlugin",
    "chain",
    # Assets
    "AssetEvent",
    "AssetPlugin",
    "AssetServer",
    "Assets",
    "Handle",
    # Audio
    "AudioPlayer",
    "AudioSink",
    "AudioSource",
    "GlobalVolume",
    "Pitch",
    "PlaybackSettings",
    "SpatialAudioSink",
    "SpatialListener",
    "Volume",
    # Camera
    "Bloom",
    "BloomCompositeMode",
    "BloomPrefilter",
    "Camera",
    "Camera2d",
    "Camera3d",
    "ClearColor",
    "ClearColorConfig",
    "ColorGrading",
    "ColorGradingGlobal",
    "ColorGradingSection",
    "Hdr",
    "InheritedVisibility",
    "Msaa",
    "OrthographicProjection",
    "PerspectiveProjection",
    "Projection",
    "ScalingMode",
    "Skybox",
    "Tonemapping",
    "ViewVisibility",
    "Visibility",
    # Color
    "Color",
    "Hsla",
    "Hsva",
    "Laba",
    "Lcha",
    "LinearRgba",
    "Oklaba",
    "Oklcha",
    "Srgba",
    "Xyza",
    # Decorators (PyBevy-specific)
    "component",
    "entrypoint",
    "material",
    "plugin",
    "post_process",
    "resource",
    # ECS
    "Added",
    "Changed",
    "ChildOf",
    "Children",
    "Commands",
    "Component",
    "DespawnOnExit",
    "Entity",
    "Has",
    "in_state",
    "Local",
    "Message",
    "MessageReader",
    "MessageWriter",
    "Messages",
    "Mut",
    "Name",
    "NextState",
    "On",
    "OnAdd",
    "OnEnter",
    "OnExit",
    "OnInsert",
    "OnRemove",
    "OnTransition",
    "Query",
    "Res",
    "ResMut",
    "Resource",
    "Single",
    "State",
    "state",
    "View",
    "With",
    "Without",
    "World",
    "run_if",
    # Expr (PyBevy-specific)
    "FieldExpr",
    # GLTF
    "Gltf",
    "GltfAssetLabel",
    "GltfExtras",
    # Image
    "Image",
    "ImagePlugin",
    "TextureAtlas",
    "TextureAtlasLayout",
    "TextureAtlasSources",
    # Input
    "ButtonInput",
    "Gamepad",
    "GamepadAxis",
    "GamepadButton",
    "GamepadSettings",
    "KeyCode",
    "MouseButton",
    "TouchInput",
    "Touches",
    # Light
    "AmbientLight",
    "DirectionalLight",
    "EnvironmentMapLight",
    "GeneratedEnvironmentMapLight",
    "GlobalAmbientLight",
    "LightProbe",
    "PointLight",
    "SpotLight",
    # Math
    "Annulus",
    "Capsule2d",
    "Capsule3d",
    "Circle",
    "CircularSector",
    "CircularSegment",
    "Cone",
    "Cuboid",
    "Cylinder",
    "Dir2",
    "Dir3",
    "EaseFunction",
    "Ellipse",
    "EulerRot",
    "InfinitePlane3d",
    "Isometry2d",
    "Isometry3d",
    "IVec2",
    "JumpAt",
    "Mat2",
    "Mat3",
    "Mat3A",
    "Mat4",
    "Plane3d",
    "Quat",
    "Ray2d",
    "Ray3d",
    "Rect",
    "Rectangle",
    "RegularPolygon",
    "Rhombus",
    "Sphere",
    "Tetrahedron",
    "Torus",
    "Triangle2d",
    "Triangle3d",
    "URect",
    "UVec2",
    "UVec3",
    "Vec2",
    "Vec3",
    "Vec3A",
    "Vec4",
    # Mesh
    "Mesh",
    "Mesh2d",
    "Mesh3d",
    "MeshMaterial2d",
    "MeshMaterial3d",
    "MorphWeights",
    "PrimitiveTopology",
    # PBR
    "AlphaMode",
    "DistanceFog",
    "FogFalloff",
    "ParallaxMappingMethod",
    "StandardMaterial",
    # Scene
    "DynamicScene",
    "DynamicSceneRoot",
    "Scene",
    "SceneRoot",
    "SceneSpawner",
    # Shader
    "Shader",
    # Sprite
    "ColorMaterial",
    "SliceScaleMode",
    "Sprite",
    "SpriteImageMode",
    "TextureSlicer",
    # Text
    "Font",
    "Justify",
    "LineBreak",
    "Text2d",
    "TextBackgroundColor",
    "TextColor",
    "TextFont",
    "TextLayout",
    "TextSpan",
    # Time
    "Time",
    "Timer",
    "TimerMode",
    # Transform
    "GlobalTransform",
    "Transform",
    "TransformPlugin",
    # UI
    "AlignContent",
    "AlignItems",
    "AlignSelf",
    "BackgroundColor",
    "BorderColor",
    "BorderGradient",
    "BorderRadius",
    "BoxSizing",
    "Button",
    "Display",
    "FlexDirection",
    "FlexWrap",
    "GlobalZIndex",
    "GridAutoFlow",
    "GridPlacement",
    "GridTrack",
    "ImageNode",
    "Interaction",
    "IsDefaultUiCamera",
    "JustifyContent",
    "JustifyItems",
    "JustifySelf",
    "Label",
    "Node",
    "NodeImageMode",
    "Outline",
    "Overflow",
    "OverflowAxis",
    "OverflowClipBox",
    "OverflowClipMargin",
    "PositionType",
    "RepeatedGridTrack",
    "Text",
    "UiRect",
    "UiTargetCamera",
    "Val",
    "Val2",
    "ZIndex",
    # Window
    "CursorEntered",
    "CursorLeft",
    "CursorMoved",
    "FileDragAndDrop",
    "Ime",
    "MonitorSelection",
    "VideoModeSelection",
    "Window",
    "WindowPlugin",
    "WindowPosition",
    "WindowResizeConstraints",
    # Typing re-exports
    "Optional",
    # Schedule aliases
    "First",
    "FixedFirst",
    "FixedLast",
    "FixedPostUpdate",
    "FixedPreUpdate",
    "FixedUpdate",
    "Last",
    "Main",
    "PostStartup",
    "PostUpdate",
    "PreStartup",
    "PreUpdate",
    "SimTick",
    "Startup",
    "Update",
]
