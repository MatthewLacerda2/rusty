mod partial;
mod preset;
mod runtime;
mod switch;

pub use partial::HitReact;
pub use runtime::{critical_damping, Drive, RagdollRuntime, REACT_DURATION, REACT_STRENGTH};

use glam::Vec3;

use super::HitboxOptions;
use crate::Scene;
use preset::{preset_for, Bend, JointPreset};
use rusty_components::{ColliderShape, JointComponent, RigidBodyComponent};

pub const RAGDOLL_LAYER: &str = "Ragdoll";

type Skeleton = (Vec<Option<u32>>, Vec<Option<usize>>, Vec<String>);

#[derive(Clone, Debug, PartialEq)]
pub struct RagdollOptions {
    /// The whole body's mass, in kilograms.
    pub mass: f32,
}

impl Default for RagdollOptions {
    fn default() -> Self {
        Self { mass: 70.0 }
    }
}

impl Scene {
    pub fn build_ragdoll(
        &mut self,
        owner: u32,
        opts: &RagdollOptions,
    ) -> Result<Vec<(String, u32)>, String> {
        self.sync_skeleton(owner);
        let (bones, parents, names) = self.skeleton_of(owner)?;
        if bones.iter().flatten().all(|&b| self.hitbox_of(b).is_none()) {
            self.generate_hitboxes(owner, &HitboxOptions::default())?;
        }
        self.ragdoll_layer()?;
        let volumes: Vec<Option<f32>> = bones
            .iter()
            .map(|b| self.hitbox_of((*b)?).map(|h| self.collider_volume(h)))
            .collect();
        let total: f32 = volumes.iter().flatten().sum::<f32>().max(f32::EPSILON);
        let owner_axes = self.front_and_side(owner);
        let mut made = Vec::new();
        for (slot, &bone) in bones.iter().enumerate() {
            let Some(bone) = bone else {
                continue;
            };
            let Some(volume) = volumes[slot] else {
                self.world.set_rigidbody(bone, None);
                self.world.set_joint(bone, None);
                // Its runtime state goes with its body.
                self.ragdolls.retain(|b| b != bone);
                continue;
            };
            let body = RigidBodyComponent {
                is_kinematic: true,
                mass: opts.mass * volume / total,
                ..crate::create::default_rigidbody()
            };
            self.world.set_rigidbody(bone, Some(body));
            let mut up = parents[slot];
            while let Some(p) = up.filter(|&p| volumes[p].is_none()) {
                up = parents[p];
            }
            let joint = up.map(|p| {
                let dir = self.bone_direction(&bones, &parents, slot);
                let mut j = joint_from(preset_for(&names[slot]), dir, owner_axes);
                j.connected_body = bones[p];
                // The exact inverse of the world frame `physics::joints`' `build_joint` gives a
                // joint: change the two together.
                (j.axis, j.swing_axis) =
                    self.world
                        .inverse_transform_frame(bone, j.axis, j.swing_axis);
                j
            });
            self.world.set_joint(bone, joint);
            made.push((names[slot].clone(), bone));
        }
        Ok(made)
    }

    pub fn ragdoll_bones(&self, id: u32) -> Vec<u32> {
        let mut out = Vec::new();
        for e in self.world.self_and_descendants(id) {
            if let Some(mesh) = self.world.mesh(e) {
                out.extend(mesh.skeleton.bones.iter().flatten().copied());
            }
        }
        out.retain(|&b| self.world.has_rigidbody(b));
        out.sort_unstable();
        out
    }

    fn skeleton_of(&self, owner: u32) -> Result<Skeleton, String> {
        let mesh = self
            .world
            .mesh(owner)
            .ok_or_else(|| format!("entity {owner} has no mesh"))?;
        let skin = mesh
            .skin
            .as_ref()
            .ok_or_else(|| format!("entity {owner}'s mesh is not skinned"))?;
        let names = (0..skin.local_bind.len()).map(|s| skin.name(s)).collect();
        let parents = (0..skin.local_bind.len())
            .map(|s| skin.parents.get(s).copied().flatten())
            .collect();
        Ok((mesh.skeleton.bones.clone(), parents, names))
    }

    fn bone_direction(
        &self,
        bones: &[Option<u32>],
        parents: &[Option<usize>],
        slot: usize,
    ) -> Vec3 {
        let at = |s: usize| Some(self.world.world_position(bones[s]?));
        let children: Vec<Vec3> = (0..bones.len())
            .filter(|&c| parents[c] == Some(slot))
            .filter_map(at)
            .collect();
        let dir = match (children.is_empty(), parents[slot].and_then(at), at(slot)) {
            (_, _, None) => Vec3::Y,
            (false, _, Some(here)) => children.iter().sum::<Vec3>() / children.len() as f32 - here,
            (true, Some(up), Some(here)) => here - up,
            (true, None, Some(_)) => Vec3::Y,
        };
        dir.try_normalize().unwrap_or(Vec3::Y)
    }

    /// The model's +Z and +X as drawn, in world space, taken through the skin's model axes
    /// so a mesh under a Blender export's turned `Armature` still bends its knees forward.
    fn front_and_side(&self, owner: u32) -> (Vec3, Vec3) {
        let w = &self.world;
        let model = w
            .mesh(owner)
            .and_then(|m| m.skin.as_ref()?.mesh_to_model_axes());
        let to_mesh = model.map(|m| m.inverse());
        let axis = |model: Vec3| {
            let local = to_mesh.map_or(model, |m| m.transform_vector3(model).normalize());
            w.transform_direction(owner, local)
        };
        (axis(Vec3::Z), axis(Vec3::X))
    }

    fn collider_volume(&self, id: u32) -> f32 {
        let scale = self.world.lossy_scale(id);
        let pi = std::f32::consts::PI;
        let local = match self.world.collider(id).map(|c| c.shape.clone()) {
            Some(ColliderShape::Box { size }) => size.x * size.y * size.z,
            Some(ColliderShape::Sphere { radius }) => 4.0 / 3.0 * pi * radius.powi(3),
            Some(ColliderShape::Capsule { radius, height, .. }) => {
                pi * radius * radius * (height - 2.0 * radius).max(0.0)
                    + 4.0 / 3.0 * pi * radius.powi(3)
            }
            Some(ColliderShape::Cylinder { radius, height }) => pi * radius * radius * height,
            _ => 0.0,
        };
        (local * scale.x * scale.y * scale.z).abs()
    }
}

fn joint_from(p: JointPreset, dir: Vec3, (front, side): (Vec3, Vec3)) -> JointComponent {
    let bend = match p.bend {
        Bend::Forward => front,
        Bend::Backward => -front,
    };
    let axis = dir.cross(bend).try_normalize().unwrap_or(side);
    JointComponent {
        kind: p.kind,
        axis,
        swing_axis: bend,
        use_limits: true,
        limits: flex_limits(p.flex),
        swing_limit: p.swing1,
        swing2_limit: p.swing2,
        enable_collision: false,
        ..JointComponent::default()
    }
}

/// rapier measures a joint's angle as the parent's turn seen from the bone, so a bone
/// flexing by `+a` reads `-a`.
fn flex_limits((min, max): (f32, f32)) -> glam::Vec2 {
    glam::Vec2::new(-max, -min)
}
