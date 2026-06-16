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
    AnimatorComponent, CameraComponent, ColliderComponent, HealthComponent, LightComponent,
    MeshComponent, NavMeshAgentComponent, ParticleEmitterComponent, RigidBodyComponent,
    ScriptComponent, TextureComponent, TransformComponent, VisualCorrectionComponent,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub texture: Option<TextureComponent>,
    pub script: Option<ScriptComponent>,
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
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
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
            texture: None,
            script: None,
            animator: None,
            light: None,
            collider: None,
            rigidbody: None,
            health: None,
            nav_agent: None,
            camera: None,
            visual_correction: None,
            particles: None,
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
