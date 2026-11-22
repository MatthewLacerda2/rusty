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
