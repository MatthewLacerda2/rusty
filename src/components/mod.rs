//! src/components/mod.rs — First-class component registry
//!
//! First-class engine components: the engine expects these and its systems
//! interface with them (e.g. NavMeshAgent <-> baked navmesh), exactly like
//! Unity's built-in components. Each is its own hecs column (`ecs::world`),
//! attached only when present; `Entity` (defined in `entity/mod.rs`) is the
//! document shape one GameObject's columns assemble into/out of, not live
//! storage. `Transform` is mandatory.

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod ai;
pub mod animator;
pub mod audio;
pub mod entity;
pub mod material;
pub mod mesh;
pub mod nav;
pub mod particle;
pub mod physics;
pub mod registry;
pub mod rendering;
pub mod ribbon;
pub mod script;
pub mod transform;
pub mod ui;

pub use ai::memory::{Known, Memory, Sense, Update as PerceptionUpdate};
pub use ai::perception::AIPerceptionComponent;
pub use animator::{
    follow_clamped, root_chain, stored_leaf, wrap_delta, ActiveMontage, AnimatorComponent,
    AnimatorParameter, AnimatorParameters, Bounds, IkChain, IkConstraint, IkTarget, LayerState,
    LeafTime, MontageEvent, MontageRefused, Montages, Motion, NodeStates, Playback, Playhead,
    Reported, RootMotion, RootMotionState, StateCallback, StateEvent, StateInfo, StateTracker,
    Step, SyncSource, SyncState, MONTAGE_BLEND_OUT, MONTAGE_END,
};
pub use audio::reverb_zone::{ReverbParams, ReverbPreset, ReverbZoneComponent};
pub use audio::source::AudioSourceComponent;
pub(crate) use entity::with_ref_components;
pub use entity::{
    escape_token, visit_override_refs, Entity, EntityRefs, PrefabLink, PrefabOverrides, RefVisit,
    PART_KEY, SIZE_TOKEN,
};
pub use material::{
    BlendMode, DecalBlend, DetailLayer, MaterialAsset, MaterialComponent, RefractionModel,
    RenderFace, RenderMode, TaaReactive, UvMapping, MAX_REACTIVE, SORTING_PRIORITY_RANGE,
};
pub use mesh::{MeshComponent, MeshRevision, ShadowCasting};
pub use nav::agent::{NavMeshAgentComponent, NavPathStatus, MAX_AVOIDANCE_PRIORITY};
pub use nav::modifier::NavMeshModifierVolumeComponent;
pub use nav::object_modifier::NavMeshModifierComponent;
pub use nav::obstacle::{NavMeshObstacleComponent, ObstacleShape};
pub use nav::offmesh_link::{
    OffMeshLinkComponent, OffMeshLinkData, OffMeshLinkKind, LINK_SNAP_DISTANCE,
};
pub use particle::{
    CollisionResponse, EmitFrom, EmitMode, EmitShape, Flipbook, Particle, ParticleBlend,
    ParticleEmitterComponent, ParticleRender, ParticleRenderMode, SimulationSpace, SubEmitTrigger,
    SubEmitters,
};
pub use physics::character_controller::CharacterControllerComponent;
pub use physics::collider::{
    capsule_dims, CapsuleAxis, ColliderComponent, ColliderShape, CombineMode, PhysicsMaterial,
    ScaledShape, SharedMesh,
};
pub use physics::joint::{JointComponent, JointKind};
pub use physics::rigidbody::{CollisionDetection, RigidBodyComponent, DEFAULT_ANGULAR_DRAG};
pub use registry::FirstClass;
pub use rendering::camera::{
    horizontal_to_vertical_fov, is_render_texture, vertical_to_horizontal_fov, CameraComponent,
    ClearFlags, FovAxis, Projection, RenderTarget, TaaQuality, RENDER_TEXTURE_PREFIX,
};
pub use rendering::light::{
    falloff, falloff_near, falloff_window, falloff_window_sq, spot_cone, LightComponent, LightMode,
    LightType, FALLOFF_EPSILON_SQ,
};
pub use rendering::light_probe_proxy_volume::{
    LightProbeProxyVolumeComponent, ProxyBoundsMode, ProxyGrid, ProxyQuality, ProxyResolutionMode,
};
pub use rendering::lod_group::{LodGroupComponent, LodLevel};
pub use rendering::post_processing::{
    AutoExposure, Bloom, ChromaticAberration, ColorAdjustments, ColorLookup, ColorWheel,
    CustomEffects, DepthOfField, DofMode, Exposure, ExposureMode, FilmGrain, LiftGammaGain,
    PostProcessingComponent, ShadowSettings, SsaoSettings, Ssil, Ssr, Tonemap, Tonemapping,
    Vignette, VolumetricFog, Wheel, WhiteBalance,
};
pub use ribbon::{LineComponent, RibbonStyle, TextureMode, TrailComponent};
pub use script::{ScriptComponent, ScriptFieldValue};
pub use transform::{Pose, TransformComponent};
pub use ui::{
    BackdropFilterComponent, CanvasComponent, CanvasGroupComponent, CanvasRenderMode, CanvasSway,
    FillMethod, FillOrigin, ImageComponent, ImageType, LayoutAxisFit, LayoutConstraint,
    LayoutCorner, LayoutElementComponent, LayoutGroupComponent, LayoutKind, MaskComponent,
    NavigationMode, RectMaskComponent, RectTransformComponent, SelectableComponent,
    SelectableTransition, SelectionState, TextAlignment, TextComponent, TextOverflow, UiShader,
    WorldAnchor,
};
pub use ui::{
    GradientKind, GradientStop, ShapeComponent, ShapeCorner, ShapeGlow, ShapeKind, ShapeShadow,
    UiBlend, UiGradient, MAX_GRADIENT_STOPS,
};
