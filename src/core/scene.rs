use glam::{Vec3, Quat, Mat4};
use crate::render::mesh::Vertex;
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshComponent {
    pub primitive_type: String, // "Box", "Sphere", "Plane", "Cylinder", "FBX"
    #[serde(skip)]
    pub vertices: Vec<Vertex>,
    #[serde(skip)]
    pub indices: Vec<u32>,
    // For GPU rendering, we hold the loaded state or buffers in the renderer
    #[serde(skip)]
    pub is_dirty: std::cell::Cell<bool>, // Set to true when mesh data changes to update GPU buffers
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TextureComponent {
    pub path: String,
    pub is_dirty: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScriptComponent {
    pub path: String,
    pub is_loaded: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimatorComponent {
    pub current_clip: String,
    pub time: f32,
    pub speed: f32,
    pub is_playing: bool,
    pub freeze: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LightType {
    Ambient,
    Directional,
    Point,
    Spotlight,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightComponent {
    pub light_type: LightType,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub inner_cone: f32, // Degrees
    pub outer_cone: f32, // Degrees
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ColliderShape {
    Box { size: Vec3 },
    Sphere { radius: f32 },
    Cylinder { radius: f32, height: f32 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColliderComponent {
    pub active: bool,
    pub shape: ColliderShape,
    pub is_trigger: bool,
    // Cached world space bounds
    pub aabb_min: Vec3,
    pub aabb_max: Vec3,
}

impl ColliderComponent {
    pub fn calculate_world_aabb(&self, world_mat: Mat4) -> (Vec3, Vec3) {
        let local_corners = match &self.shape {
            ColliderShape::Box { size } => {
                let h = *size * 0.5;
                vec![
                    Vec3::new(-h.x, -h.y, -h.z),
                    Vec3::new(-h.x, -h.y, h.z),
                    Vec3::new(-h.x, h.y, -h.z),
                    Vec3::new(-h.x, h.y, h.z),
                    Vec3::new(h.x, -h.y, -h.z),
                    Vec3::new(h.x, -h.y, h.z),
                    Vec3::new(h.x, h.y, -h.z),
                    Vec3::new(h.x, h.y, h.z),
                ]
            }
            ColliderShape::Sphere { radius } => {
                let r = *radius;
                vec![
                    Vec3::new(-r, -r, -r),
                    Vec3::new(r, r, r),
                ]
            }
            ColliderShape::Cylinder { radius, height } => {
                let r = *radius;
                let h = *height * 0.5;
                vec![
                    Vec3::new(-r, -h, -r),
                    Vec3::new(r, h, r),
                ]
            }
        };

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for &corner in &local_corners {
            let world_pos = world_mat.transform_point3(corner);
            min = min.min(world_pos);
            max = max.max(world_pos);
        }

        (min, max)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RigidBodyComponent {
    pub active: bool,
    pub is_kinematic: bool,
    pub mass: f32,
    pub velocity: Vec3,
    pub use_gravity: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthComponent {
    pub current_health: f32,
    pub max_health: f32,
    pub is_dead: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub rigidbody: Option<RigidBodyComponent>,
    pub health: Option<HealthComponent>,
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
            transform: TransformComponent::default(),
            mesh: None,
            texture: None,
            script: None,
            animator: None,
            light: None,
            collider: None,
            rigidbody: None,
            health: None,
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

#[derive(Serialize, Deserialize)]
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

    pub fn compute_world_matrix(&self, entity_id: u32) -> Mat4 {
        if let Some(entity) = self.get_entity(entity_id) {
            let local_mat = entity.transform.to_matrix();
            if let Some(parent_id) = entity.parent_id {
                self.compute_world_matrix(parent_id) * local_mat
            } else {
                local_mat
            }
        } else {
            Mat4::IDENTITY
        }
    }

    pub fn update_entity_collider(&mut self, id: u32) {
        let parent_id = self.get_entity(id).and_then(|e| e.parent_id);
        let parent_mat = parent_id.map(|p| self.compute_world_matrix(p));
        if let Some(entity) = self.get_entity_mut(id) {
            entity.update_collider(parent_mat);
        }
    }

    pub fn update_all_colliders(&mut self) {
        let ids: Vec<u32> = self.entities.iter().map(|e| e.id).collect();
        for id in ids {
            self.update_entity_collider(id);
        }
    }

    pub fn set_parent(&mut self, entity_id: u32, parent_id: Option<u32>) -> Result<(), String> {
        // 1. Detect cycles
        if let Some(p_id) = parent_id {
            if entity_id == p_id {
                return Err("An entity cannot be parented to itself.".to_string());
            }
            
            // Traverse up from parent_id to check if entity_id is an ancestor
            let mut current = p_id;
            while let Some(ancestor_parent) = self.get_entity(current).and_then(|e| e.parent_id) {
                if ancestor_parent == entity_id {
                    return Err("Circular parenting detected (ancestor loop).".to_string());
                }
                current = ancestor_parent;
            }
        }

        // 2. Clear old parent's children list
        let old_parent_id = if let Some(entity) = self.get_entity(entity_id) {
            entity.parent_id
        } else {
            return Err("Entity not found.".to_string());
        };

        if let Some(old_p) = old_parent_id {
            if let Some(parent) = self.get_entity_mut(old_p) {
                parent.children.retain(|&c| c != entity_id);
            }
        }

        // 3. Set new parent and update children list
        if let Some(new_p) = parent_id {
            if let Some(parent) = self.get_entity_mut(new_p) {
                parent.children.push(entity_id);
            } else {
                return Err("Parent entity not found.".to_string());
            }
        }

        if let Some(entity) = self.get_entity_mut(entity_id) {
            entity.parent_id = parent_id;
        }

        Ok(())
    }

    pub fn save_to_file(&self, path: &str) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize scene: {}", e))?;
        std::fs::write(path, json)
            .map_err(|e| format!("Failed to write scene file: {}", e))?;
        Ok(())
    }

    pub fn load_from_file(&mut self, path: &str) -> Result<(), String> {
        let json = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read scene file: {}", e))?;
        let mut loaded_scene: Scene = serde_json::from_str(&json)
            .map_err(|e| format!("Failed to deserialize scene: {}", e))?;
        
        // Regenerate primitive meshes based on primitive_type
        for entity in &mut loaded_scene.entities {
            if let Some(mesh) = &mut entity.mesh {
                let p_type = mesh.primitive_type.as_str();
                let (vertices, indices) = match p_type {
                    "Box" => crate::render::mesh::generate_box(1.0, 1.0, 1.0),
                    "Sphere" => crate::render::mesh::generate_sphere(1.0, 16, 16),
                    "Plane" => crate::render::mesh::generate_plane(15.0, 15.0),
                    "Cylinder" => crate::render::mesh::generate_cylinder(glam::Vec3::new(0.0, -0.5, 0.0), glam::Vec3::new(0.0, 0.5, 0.0), 0.5, 12),
                    _ => (Vec::new(), Vec::new()),
                };
                mesh.vertices = vertices;
                mesh.indices = indices;
                mesh.is_dirty.set(true);
            }
        }

        *self = loaded_scene;
        self.update_all_colliders();
        Ok(())
    }
}
