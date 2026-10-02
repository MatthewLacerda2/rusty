//! Switching a ragdoll between the Animator and physics (#466), and the layer
//! its hitboxes move to while it is simulated.

use glam::Vec3;

use super::RAGDOLL_LAYER;
use crate::scene::layers::LAYER_COUNT;
use crate::scene::skeleton::HitboxOptions;
use crate::scene::Scene;

impl Scene {
    /// Switch the ragdoll at or under `id` to physics (`on`) or back to the
    /// Animator, and return how many bodies switched. Switching on seeds each body
    /// with `velocity(bone)` (linear, angular) — what its animation was doing —
    /// and moves its hitboxes from the hitbox layer to the ragdoll layer;
    /// switching off reverses both and stops the bodies.
    pub fn set_ragdoll(
        &mut self,
        id: u32,
        on: bool,
        velocity: impl Fn(u32) -> Option<(Vec3, Vec3)>,
    ) -> Result<usize, String> {
        let bones = self.ragdoll_bones(id);
        let hitbox = self.layers.index_of(&HitboxOptions::default().layer);
        let ragdoll = match on {
            true => Some(self.ragdoll_layer()?),
            false => self.layers.index_of(RAGDOLL_LAYER),
        };
        let (from, to) = if on {
            (hitbox, ragdoll)
        } else {
            (ragdoll, hitbox)
        };
        for &bone in &bones {
            let (v, w) = match on {
                true => velocity(bone).unwrap_or_default(),
                false => Default::default(),
            };
            if let Some(mut rb) = self.world.rigidbody_mut(bone) {
                rb.is_kinematic = !on;
                rb.velocity = v;
                rb.angular_velocity = w;
            }
            if let (Some(from), Some(to)) = (from, to) {
                self.move_part_layers(bone, from, to);
            }
        }
        Ok(bones.len())
    }

    /// Whether any ragdoll body at or under `id` is simulated.
    pub fn ragdoll_enabled(&self, id: u32) -> bool {
        self.ragdoll_bones(id)
            .into_iter()
            .any(|b| self.world.rigidbody(b).is_some_and(|rb| !rb.is_kinematic))
    }

    /// Move the colliders that are `bone`'s body parts — its own and its
    /// non-bone children's — from layer `from` to `to`.
    fn move_part_layers(&mut self, bone: u32, from: u8, to: u8) {
        let parts = std::iter::once(bone).chain(
            self.world
                .children(bone)
                .into_iter()
                .filter(|&c| self.bone_owner(c).is_none()),
        );
        let parts: Vec<u32> = parts
            .filter(|&p| self.world.has_collider(p) && self.world.layer(p) == from)
            .collect();
        for p in parts {
            self.world.set_layer(p, to);
        }
    }

    /// The ragdoll layer, claimed on first use in the first free slot: it collides
    /// with every layer but the hitbox layer. An existing layer keeps its matrix.
    pub(super) fn ragdoll_layer(&mut self) -> Result<u8, String> {
        if let Some(layer) = self.layers.index_of(RAGDOLL_LAYER) {
            return Ok(layer);
        }
        let free = self
            .layers
            .iter()
            .find(|&(i, n)| i > 0 && n.is_empty())
            .map(|(i, _)| i)
            .ok_or_else(|| format!("no free layer left for '{RAGDOLL_LAYER}'"))?;
        self.layers.set_name(free, RAGDOLL_LAYER);
        let hitbox = self.layers.index_of(&HitboxOptions::default().layer);
        for other in 0..LAYER_COUNT as u8 {
            self.collision_matrix
                .set_collision(free, other, Some(other) != hitbox);
        }
        Ok(free)
    }
}
