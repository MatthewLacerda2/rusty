use glam::{Vec3, Quat, Mat4};
use crate::render::mesh::Vertex;

#[derive(Clone, Debug)]
pub struct TransformComponent {
    pub position: Vec3,
    pub rotation: Quat, // We will also support Euler representation in UI
    pub scale: Vec3,
}

impl Default for TransformComponent {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

impl TransformComponent {
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    pub fn euler_angles(&self) -> Vec3 {
        let (yaw, pitch, roll) = self.rotation.to_euler(glam::EulerRot::YXZ);
        Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees())
    }

    pub fn set_euler_angles(&mut self, euler_deg: Vec3) {
        let yaw = euler_deg.y.to_radians();
        let pitch = euler_deg.x.to_radians();
        let roll = euler_deg.z.to_radians();
        self.rotation = Quat::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll);
    }
}

#[derive(Clone, Debug)]
pub struct MeshComponent {
    pub primitive_type: String, // "Box", "Sphere", "Plane", "Cylinder", "FBX"
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    // For GPU rendering, we hold the loaded state or buffers in the renderer
    pub is_dirty: std::cell::Cell<bool>, // Set to true when mesh data changes to update GPU buffers
}

#[derive(Clone, Debug)]
pub struct TextureComponent {
    pub path: String,
    pub is_dirty: bool,
}

#[derive(Clone, Debug)]
pub struct ScriptComponent {
    pub path: String,
    pub is_loaded: bool,
}

#[derive(Clone, Debug)]
pub struct AnimatorComponent {
    pub current_clip: String,
    pub time: f32,
    pub speed: f32,
    pub is_playing: bool,
    pub freeze: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LightType {
    Ambient,
    Directional,
    Point,
    Spotlight,
}

#[derive(Clone, Debug)]
pub struct LightComponent {
    pub light_type: LightType,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub inner_cone: f32, // Degrees
    pub outer_cone: f32, // Degrees
}

#[derive(Clone, Debug)]
pub struct ColliderComponent {
    pub active: bool,
    // Cached world space bounds
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
}

#[derive(Clone, Debug)]
pub struct HealthComponent {
    pub current_health: f32,
    pub max_health: f32,
    pub is_dead: bool,
}

#[derive(Clone, Debug)]
pub struct Entity {
    pub id: u32,
    pub name: String,
    pub active: bool,
    pub is_static: bool,
    pub transform: TransformComponent,
    pub mesh: Option<MeshComponent>,
    pub texture: Option<TextureComponent>,
    pub script: Option<ScriptComponent>,
    pub animator: Option<AnimatorComponent>,
    pub light: Option<LightComponent>,
    pub collider: Option<ColliderComponent>,
    pub health: Option<HealthComponent>,
}

impl Entity {
    pub fn new(id: u32, name: String) -> Self {
        Self {
            id,
            name,
            active: true,
            is_static: false,
            transform: TransformComponent::default(),
            mesh: None,
            texture: None,
            script: None,
            animator: None,
            light: None,
            collider: None,
            health: None,
        }
    }

    pub fn compute_world_aabb(&self) -> Option<(Vec3, Vec3)> {
        let mesh = self.mesh.as_ref()?;
        if mesh.vertices.is_empty() {
            return None;
        }

        let mat = self.transform.to_matrix();
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for v in &mesh.vertices {
            let local_pos = Vec3::from_array(v.position);
            let world_pos = mat.transform_point3(local_pos);
            min = min.min(world_pos);
            max = max.max(world_pos);
        }

        Some((min, max))
    }

    pub fn update_collider(&mut self) {
        if self.collider.is_some() {
            if let Some((min, max)) = self.compute_world_aabb() {
                let col = self.collider.as_mut().unwrap();
                col.aabb_min = min;
                col.aabb_max = max;
            }
        }
    }
}

pub struct Scene {
    pub entities: Vec<Entity>,
    pub next_entity_id: u32,
    pub selected_entity_id: Option<u32>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            entities: Vec::new(),
            next_entity_id: 1,
            selected_entity_id: None,
        }
    }
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_entity(&mut self, name: String) -> u32 {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        self.entities.push(Entity::new(id, name));
        id
    }

    pub fn destroy_entity(&mut self, id: u32) {
        self.entities.retain(|e| e.id != id);
        if self.selected_entity_id == Some(id) {
            self.selected_entity_id = None;
        }
    }

    pub fn get_entity(&self, id: u32) -> Option<&Entity> {
        self.entities.iter().find(|e| e.id == id)
    }

    pub fn get_entity_mut(&mut self, id: u32) -> Option<&mut Entity> {
        self.entities.iter_mut().find(|e| e.id == id)
    }

    pub fn find_entity_by_name(&self, name: &str) -> Option<u32> {
        self.entities.iter()
            .find(|e| e.name == name && e.active)
            .map(|e| e.id)
    }

    pub fn update_all_colliders(&mut self) {
        for entity in &mut self.entities {
            entity.update_collider();
        }
    }
}
