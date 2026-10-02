//! src/components/mod.rs — First-class component registry
//!
//! First-class engine components: the engine expects these and its systems
//! interface with them (e.g. NavMeshAgent <-> baked navmesh), exactly like
//! Unity's built-in components. Each is its own hecs column (`ecs::world`),
//! attached only when present; `Entity` (defined in `entity.rs`) is the
//! document shape one GameObject's columns assemble into/out of, not live
//! storage. `Transform` is mandatory.
//!
//! Allowed deps: components::*, glam, serde, asset (data only).

pub mod animator;
pub mod audio_source;
pub mod camera;
pub mod character_controller;
pub mod collider;
pub mod entity;
pub mod joint;
pub mod light;
pub mod lod_group;
pub mod material;
pub mod mesh;
pub mod nav_agent;
pub mod nav_obstacle;
pub mod particle;
pub mod ribbon;
pub mod rigidbody;
pub mod script;
pub mod texture;
pub mod transform;
pub mod ui;
pub mod visual_correction;

pub use animator::{AnimatorComponent, AnimatorParameter, AnimatorParameters};
pub use audio_source::AudioSourceComponent;
pub use camera::{CameraComponent, ClearFlags, Projection, RenderTarget, RENDER_TEXTURE_PREFIX};
pub use character_controller::CharacterControllerComponent;
pub use collider::{CapsuleAxis, ColliderComponent, ColliderShape, CombineMode, PhysicsMaterial};
pub use entity::{Entity, PrefabLink};
pub use joint::{JointComponent, JointKind};
pub use light::{LightComponent, LightType};
pub use lod_group::{LodGroupComponent, LodLevel};
pub use material::{MaterialAsset, MaterialComponent, RenderMode};
pub use mesh::{DirtyFlag, MeshComponent};
pub use nav_agent::{
    NavMeshAgentComponent, NavPathStatus, DEFAULT_AVOIDANCE_PRIORITY, MAX_AVOIDANCE_PRIORITY,
};
pub use nav_obstacle::{NavMeshObstacleComponent, ObstacleShape};
pub use particle::{
    CollisionResponse, EmitFrom, EmitMode, EmitShape, Flipbook, Particle, ParticleBlend,
    ParticleEmitterComponent, ParticleRender, ParticleRenderMode, SubEmitTrigger, SubEmitters,
};
pub use ribbon::{LineComponent, RibbonStyle, TextureMode, TrailComponent};
pub use rigidbody::{CollisionDetection, RigidBodyComponent};
pub use script::{ScriptComponent, ScriptFieldValue};
pub use texture::TextureComponent;
pub use transform::TransformComponent;
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
pub use visual_correction::{ShadowSettings, SsaoSettings, Tonemap, VisualCorrectionComponent};
