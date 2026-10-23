//! src/scene/scene.rs — Scene: the hecs-backed `ecs::World` plus scene-level state.
//!
//! Entity/component storage of record is `ecs::World` (one `Entity` bundle per
//! hecs entity, generational handles + stable-id/name lookup). `Scene` owns that
//! `World` together with the engine-level scene scalars (selection, skybox,
//! ambient light) and the parenting / collider / serialization helpers that
//! operate across the World — concerns that must NOT live inside `ecs::World`
//! (whose deps are restricted to hecs + components). This wrapper lives in the
//! `scene` layer, which is allowed to depend on `crate::scene` and rendering.

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;

use crate::ecs::World;
use crate::scene::collision_matrix::CollisionMatrix;
use crate::scene::identity::SceneId;
use crate::scene::layers::LayerRegistry;
use crate::scene::nav_settings::NavMeshSettings;

// Re-export the component types so the many existing `scene::…Component` paths
// resolve. The structs themselves live in `crate::components`.
pub use crate::components::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ClearFlags, ColliderComponent,
    ColliderShape, CollisionDetection, CollisionResponse, DirtyFlag, EmitMode, Entity,
    LightComponent, LightMode, LightType, MaterialAsset, MaterialComponent, MeshComponent,
    NavMeshAgentComponent, Particle, ParticleBlend, ParticleEmitterComponent, RenderMode,
    RigidBodyComponent, ScriptComponent, ScriptFieldValue, Tonemap, TransformComponent,
    VisualCorrectionComponent,
};

fn default_skybox_path() -> String {
    "".to_string()
}

/// The default hemisphere-ambient SKY colour (the up-facing tint) — a soft daylight
/// blue. This is the engine's single source of truth for the default ambient: the
/// serde defaults in `serialize.rs` re-export it, and the procedural gradient sky
/// (#256) derives its dome from the same `lighting.ambient` uniform the surface
/// shader reads, so background and lighting always agree. The ground term is this
/// colour scaled down in the shader. Bright/coloured enough that two differently
/// oriented faces visibly differ with NO baked probes and without a direct light on
/// every face.
pub const DEFAULT_AMBIENT_COLOR: Vec3 = Vec3::new(0.45, 0.55, 0.70);
/// The default hemisphere-ambient intensity (scales [`DEFAULT_AMBIENT_COLOR`]).
pub const DEFAULT_AMBIENT_INTENSITY: f32 = 0.55;

fn default_ambient_color() -> Vec3 {
    DEFAULT_AMBIENT_COLOR
}
fn default_ambient_intensity() -> f32 {
    DEFAULT_AMBIENT_INTENSITY
}

