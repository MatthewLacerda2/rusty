//! src/components/mod.rs — First-class component registry
//!
//! First-class engine components: the engine expects these and its systems
//! interface with them (e.g. NavMeshAgent <-> baked navmesh), exactly like
//! Unity's built-in components. Each is its own hecs column (`ecs::world`),
//! attached only when present; `Entity` (defined in `entity.rs`) is the
//! document shape one GameObject's columns assemble into/out of, not live
//! storage. `Transform` is mandatory.

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod animator;
pub mod audio;
pub mod entity;
pub mod material;
pub mod mesh;
pub mod nav;
pub mod particle;
pub mod physics;
pub mod rendering;
pub mod ribbon;
pub mod script;
pub mod transform;
pub mod ui;

pub use animator::{
    AnimatorComponent, AnimatorParameter, AnimatorParameters, Bounds, IkChain, IkConstraint,
    IkTarget, LayerState, Motion, Playback, Playhead, Step,
};
pub use audio::reverb_zone::{ReverbParams, ReverbPreset, ReverbZoneComponent};
pub use audio::source::AudioSourceComponent;
pub(crate) use entity::with_ref_components;
pub use entity::{escape_token, visit_override_refs, Entity, EntityRefs, PrefabLink, RefVisit};
pub use material::{DecalBlend, MaterialAsset, MaterialComponent, RenderMode};
pub use mesh::{DirtyFlag, MeshComponent};
pub use nav::agent::{NavMeshAgentComponent, NavPathStatus, MAX_AVOIDANCE_PRIORITY};
pub use nav::modifier::NavMeshModifierVolumeComponent;
pub use nav::obstacle::{NavMeshObstacleComponent, ObstacleShape};
pub use nav::offmesh_link::{
    OffMeshLinkComponent, OffMeshLinkData, OffMeshLinkKind, LINK_SNAP_DISTANCE,
};
pub use particle::{
    CollisionResponse, EmitFrom, EmitMode, EmitShape, Flipbook, Particle, ParticleBlend,
    ParticleEmitterComponent, ParticleRender, ParticleRenderMode, SubEmitTrigger, SubEmitters,
};
pub use physics::character_controller::CharacterControllerComponent;
pub use physics::collider::{
    capsule_dims, CapsuleAxis, ColliderComponent, ColliderShape, CombineMode, PhysicsMaterial,
    ScaledShape,
};
pub use physics::joint::{JointComponent, JointKind};
pub use physics::rigidbody::{CollisionDetection, RigidBodyComponent};
pub use rendering::camera::{
    is_render_texture, CameraComponent, ClearFlags, Projection, RenderTarget, RENDER_TEXTURE_PREFIX,
};
pub use rendering::light::{LightComponent, LightMode, LightType};
pub use rendering::lod_group::{LodGroupComponent, LodLevel};
pub use rendering::post_processing::{
    AutoExposure, ChromaticAberration, ColorWheel, ExposureMode, FilmGrain, LiftGammaGain,
    PostProcessingComponent, ShadowSettings, SsaoSettings, Tonemap, Vignette, Wheel, WhiteBalance,
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
