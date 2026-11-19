//! crates/scene/src/lib.rs — the Scene and its document (single active scene)
//!
//! Saving, loading into a live scene, the authoring verbs, prefabs and the snapshot
//! around Play sit one crate up, in `rusty-edit`, which writes a scene through this
//! crate's API; nothing here names it.
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
//! baked GPU buffers — those are rehydrated on load (see rehydrate).
//!
//! Submodules:
//!   scene     — the `Scene` aggregate (the `ecs::World` + scene-level state)
//!   serialize — the scene document, `SceneData`
//!   authored  — what the authoring layer lists for the post effects, by name
//!   create    — a new GameObject: the primitives, the Add-Component defaults, `create_entity`
//!   rules     — the bounds a stored value is held to (float sanitizing, lens, drive, ranges)
//!   prefab_link — a linked instance's members, a link's source, a removed part
//!   camera, decal, lighting — the active view, decal records, baked lighting
//!   world_cache — the world-space read cache
//!   identity, material_lookup — a live scene's identity; material slot lookups
//!   revision  — how a reader learns a scene-level value changed (`Revisioned`)
//!   rehydrate — rebuild a mesh's geometry + rig from its on-disk reference
//!   settings  — project/scene-wide settings: fog, layers, collision matrix, shader
//!               overrides, sky, ambient
//!   skeleton  — bones as GameObjects: spawn, save-strip, palette build (#453)
//!   nav_settings — the per-scene navmesh bake settings (`navigation` reads them)
//!   mesh_library — the meshes a script makes from vertex data (#2059)
//!   physics_material — the shared physics material assets colliders name (#1878)
//!   io        — the scene file's extension, reading its document, the bake files' record
//!   state     — the one inventory of the scene's state and what each lifecycle owes it
//!   undo      — edit mode's Undo / Redo: per-object records, grouped by gesture
//!   runtime   — play-mode requests drained at the tick tail: destroys, scene loads

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod authored;
pub mod camera;
pub mod create;
pub mod decal;
pub mod identity;
pub mod io;
mod light_mode;
pub mod lighting;
mod material_lookup;
pub mod mesh_library;
pub mod nav_settings;
pub mod physics_material;
pub mod prefab_link;
pub mod rehydrate;
pub mod revision;
pub mod rules;
pub mod runtime;
pub mod scene;
pub mod serialize;
pub mod settings;
pub mod skeleton;
pub mod state;
#[doc(hidden)]
pub mod test_levels;
pub mod undo;
pub mod world_cache;

pub use camera::{
    camera_for_entity, game_camera_from_scene, sync_lens_from_scene, Camera, ScreenPoint,
};
pub use identity::SceneId;
pub use io::{new_scene, parse_scene_text, read_scene_file, NEW_IN_PLAY, SCENE_EXTENSION};
pub use lighting::io::{sidecar_path, LightingData};
pub use lighting::probe::{Probe, ProbeGrid, ProbeVolume};
pub use lighting::probe_fill::analytic_fill;
pub use lighting::reflection_probe::{
    PreparedProbes, ProbeShape, ReflectionProbe, ReflectionProbeSet,
};
pub use lighting::sh::Sh9;
pub use runtime::despawn::Despawn;
pub use scene::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ClearFlags, ColliderComponent,
    ColliderShape, CollisionDetection, CollisionResponse, EmitMode, Entity, LightComponent,
    LightMode, LightType, MaterialAsset, MaterialComponent, MeshComponent, MeshRevision,
    NavMeshAgentComponent, Particle, ParticleBlend, ParticleEmitterComponent,
    PostProcessingComponent, RenderMode, RigidBodyComponent, Scene, ScriptComponent,
    ScriptFieldValue, Tonemap, TransformComponent, DEFAULT_AMBIENT_COLOR,
    DEFAULT_AMBIENT_INTENSITY,
};
pub use serialize::{asset_mesh_component, SceneData};
pub use settings::ambient::procedural_sky_color;
pub use settings::ambient::{AmbientMode, AmbientRadiance, AmbientSettings, DrawnAmbient};
pub use settings::collision_matrix::CollisionMatrix;
pub use settings::fog::{FogMode, FogSettings};
pub use settings::layers::{layer_in_mask, LayerRegistry, LAYER_COUNT};
pub use settings::sky::SkySettings;
pub use settings::{collision_matrix, fog, layers, shader_overrides, sky};
