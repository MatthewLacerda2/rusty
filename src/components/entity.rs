//! src/components/entity.rs — the per-entity component bundle.
//!
//! Each game object is one hecs entity holding a single `Entity` value (this
//! struct). Storing the whole bundle as one component keeps the editor/inspector
//! "view the whole GameObject" ergonomics while hecs owns identity + storage.
//! `transform` is the one mandatory field (Transform is never optional); every
//! other component is `Option<…>`. Moved verbatim from the legacy `core/scene.rs`.

use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

use super::{
    AnimatorComponent, AudioSourceComponent, CameraComponent, ColliderComponent, HealthComponent,
    LightComponent, MaterialAsset, MaterialComponent, MeshComponent, NavMeshAgentComponent,
    ParticleEmitterComponent, RigidBodyComponent, ScriptComponent, TextureComponent,
    TransformComponent, VisualCorrectionComponent,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(from = "EntityRepr")]
pub struct Entity {
    pub id: u32,
    pub name: String,
    pub active: bool,
    pub is_static: bool,
    /// Index into the project's shared Layers registry (Unity's per-object layer).
    /// Defaults to 0 (`"Default"`); `#[serde(default)]` so pre-#90 scenes load.
    /// Groundwork for the collision matrix (#91) and camera culling masks (#92) —
    /// it carries no behaviour on its own yet.
    #[serde(default)]
    pub layer: u8,
    pub transform: TransformComponent,
    pub mesh: Option<MeshComponent>,
    /// Reference to a library material by name (the first-class component). The
    /// material DATA is a shared asset in `Scene.materials`; this only points at it.
    pub material: Option<MaterialComponent>,
    /// Transient migration carrier: a legacy inline material lifted out of an old
    /// scene by `From<EntityRepr>`, to be inserted into the scene's material library
    /// by `apply_scene_data` (which is where the library is reachable). Never
    /// serialized; always `None` on a freshly-built/loaded runtime entity.
    #[serde(skip)]
    pub pending_material: Option<MaterialAsset>,
    /// An entity can carry MANY scripts, each its own MonoBehaviour-equivalent
    /// Lua lifecycle table (#83). `#[serde(default)]` plus the legacy `script`
    /// field in `EntityRepr` keep pre-#83 single-`script` scenes loadable.
    #[serde(default)]
    pub scripts: Vec<ScriptComponent>,
    pub animator: Option<AnimatorComponent>,
    pub light: Option<LightComponent>,
    pub collider: Option<ColliderComponent>,
    pub rigidbody: Option<RigidBodyComponent>,
    pub health: Option<HealthComponent>,
    pub nav_agent: Option<NavMeshAgentComponent>,
    pub camera: Option<CameraComponent>,
    pub visual_correction: Option<VisualCorrectionComponent>,
    #[serde(default)]
    pub particles: Option<ParticleEmitterComponent>,
    /// Per-entity 2D audio emitter (#212). `#[serde(default)]` so pre-#212 scenes
    /// load with no audio source.
    #[serde(default)]
    pub audio: Option<AudioSourceComponent>,
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
}

/// On-disk shape used only for deserialization, so old single-`script` scenes
/// (pre-#83) keep loading. It accepts both the new `scripts: Vec<…>` and the
/// legacy `script: Option<…>` keys; the `From` impl migrates the singular into
/// the vec. Serialization always goes through `Entity` directly (new `scripts`
/// key), so files written today never carry the legacy field.
#[derive(Deserialize)]
struct EntityRepr {
    id: u32,
    name: String,
    active: bool,
    is_static: bool,
    #[serde(default)]
    layer: u8,
    transform: TransformComponent,
    mesh: Option<MeshComponent>,
    /// Legacy pre-#201 inline material, migrated into a library material by `From`.
    #[serde(default)]
    texture: Option<TextureComponent>,
    /// New reference-to-library material (post-#201 scenes carry this directly).
    #[serde(default)]
    material: Option<MaterialComponent>,
    #[serde(default)]
    scripts: Vec<ScriptComponent>,
    /// Legacy singular field (pre-#83), migrated into `scripts` by `From`.
    #[serde(default)]
    script: Option<ScriptComponent>,
    animator: Option<AnimatorComponent>,
    light: Option<LightComponent>,
    collider: Option<ColliderComponent>,
    rigidbody: Option<RigidBodyComponent>,
    health: Option<HealthComponent>,
    nav_agent: Option<NavMeshAgentComponent>,
    camera: Option<CameraComponent>,
    visual_correction: Option<VisualCorrectionComponent>,
    #[serde(default)]
    particles: Option<ParticleEmitterComponent>,
    #[serde(default)]
    audio: Option<AudioSourceComponent>,
    parent_id: Option<u32>,
    children: Vec<u32>,
}

impl From<EntityRepr> for Entity {
    fn from(r: EntityRepr) -> Self {
        let mut scripts = r.scripts;
        // Migrate a pre-#83 single `script` into the plural vec, ahead of any
        // (normally empty) new-format scripts, so legacy attachments still run.
        if let Some(legacy) = r.script {
            scripts.insert(0, legacy);
        }
        // Material migration: a new-format `material` reference is taken verbatim; a
        // legacy inline `texture` becomes a per-entity library material whose data is
        // carried in `pending_material` for `apply_scene_data` to insert by name.
        let (material, pending_material) = match (r.material, r.texture) {
            (Some(m), _) => (Some(m), None),
            (None, Some(t)) => (
                Some(MaterialComponent {
                    material: format!("entity_{}_material", r.id),
                }),
                Some(MaterialAsset::from_legacy(&t)),
            ),
            (None, None) => (None, None),
        };
        Self {
            id: r.id,
            name: r.name,
            active: r.active,
            is_static: r.is_static,
            layer: r.layer,
            transform: r.transform,
            mesh: r.mesh,
            material,
            pending_material,
            scripts,
            animator: r.animator,
            light: r.light,
            collider: r.collider,
            rigidbody: r.rigidbody,
            health: r.health,
            nav_agent: r.nav_agent,
            camera: r.camera,
            visual_correction: r.visual_correction,
            particles: r.particles,
            audio: r.audio,
            parent_id: r.parent_id,
            children: r.children,
        }
    }
}

impl Entity {
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            active: true,
            is_static: false,
            layer: 0,
            transform: TransformComponent::default(),
            mesh: None,
            material: None,
            pending_material: None,
            scripts: Vec::new(),
            animator: None,
            light: None,
            collider: None,
            rigidbody: None,
            health: None,
            nav_agent: None,
            camera: None,
            visual_correction: None,
            particles: None,
            audio: None,
            parent_id: None,
            children: Vec::new(),
        }
    }

    pub fn compute_world_aabb(&self, world_matrix: Mat4) -> Option<(Vec3, Vec3)> {
        let mesh = self.mesh.as_ref()?;
        if mesh.vertices.is_empty() {
            return None;
        }

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for v in &mesh.vertices {
            let local_pos = Vec3::from_array(v.position);
            let world_pos = world_matrix.transform_point3(local_pos);
            min = min.min(world_pos);
            max = max.max(world_pos);
        }

        Some((min, max))
    }

    pub fn update_collider(&mut self, parent_world_matrix: Option<Mat4>) {
        if let Some(col) = &mut self.collider {
            let world_matrix = if let Some(parent_mat) = parent_world_matrix {
                parent_mat * self.transform.to_matrix()
            } else {
                self.transform.to_matrix()
            };
            let (min, max) = col.calculate_world_aabb(world_matrix);
            col.aabb_min = min;
            col.aabb_max = max;
        }
    }
}
