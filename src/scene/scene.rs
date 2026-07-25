//! src/scene/scene.rs — Scene: the hecs-backed `ecs::World` plus scene-level state.
//!
//! Entity/component storage of record is `ecs::World` (one `Entity` bundle per
//! hecs entity, generational handles + stable-id/name lookup). `Scene` owns that
//! `World` together with the engine-level scene scalars (selection, skybox,
//! ambient light) and the parenting / collider / serialization helpers that
//! operate across the World — concerns that must NOT live inside `ecs::World`
//! (whose deps are restricted to hecs + components). This wrapper lives in the
//! `scene` layer, which is allowed to depend on `crate::scene` and rendering.

use std::collections::BTreeMap;

use glam::Vec3;

use crate::ecs::World;
use crate::navigation::NavMeshSettings;
use crate::scene::collision_matrix::CollisionMatrix;
use crate::scene::identity::SceneId;
use crate::scene::layers::LayerRegistry;
use crate::scene::world_cache::WorldMatrixCache;

// Re-export the component types so the many existing `scene::…Component` paths
// resolve. The structs themselves live in `crate::components`.
pub use crate::components::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ClearFlags, ColliderComponent,
    ColliderShape, CollisionDetection, CollisionResponse, DirtyFlag, EmitMode, Entity,
    LightComponent, LightType, MaterialAsset, MaterialComponent, MeshComponent,
    NavMeshAgentComponent, Particle, ParticleBlend, ParticleEmitterComponent, RenderMode,
    RigidBodyComponent, ScriptComponent, ScriptFieldValue, TextureComponent, Tonemap,
    TransformComponent, VisualCorrectionComponent,
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
    /// scalars, with serde defaults so older scenes load with the historical values.
    pub nav_settings: NavMeshSettings,
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
    /// surfaces they overlap. Bounded FIFO (oldest evicted past `MAX_DECALS`).
    pub decals: Vec<crate::render::passes::decals::Decal>,
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
    /// Ids queued for deferred destruction by play-mode `Scene.DestroyEntity`
    /// (#323); drained in `destroy_queue`. Transient, never serialized.
    pub pending_destroy: Vec<u32>,
    /// Authoritative per-frame world-matrix store (#331): filled O(N) once per frame in
    /// hierarchy order, then read by every render-frame consumer (the forward pass, both
    /// shadow collects, and #330's frustum culling) instead of each walking the parent
    /// chain itself. Not serialized — it is derived, per-frame runtime state. See
    /// `world_cache.rs` for the fill algorithm and the freshness contract.
    pub(crate) world_cache: WorldMatrixCache,
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
            layers: LayerRegistry::default(),
            collision_matrix: CollisionMatrix::default(),
            materials: BTreeMap::new(),
            decals: Vec::new(),
            probes: crate::scene::lighting::probe::ProbeVolume::new(),
            reflection_probes: crate::scene::lighting::reflection_probe::ReflectionProbeSet::new(),
            pending_destroy: Vec::new(),
            world_cache: WorldMatrixCache::default(),
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

    /// Spawn a box-projector decal at a surface hit (the point + outward normal
    /// already produced by `Physics.Raycast`). `size` is the
    /// stamp's width/height in world units; `depth` how far the box projects
    /// through the surface; `rotation_deg` spins the stamp around its axis;
    /// `color` tints the texel (alpha scales the blend); `texture` is the decal
    /// sprite (or the default checker). The registry is a bounded FIFO so spam
    /// can't grow it without limit.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn_decal(
        &mut self,
        point: Vec3,
        normal: Vec3,
        size: f32,
        depth: f32,
        rotation_deg: f32,
        color: [f32; 4],
        texture: Option<String>,
    ) {
        let decal = crate::render::passes::decals::Decal::from_hit(
            point,
            normal,
            size.max(1.0e-4),
            depth.max(1.0e-4),
            rotation_deg,
            color,
            texture,
        );
        if self.decals.len() >= crate::render::passes::decals::MAX_DECALS {
            self.decals.remove(0);
        }
        self.decals.push(decal);
    }

    /// Drop every live decal (e.g. on level reset).
    pub fn clear_decals(&mut self) {
        self.decals.clear();
    }

    pub fn add_entity(&mut self, name: String) -> u32 {
        self.world.spawn(name)
    }

    pub fn destroy_entity(&mut self, id: u32) {
        self.world.despawn(id);
        if self.selected_entity_id == Some(id) {
            self.selected_entity_id = None;
        }
    }

    pub fn find_entity_by_name(&self, name: &str) -> Option<u32> {
        self.world.find_by_name(name)
    }

    /// Resolve an entity's referenced material from the library, if any. Returns
    /// `None` when the entity has no `MaterialComponent` or it points at a missing
    /// library key.
    pub fn material_asset_of(&self, id: u32) -> Option<&MaterialAsset> {
        let key = self.world.material(id)?.material.clone();
        self.materials.get(&key)
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
            let mut current = p_id;
            while let Some(ancestor_parent) = self.world.parent_id(current) {
                if ancestor_parent == entity_id {
                    return Err("Circular parenting detected (ancestor loop).".to_string());
                }
                current = ancestor_parent;
            }
        }

        // 2. Clear old parent's children list
        if !self.world.contains(entity_id) {
            return Err("Entity not found.".to_string());
        }
        if let Some(old_p) = self.world.parent_id(entity_id) {
            self.world.remove_child(old_p, entity_id);
        }

        // 3. Set new parent and update children list
        if let Some(new_p) = parent_id {
            if !self.world.add_child(new_p, entity_id) {
                return Err("Parent entity not found.".to_string());
            }
        }

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
