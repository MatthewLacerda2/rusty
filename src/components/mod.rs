//! src/components/mod.rs — Built-in component registry
//!
//! First-class engine components: the engine expects these and its systems
//! interface with them (e.g. NavMeshAgent <-> baked navmesh), exactly like
//! Unity's built-in components. Each entity bundles its components into the
//! `Entity` struct (defined in `entity.rs`), which is the single value stored
//! per hecs entity; `Transform` is mandatory.
//!
//! Allowed deps: components::*, glam, serde, render::mesh (data only).

pub mod animator;
pub mod camera;
pub mod collider;
pub mod entity;
pub mod health;
pub mod light;
pub mod mesh;
pub mod nav_agent;
pub mod particle;
pub mod rigidbody;
pub mod script;
pub mod texture;
pub mod transform;
pub mod visual_correction;

pub use animator::AnimatorComponent;
pub use camera::{CameraComponent, ClearFlags};
pub use collider::{ColliderComponent, ColliderShape};
pub use entity::Entity;
pub use health::HealthComponent;
pub use light::{LightComponent, LightType};
pub use mesh::{DirtyFlag, MeshComponent};
pub use nav_agent::NavMeshAgentComponent;
pub use particle::{
    CollisionResponse, EmitMode, Particle, ParticleBlend, ParticleEmitterComponent,
};
pub use rigidbody::RigidBodyComponent;
pub use script::{ScriptComponent, ScriptFieldValue};
pub use texture::TextureComponent;
pub use transform::TransformComponent;
pub use visual_correction::{Tonemap, VisualCorrectionComponent};
