//! src/scene/ — Scene documents: save / load / snapshot (single active scene)
//!
//! No multi-scene / open-world loading — loading a scene REPLACES the World.
//!
//! On-disk format is a plain serde `SceneData` document (entities + their
//! component VALUES + scene settings), kept separate from runtime storage: the
//! hecs World is runtime, `SceneData` is the file. This bridges hecs (which does
//! not serialize a World for free), keeps scene files human-readable/diffable, and
//! is also what the headless harness loads.
//!
//! Principle: scenes store REFERENCES (primitive name, asset path) + values, never
//! baked GPU buffers — those are rehydrated on load (see serialize.rs).
//!
//! Submodules:
//!   scene     — the `Scene` aggregate (the `ecs::World` + scene-level state)
//!   serialize — World <-> SceneData
//!   io        — save/load, path + extension, default-scene seeding
//!   snapshot  — edit-mode snapshot/restore around Play

pub mod collision_matrix;
pub mod io;
pub mod layers;
#[allow(clippy::module_inception)]
pub mod scene;
pub mod serialize;
pub mod snapshot;

pub use collision_matrix::CollisionMatrix;
pub use io::{
    is_scene_path, load_from_file, save_to_file, seed_default_scene, seed_default_scripts,
    DEFAULT_SCENE_PATH, DEFAULT_SCENE_SOURCE, SCENE_EXTENSION,
};
pub use layers::{layer_in_mask, LayerRegistry, LAYER_COUNT};
pub use scene::{
    AnimatorComponent, CameraComponent, ClearFlags, ColliderComponent, ColliderShape,
    CollisionResponse, DirtyFlag, EmitMode, Entity, HealthComponent, LightComponent, LightType,
    MeshComponent, NavMeshAgentComponent, Particle, ParticleBlend, ParticleEmitterComponent,
    RigidBodyComponent, Scene, ScriptComponent, TextureComponent, Tonemap, TransformComponent,
    VisualCorrectionComponent,
};
pub use serialize::{apply_scene_data, asset_mesh_component, to_scene_data, SceneData};
pub use snapshot::SceneSnapshot;
