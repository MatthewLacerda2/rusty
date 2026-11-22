use glam::camera::rh::view::look_to_mat4;
use glam::{Mat4, Quat, Vec3};
use rusty_asset::MapRef;

mod counts;
mod persist;
mod pose;
mod registry;

pub use pose::DecalPose;
pub use registry::{DecalSet, EVICTION_FADE, EVICTION_HEADROOM};

#[derive(Clone, Debug, PartialEq)]
pub struct DecalSpec {
    pub size: f32,
    pub depth: f32,
    pub rotation_deg: f32,
    pub color: [f32; 4],
    pub texture: Option<MapRef>,
    pub material: Option<String>,
    pub owner: Option<u32>,
    pub lifetime: Option<f32>,
    pub fade: f32,
    pub persist: bool,
}

impl Default for DecalSpec {
    fn default() -> Self {
        Self {
            size: 0.5,
            depth: 0.5,
            rotation_deg: 0.0,
            color: [1.0; 4],
            texture: None,
            material: None,
            owner: None,
            lifetime: None,
            fade: 0.0,
            persist: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Decal {
    pub id: u32,
    pub position: Vec3,
    pub rotation: Quat,
    pub size: Vec3,
    pub color: [f32; 4],
    pub texture: Option<MapRef>,
    pub material: Option<String>,
    pub owner: Option<DecalOwner>,
    pub age: f32,
    pub lifetime: Option<f32>,
    pub fade: f32,
    pub retiring: bool,
    pub evicted: bool,
    pub persist: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecalOwner {
    pub entity: u32,
    pub local: Mat4,
}

impl Decal {
    pub fn bucket(&self) -> Option<u32> {
        self.owner.map(|o| o.entity)
    }

    pub fn from_hit(point: Vec3, normal: Vec3, spec: DecalSpec) -> Self {
        let n = normal.normalize_or_zero();
        let n = if n.length_squared() < 1.0e-6 {
            Vec3::Y
        } else {
            n
        };
        let mut rot = quat_look_along(-n);
        rot *= Quat::from_axis_angle(Vec3::Z, spec.rotation_deg.to_radians());
        let depth = spec.depth.max(1.0e-4);
        let size = spec.size.max(1.0e-4);
        Self {
            id: 0,
            position: point,
            rotation: rot,
            size: Vec3::new(size, size, depth),
            color: spec.color,
            texture: spec.texture,
            material: spec.material,
            owner: None,
            age: 0.0,
            lifetime: spec.lifetime.map(|l| l.max(0.0)),
            fade: spec.fade.max(0.0),
            retiring: false,
            evicted: false,
            persist: spec.persist,
        }
    }

    /// Object→world matrix mapping the unit cube `[-0.5, 0.5]³` to this box.
    pub fn model_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.size, self.rotation, self.position)
    }

    pub fn opacity(&self) -> f32 {
        match self.lifetime {
            Some(end) if self.fade > 0.0 => ((end - self.age) / self.fade).clamp(0.0, 1.0),
            Some(end) if self.age >= end => 0.0,
            _ => 1.0,
        }
    }

    pub fn expired(&self) -> bool {
        self.lifetime.is_some_and(|end| self.age >= end)
    }

    pub fn retire(&mut self, seconds: f32) {
        self.retiring = true;
        let seconds = seconds.max(0.0);
        let end = self.age + seconds;
        if self.lifetime.is_some_and(|own| own <= end) {
            return;
        }
        self.fade = seconds / self.opacity().max(1.0e-3);
        self.lifetime = Some(end);
    }
}

fn quat_look_along(dir: Vec3) -> Quat {
    let f = dir.normalize_or_zero();
    let up = if f.y.abs() > 0.99 { Vec3::X } else { Vec3::Y };
    Quat::from_mat4(&look_to_mat4(Vec3::ZERO, f, up)).inverse()
}

#[cfg(test)]
#[path = "decal_tests.rs"]
mod tests;
