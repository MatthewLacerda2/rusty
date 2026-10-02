//! Ragdolls (#466): a skinned character's hitboxes turned into jointed bodies.
//!
//! [`Scene::build_ragdoll`] is Unity's Ragdoll Wizard: it puts a `Rigidbody` on
//! every bone that has a hitbox (generating the hitboxes first if there are none)
//! and a `Joint` to its nearest ragdolled ancestor, shaped by the humanoid preset
//! (`preset`). The output is ordinary components on the bones, saved by bone name
//! (`BoneBinding::bodies`), so it stays hand-tweakable.
//!
//! **Mass.** The total `mass` is split over the bodies by hitbox volume, so a
//! chest outweighs a hand whatever the rig.
//!
//! **Animated or simulated.** The bodies start kinematic: the Animator poses the
//! bones and the bodies follow. [`Scene::set_ragdoll`] flips them dynamic (or back)
//! and, on the way in, seeds each body with the velocity its animation gave it.
//! While a body is dynamic the physics step owns its bone; the post-animation
//! writer (`PhysicsWorld::pose_bones`) overrides the Animator before skinning.
//!
//! **Layers.** Alive, a hitbox sits on the `Hitbox` layer, which collides with
//! nothing, so the character's capsule moves it. Ragdolled, it moves to the
//! `Ragdoll` layer — created on first use, colliding with every layer except
//! `Hitbox` — so the body falls on the floor and onto other bodies. Joined bones
//! never collide with each other (`Joint::enable_collision` off).

mod preset;
mod switch;

use glam::{Quat, Vec3};

use super::HitboxOptions;
use crate::components::{ColliderShape, CollisionDetection, JointComponent, RigidBodyComponent};
use crate::scene::Scene;
use preset::{preset_for, Bend, JointPreset};

/// The layer ragdolled hitboxes move to.
pub const RAGDOLL_LAYER: &str = "Ragdoll";

/// A skeleton's bones, their parent slots and their names, in joint order.
type Skeleton = (Vec<u32>, Vec<Option<usize>>, Vec<String>);

/// What [`Scene::build_ragdoll`] builds.
#[derive(Clone, Debug, PartialEq)]
pub struct RagdollOptions {
    /// The whole body's mass (kilograms), split over the bones by volume.
    pub mass: f32,
}

impl Default for RagdollOptions {
    fn default() -> Self {
        Self { mass: 70.0 }
    }
}

impl Scene {
    /// Build (or rebuild) `owner`'s ragdoll and return its bodies as `(bone name,
    /// bone)` pairs in joint order. Bones without a hitbox lose any Rigidbody and
    /// Joint. `Err` when `owner` is not skinned or no layer is free.
    pub fn build_ragdoll(
        &mut self,
        owner: u32,
        opts: &RagdollOptions,
    ) -> Result<Vec<(String, u32)>, String> {
        self.sync_skeleton(owner);
        let (bones, parents, names) = self.skeleton_of(owner)?;
        if bones.iter().all(|&b| self.hitbox_of(b).is_none()) {
            self.generate_hitboxes(owner, &HitboxOptions::default())?;
        }
        self.ragdoll_layer()?;
        let volumes: Vec<Option<f32>> = bones
            .iter()
            .map(|&b| self.hitbox_of(b).map(|h| self.collider_volume(h)))
            .collect();
        let total: f32 = volumes.iter().flatten().sum::<f32>().max(f32::EPSILON);
        let owner_rot = self.world_rotation(owner);
        let mut made = Vec::new();
        for (slot, &bone) in bones.iter().enumerate() {
            let Some(volume) = volumes[slot] else {
                self.world.set_rigidbody(bone, None);
                self.world.set_joint(bone, None);
                continue;
            };
            let body = RigidBodyComponent {
                active: true,
                is_kinematic: true,
                mass: opts.mass * volume / total,
                velocity: Vec3::ZERO,
                angular_velocity: Vec3::ZERO,
                use_gravity: true,
                collision_detection: CollisionDetection::Discrete,
            };
            self.world.set_rigidbody(bone, Some(body));
            let mut up = parents[slot];
            while let Some(p) = up.filter(|&p| volumes[p].is_none()) {
                up = parents[p];
            }
            let joint = up.map(|p| {
                let dir = self.bone_direction(&bones, &parents, slot);
                let mut j = joint_from(preset_for(&names[slot]), dir, owner_rot);
                j.connected_body = Some(bones[p]);
                local_axes(&mut j, self.world_rotation(bone));
                j
            });
            self.world.set_joint(bone, joint);
            made.push((names[slot].clone(), bone));
        }
        Ok(made)
    }

    /// Every ragdoll bone (a bone with a Rigidbody) of every skinned mesh at or
    /// under `id`, ascending.
    pub fn ragdoll_bones(&self, id: u32) -> Vec<u32> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(e) = stack.pop() {
            stack.extend(self.world.children(e));
            if let Some(mesh) = self.world.mesh(e) {
                out.extend(mesh.skeleton.bones.iter().copied());
            }
        }
        out.retain(|&b| self.world.has_rigidbody(b));
        out.sort_unstable();
        out
    }

    /// `owner`'s bones, their parent slots and names, in joint order.
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

    /// The world direction bone `slot` points: toward the mean of its child
    /// bones, or on from its parent when it has none.
    fn bone_direction(&self, bones: &[u32], parents: &[Option<usize>], slot: usize) -> Vec3 {
        let at = |s: usize| self.compute_world_matrix(bones[s]).w_axis.truncate();
        let children: Vec<Vec3> = (0..bones.len())
            .filter(|&c| parents[c] == Some(slot))
            .map(at)
            .collect();
        let dir = match (children.is_empty(), parents[slot]) {
            (false, _) => children.iter().sum::<Vec3>() / children.len() as f32 - at(slot),
            (true, Some(p)) => at(slot) - at(p),
            (true, None) => Vec3::Y,
        };
        dir.try_normalize().unwrap_or(Vec3::Y)
    }

    fn world_rotation(&self, id: u32) -> Quat {
        self.compute_world_matrix(id)
            .to_scale_rotation_translation()
            .1
    }

    /// A collider's volume at its world scale — what its share of the mass is
    /// weighed by.
    fn collider_volume(&self, id: u32) -> f32 {
        let scale = self
            .compute_world_matrix(id)
            .to_scale_rotation_translation()
            .0;
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

/// A `Joint` in world terms from `p`: its axis is the one the bone (pointing
/// `dir`) flexes about toward `p.bend` in the character's frame (`owner_rot`), its
/// swing axis that bend direction.
fn joint_from(p: JointPreset, dir: Vec3, owner_rot: Quat) -> JointComponent {
    let bend = owner_rot
        * match p.bend {
            Bend::Forward => Vec3::Z,
            Bend::Backward => -Vec3::Z,
        };
    let axis = dir
        .cross(bend)
        .try_normalize()
        .unwrap_or(owner_rot * Vec3::X);
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

/// rapier measures a joint's angle as the connected body's turn relative to this
/// one's — the parent's turn seen from the bone — so a bone flexing by `+a`
/// reads `-a`.
fn flex_limits((min, max): (f32, f32)) -> glam::Vec2 {
    glam::Vec2::new(-max, -min)
}

/// Turn a joint's world-space axes into `bone`'s local space (`bone_rot`).
fn local_axes(j: &mut JointComponent, bone_rot: Quat) {
    let inv = bone_rot.inverse();
    j.axis = inv * j.axis;
    j.swing_axis = inv * j.swing_axis;
}

#[cfg(test)]
mod persist_tests;
