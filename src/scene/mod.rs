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
//!   rehydrate — rebuild a mesh's geometry + rig from its on-disk reference; `model_writes` reloads re-written models
//!   settings  — project/scene-wide settings: fog, layers, collision matrix, shader overrides
//!   lod_instance, world_cache — LOD-set instantiation; the world-space read cache
//!   skeleton  — bones as GameObjects: spawn, save-strip, palette build (#453)
//!   nav_settings — the per-scene navmesh bake settings (`navigation` reads them)
//!   io        — save/load, path + extension, default-scene seeding
//!   default_scene — the default scene's builder, materials and seeded assets
//!   snapshot  — edit-mode snapshot/restore around Play
//!   runtime   — play-mode requests drained at the tick tail: destroys, scene loads

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod asset_instance;
pub mod authoring;
pub mod camera;
pub mod decal;
pub mod default_scene;
pub mod identity;
pub mod io;
pub mod lighting;
pub mod lod_instance;
mod material_lookup;
pub mod nav_settings;
pub mod prefab;
pub mod rehydrate;
pub mod runtime;
#[expect(
    clippy::module_inception,
    reason = "`scene::scene` holds the `Scene` type; renaming the file churns every import for no gain"
)]
pub mod scene;
pub mod serialize;
pub mod settings;
pub mod skeleton;
pub mod snapshot;
pub mod world_cache;

pub use camera::{
    camera_for_entity, game_camera_from_scene, sync_lens_from_scene, Camera, ScreenPoint,
};
pub use identity::SceneId;
pub use io::{
    build_default_scene, bundled, load_from_file, load_parsed, parse_scene_text, read_scene_file,
    save_lighting, save_to_file, seed_default_scene, seed_default_scripts, seed_workspace,
    LightingSave, LoadError, DEFAULT_SCENE_PATH, DEFAULT_SCRIPTS_DEST_DIR, LIGHTING_IN_PLAY,
    SCENE_EXTENSION,
};
pub use lighting::io::{sidecar_path, LightingData};
pub use lighting::probe::{Probe, ProbeGrid, ProbeVolume};
pub use lighting::probe_fill::analytic_fill;
pub use lighting::reflection_probe::{ReflectionProbe, ReflectionProbeSet};
pub use lighting::sh::Sh9;
pub use prefab::link::{
    list_instance_overrides, record_instance_overrides, reimport_instance,
    revert_instance_overrides,
};
pub use prefab::{
    extract_prefab, instantiate_prefab, is_prefab_path, load_and_instantiate,
    load_and_instantiate_linked, read_prefab_file, save_prefab, PrefabData, PREFAB_EXTENSION,
};
pub use rehydrate::model_writes;
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
pub use serialize::{apply_scene_data, asset_mesh_component, to_scene_data, SceneData};
pub use settings::collision_matrix::CollisionMatrix;
pub use settings::fog::{FogMode, FogSettings};
pub use settings::layers::{layer_in_mask, LayerRegistry, LAYER_COUNT};
pub use settings::{collision_matrix, fog, layers, shader_overrides};
pub use snapshot::SceneSnapshot;
