//! crates/scene/src/lib.rs — Scene documents: save / load / snapshot (single active scene)
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
//!   edit_side — World <-> SceneData, save/load/seed, the load swap, reimports
//!   authored  — what the authoring layer lists for the post effects, by name
//!   authoring — the shared edit verbs the editor, the API and loads all go through
//!   create    — a new GameObject: the primitives, the Add-Component defaults, `create_entity`
//!   rules     — the bounds a stored value is held to (float sanitizing, lens, drive, ranges)
//!   prefab_link — a linked instance's members, a link's source, a removed part
//!   asset_instance, prefab — instantiate one imported sub-object; prefabs
//!   camera, decal, lighting — the active view, decal records, baked lighting
//!   identity, material_lookup — a live scene's identity; material slot lookups
//!   revision  — how a reader learns a scene-level value changed (`Revisioned`)
//!   rehydrate — rebuild a mesh's geometry + rig from its on-disk reference
//!   settings  — project/scene-wide settings: fog, layers, collision matrix, shader
//!               overrides, sky, ambient
//!   lod_instance, world_cache — LOD-set instantiation; the world-space read cache
//!   model_instance — a whole model as its node tree, one child per node (#1083)
//!   skeleton  — bones as GameObjects: spawn, save-strip, palette build (#453)
//!   nav_settings — the per-scene navmesh bake settings (`navigation` reads them)
//!   mesh_library — the meshes a script makes from vertex data (#2059)
//!   physics_material — the shared physics material assets colliders name (#1878)
//!   io        — the scene file's extension, reading its document, the bake files' record
//!   default_scene — the default scene's builder, materials and seeded assets
//!   snapshot  — edit-mode snapshot/restore around Play
//!   state     — the one inventory of the scene's state and what each lifecycle owes it
//!   undo      — edit mode's Undo / Redo: per-object records, grouped by gesture
//!   runtime   — play-mode requests drained at the tick tail: destroys, scene loads

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod asset_instance;
pub mod authored;
pub mod authoring;
pub mod camera;
pub mod create;
pub mod decal;
pub mod default_scene;
pub mod edit_side;
pub mod identity;
pub mod io;
mod light_mode;
pub mod lighting;
pub mod lod_instance;
mod material_lookup;
pub mod mesh_library;
pub mod model_instance;
pub mod nav_settings;
pub mod physics_material;
pub mod prefab;
pub mod prefab_link;
pub mod rehydrate;
pub mod revision;
pub mod rules;
pub mod runtime;
pub mod scene;
pub mod serialize;
pub mod settings;
pub mod skeleton;
pub mod snapshot;
pub mod state;
#[doc(hidden)]
pub mod test_levels;
pub mod undo;
pub mod world_cache;

pub use camera::{
    camera_for_entity, game_camera_from_scene, sync_lens_from_scene, Camera, ScreenPoint,
};
pub use edit_side::io::{
    build_default_scene, bundled, has_unsaved_edits, load_from_file, load_parsed, print_refresh,
    save_lighting, save_to_file, seed_default_scene, seed_default_scripts, seed_project,
    seed_workspace, LightingSave, LoadError, DEFAULT_SCENE_PATH, DEFAULT_SCRIPTS_DEST_DIR,
    LIGHTING_IN_PLAY,
};
pub use edit_side::rehydrate::model_writes;
pub use edit_side::serialize::{apply_scene_data, to_scene_data};
pub use identity::SceneId;
pub use io::{new_scene, parse_scene_text, read_scene_file, NEW_IN_PLAY, SCENE_EXTENSION};
pub use lighting::io::{sidecar_path, LightingData};
pub use lighting::probe::{Probe, ProbeGrid, ProbeVolume};
pub use lighting::probe_fill::analytic_fill;
pub use lighting::reflection_probe::{ProbeShape, ReflectionProbe, ReflectionProbeSet};
pub use lighting::sh::Sh9;
pub use prefab::link::{list_instance_overrides, reimport_instance, revert_instance_overrides};
pub use prefab::{
    extract_prefab, instantiate_prefab, is_prefab_path, load_and_instantiate,
    load_and_instantiate_linked, read_prefab_file, save_prefab, PrefabData, PREFAB_EXTENSION,
};
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
pub use snapshot::SceneSnapshot;