pub struct Scene {
    /// This live scene's runtime identity (#355). Not scene *data* — it names the
    /// instance, so the renderer can key per-entity GPU caches by (scene, entity)
    /// and stop two scenes' entity ids from colliding. See [`SceneId`].
    id: SceneId,
    pub world: World,
    pub selected_entity_id: Option<u32>,
    pub skybox_path: String,
    pub ambient_color: Vec3,
    pub ambient_intensity: f32,
    /// Per-scene navmesh bake tunables (#276): Unity's per-scene navmesh bake
    /// settings (agent radius, max slope, max step, grid spacing). The nav bake
    /// reads these off the active scene; serialized with the scene like the ambient
    /// scalars, with serde defaults equal to the historical bake constants.
    pub nav_settings: NavMeshSettings,
    /// Scene fog (#437): distance + height fog the renderer applies to every
    /// world-space pass. Serialized with the scene; off by default.
    pub fog: crate::scene::FogSettings,
    /// The project's shared Layers registry (Unity's Tags & Layers). Serialized
    /// with the scene; the per-entity `layer` index points into it.
    pub layers: LayerRegistry,
    /// Unity's Layer Collision Matrix: which layer collides with which. Drives
    /// rapier `InteractionGroups` on collider build (#91); serialized with the scene.
    pub collision_matrix: CollisionMatrix,
    /// The per-World material library: reusable `MaterialAsset`s keyed by name.
    /// Entities reference a material by name via their `MaterialComponent`; many
    /// entities can share one entry. A `BTreeMap` (not `HashMap`) so serialization
    /// order is deterministic (the repo has a determinism guard).
    pub materials: BTreeMap<String, MaterialAsset>,
    /// Runtime box-projector decals (bullet holes, scorch, blood splats). These are
    /// ephemeral *visual* state spawned from raycast hits, NOT serialized scene
    /// data — the decal renderer reads them each frame and projects them onto the
    /// surfaces they overlap. Bounded FIFO (the oldest fades out past `MAX_DECALS`);
    /// aged on the fixed tick (#639).
    pub decals: crate::scene::decal::DecalSet,
    /// Scene-level light-probe dataset (#240): probe POSITIONS + grid layout live in
    /// the scene document; their baked L2 SH irradiance lives in the
    /// `<scene>.lighting.json` sidecar. Dynamic (non-static) objects sample the
    /// interpolated probe field instead of the flat hemispherical ambient term.
    pub probes: crate::scene::lighting::probe::ProbeVolume,
    /// Scene-level reflection-probe dataset (#244): each probe is a capture position, a
    /// box volume for parallax correction, and a PATH to its baked cubemap (a KTX2
    /// sidecar — never inlined, like `skybox_path`). Lit objects reflect the nearest
    /// applicable probe with box-projected parallax correction instead of the global
    /// skybox. The cubemaps themselves are baked by a later issue (#245).
    pub reflection_probes: crate::scene::lighting::reflection_probe::ReflectionProbeSet,
    /// Baked lightmaps (#438): which RGBM lightmap file each static mesh wears. A mesh
    /// absent from it keeps probe / ambient lighting.
    pub lightmaps: crate::scene::lighting::lightmap::LightmapSet,
    /// What Generate Lighting bakes with (#832), saved with the scene.
    pub lighting_settings: crate::scene::lighting::LightingSettings,
    /// Ids queued for deferred destruction by play-mode `Scene.DestroyEntity`
    /// (#323); drained in `runtime::destroy_queue`. Transient, never serialized.
    pub pending_destroy: Vec<u32>,
    /// Whether this World is in Play (#1298): set on entering Play, cleared on
    /// leaving it. While it holds, every destroy goes through the queue so its
    /// scripts are torn down ([`Scene::destroy`]). Transient, never serialized.
    pub playing: bool,
    /// The scene file a play-mode `Scene.Load` asked for (#432); swapped in at the
    /// tick's tail by the scene-load phase (`scene_load`). Transient, never serialized.
    pub pending_load: Option<String>,
    /// Entities marked `Scene.DontDestroyOnLoad` (#432): they and their descendants
    /// survive a scene swap. Transient runtime state, never serialized.
    pub persistent: std::collections::BTreeSet<u32>,
    /// The sim's game time (`Time::time`) the renderer uploads as the shaders'
    /// `camera.time` (#398). Written by the tick; `0` outside Play. Never serialized.
    pub shader_time: f32,
    /// The sim's unscaled clock (`Time::unscaled_time`) custom UI shaders read (#427):
    /// it keeps running at time scale 0, so a paused menu still animates. Written by
    /// the tick; `0` outside Play. Never serialized.
    pub ui_time: f32,
    /// Per-entity runtime shader-param overrides (#670). Runtime-only, never serialized.
    pub shader_overrides: crate::scene::shader_overrides::ShaderOverrides,
    /// The write-log generation (`core::fs::written`) when the meshes were last read
    /// from a document: every note up to it is in them (#1242). Never serialized.
    pub meshes_read_at: u64,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            id: SceneId::next(),
            world: World::new(),
            selected_entity_id: None,
            skybox_path: default_skybox_path(),
            ambient_color: default_ambient_color(),
            ambient_intensity: default_ambient_intensity(),
            nav_settings: NavMeshSettings::default(),
            fog: crate::scene::FogSettings::default(),
            layers: LayerRegistry::default(),
            collision_matrix: CollisionMatrix::default(),
            materials: BTreeMap::new(),
            decals: Default::default(),
            probes: crate::scene::lighting::probe::ProbeVolume::new(),
            reflection_probes: crate::scene::lighting::reflection_probe::ReflectionProbeSet::new(),
            lightmaps: Default::default(),
            lighting_settings: Default::default(),
            pending_destroy: Vec::new(),
            playing: false,
            pending_load: None,
            persistent: std::collections::BTreeSet::new(),
            shader_time: 0.0,
            ui_time: 0.0,
            shader_overrides: Default::default(),
            meshes_read_at: 0,
        }
    }
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// This scene's runtime identity (#355) — stable for the life of the object,
    /// distinct from every other live scene's.
    pub fn id(&self) -> SceneId {
        self.id
    }

    /// Mint a fresh runtime identity (#355): the World now holds a different scene
    /// (a load replaced it), so GPU caches keyed by the old identity must not apply.
    pub fn renew_id(&mut self) {
        self.id = SceneId::next();
    }

    pub fn add_entity(&mut self, name: String) -> u32 {
        self.world.spawn(name)
    }

    /// Remove entity `id` and everything under it ([`Scene::destroy_set`]), as
    /// Unity's `Object.Destroy` takes a GameObject's children (#1283). Nothing
    /// keeps naming them (`Scene::despawn_batch`). Edit mode only: in Play a
    /// destroy must tear its scripts down first, so it goes through
    /// [`Scene::destroy`] (#1298).
    pub fn destroy_entity(&mut self, id: u32) {
        debug_assert!(
            !self.playing,
            "destroy_entity({id}) in Play skips script teardown; use Scene::destroy"
        );
        let doomed = self.destroy_set(id);
        self.despawn_batch(&doomed);
    }

    /// What destroying `id` removes: `id` and every descendant, so a skinned
    /// entity's bones and what hangs from them, and a bone's subtree (#1273). The
    /// one answer [`Scene::destroy_entity`] despawns and the script teardown
    /// notifies, so the two cannot drift (#1283). Empty for a dead id.
    pub fn destroy_set(&self, id: u32) -> BTreeSet<u32> {
        self.world.self_and_descendants(id).collect()
    }

    /// Despawn every entity of `ids` together, then drop the scene's own references
    /// to them (#1264): the selection, `DontDestroyOnLoad`, a queued destroy, their
    /// lightmap entries and shader overrides. The World's references go in
    /// `World::despawn_all`, one pass for the batch. Every despawn in the scene runs
    /// through here.
    pub(crate) fn despawn_batch(&mut self, ids: &BTreeSet<u32>) {
        self.world.despawn_all(ids);
        for &id in ids {
            self.shader_overrides.clear(id, None);
        }
        if self.selected_entity_id.is_some_and(|s| ids.contains(&s)) {
            self.selected_entity_id = None;
        }
        self.persistent.retain(|id| !ids.contains(id));
        self.pending_destroy.retain(|e| !ids.contains(e));
        self.lightmaps.entries.retain(|e| !ids.contains(&e.entity));
    }

    pub fn find_entity_by_name(&self, name: &str) -> Option<u32> {
        self.world.find_by_name(name)
    }

    /// Stable ids in insertion order.
    pub fn entity_ids(&self) -> Vec<u32> {
        self.world.ids().to_vec()
    }

    pub fn entity_count(&self) -> usize {
        self.world.len()
    }

    pub fn is_empty(&self) -> bool {
        self.world.is_empty()
    }

    pub fn set_parent(&mut self, entity_id: u32, parent_id: Option<u32>) -> Result<(), String> {
        // 1. Detect cycles
        if let Some(p_id) = parent_id {
            if entity_id == p_id {
                return Err("An entity cannot be parented to itself.".to_string());
            }

            // Traverse up from parent_id to check if entity_id is an ancestor
            if self.world.ancestors(p_id).any(|a| a == entity_id) {
                return Err("Circular parenting detected (ancestor loop).".to_string());
            }
        }

        // 2. Validate everything before touching a link, so a failed call changes
        // nothing: a child dropped from its old parent's `children` while keeping its
        // `parent_id` is a one-sided link the world store never dirties again.
        if !self.world.contains(entity_id) {
            return Err("Entity not found.".to_string());
        }
        if parent_id.is_some_and(|p| !self.world.contains(p)) {
            return Err("Parent entity not found.".to_string());
        }
        if let Some(old_p) = self.world.parent_id(entity_id) {
            self.world.remove_child(old_p, entity_id);
        }

        // 3. Set new parent and update children list
        if let Some(new_p) = parent_id {
            self.world.add_child(new_p, entity_id);
        }

        // The local transform is kept, so the subtree moves in world space (#1067):
        // the parent write re-derives its world matrices and collider AABBs.
        self.world.set_parent_id(entity_id, parent_id);

        Ok(())
    }

    /// Serialize the live World to `path`. Delegates to `crate::scene::io`,
    /// which owns the on-disk `SceneData` document.
    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        crate::scene::save_to_file(self, path)
    }

    /// Load `path`, REPLACING this scene's World (single active scene).
    /// Delegates to `crate::scene::io`; meshes are rehydrated from disk there.
    pub fn load_from_file(&mut self, path: &str) -> Result<(), String> {
        crate::scene::load_from_file(self, path)
    }
}
