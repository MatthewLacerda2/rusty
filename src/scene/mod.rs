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

pub mod asset_instance;
pub mod authoring;
pub mod collision_matrix;
pub mod io;
pub mod layers;
pub mod lighting;
pub mod prefab;
#[allow(clippy::module_inception)]
pub mod scene;
pub mod serialize;
pub mod snapshot;

pub use collision_matrix::CollisionMatrix;
pub use io::{
    is_scene_path, load_from_file, save_to_file, seed_default_scene, seed_default_scripts,
    DEFAULT_SCENE_PATH, DEFAULT_SCENE_SOURCE, DEFAULT_SCRIPTS_DEST_DIR, SCENE_EXTENSION,
};
pub use layers::{layer_in_mask, LayerRegistry, LAYER_COUNT};
pub use lighting::io::{
    apply_lighting, extract_lighting, load_lighting_sidecar, save_lighting_sidecar, sidecar_path,
    LightingData,
};
pub use lighting::probe::{Probe, ProbeGrid, ProbeVolume};
pub use lighting::probe_fill::{analytic_fill, AnalyticEnv};
pub use lighting::reflection_probe::{ReflectionProbe, ReflectionProbeSet};
pub use lighting::sh::{Sh9, SH_COEFFS};
pub use prefab::apply::{apply_instance_field_to_source, apply_instance_to_source};
pub use prefab::link::{
    list_instance_overrides, record_instance_overrides, reimport_all_linked_instances,
    reimport_instance, revert_instance_overrides,
};
pub use prefab::{
    extract_prefab, instantiate_prefab, instantiate_prefab_linked, is_prefab_path,
    load_and_instantiate, load_and_instantiate_linked, read_prefab_file, save_prefab,
    write_prefab_file, PrefabData, PREFAB_EXTENSION,
};
pub use scene::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ClearFlags, ColliderComponent,
    ColliderShape, CollisionResponse, DirtyFlag, EmitMode, Entity, HealthComponent, LightComponent,
    LightType, MaterialAsset, MaterialComponent, MeshComponent, NavMeshAgentComponent, Particle,
    ParticleBlend, ParticleEmitterComponent, RenderMode, RigidBodyComponent, Scene,
    ScriptComponent, ScriptFieldValue, TextureComponent, Tonemap, TransformComponent,
    VisualCorrectionComponent, DEFAULT_AMBIENT_COLOR, DEFAULT_AMBIENT_INTENSITY,
};
pub use serialize::{apply_scene_data, asset_mesh_component, to_scene_data, SceneData};
pub use snapshot::SceneSnapshot;
