//! Per-bone hitboxes (#464): one `Collider` per body part, on a child of its bone.
//!
//! [`Scene::generate_hitboxes`] fits a box or capsule to each bone's skinned
//! vertices (see `fit`) and puts it on a child entity named [`HITBOX_NAME`] under
//! that bone, on a dedicated layer that collides with nothing. The Animator moves
//! the bone, the bone carries the hitbox, and a shot reports which bone it struck
//! ([`Scene::hit_bone`]).
//!
//! **Why a child of the bone, not the bone itself.** Bones are rebuilt from the
//! model on every load and never saved (#453); a child of a bone is saved as an
//! attachment and re-bound to its bone by name. So a hitbox is an ordinary,
//! hand-tweakable, saved entity, and a model re-export keeps it on the right bone.
//!
//! **The layer convention.** Movement collides through the character's capsule;
//! shots test the hitboxes. The hitbox layer is created on first use with every
//! collision-matrix cell off, so hitboxes never push, block or trigger anything,
//! and a cast's layer mask picks which of the two it tests.

mod fit;

pub use fit::{fit_hitboxes, HitboxFit, HitboxOptions};

use glam::{Quat, Vec3};

use crate::components::{ColliderComponent, PhysicsMaterial, TransformComponent};
use crate::scene::layers::LAYER_COUNT;
use crate::scene::Scene;

/// The name of the child entity a generated hitbox lives on, under its bone.
pub const HITBOX_NAME: &str = "Hitbox";

impl Scene {
    /// Generate (or regenerate) `owner`'s per-bone hitboxes and return them as
    /// `(bone name, hitbox entity)` pairs, ascending by joint slot. A bone that
    /// already has a `Hitbox` child gets it refitted in place; one that no longer
    /// qualifies loses it ([`Scene::destroy`]: queued in Play, so a script under it
    /// is torn down, #1298). `Err` when `owner` has no skinned mesh, or no layer is
    /// free for the hitbox layer.
    pub fn generate_hitboxes(
        &mut self,
        owner: u32,
        opts: &HitboxOptions,
    ) -> Result<Vec<(String, u32)>, String> {
        self.sync_skeleton(owner);
        let (fits, names, bones) = {
            let mesh = self
                .world
                .mesh(owner)
                .ok_or_else(|| format!("entity {owner} has no mesh"))?;
            let skin = mesh
                .skin
                .as_ref()
                .ok_or_else(|| format!("entity {owner}'s mesh is not skinned"))?;
            let names: Vec<String> = (0..skin.local_bind.len()).map(|s| skin.name(s)).collect();
            let fits = fit_hitboxes(&mesh.vertices, skin, opts);
            (fits, names, mesh.skeleton.bones.clone())
        };
        let layer = self.hitbox_layer(&opts.layer)?;
        let mut made = Vec::with_capacity(fits.len());
        for fit in &fits {
            // A destroyed bone's slot is missing (#1273): nothing to fit a hitbox to.
            let Some(bone) = bones.get(fit.slot).copied().flatten() else {
                continue;
            };
            made.push((names[fit.slot].clone(), self.place_hitbox(bone, fit, layer)));
        }
        for (slot, bone) in bones.iter().enumerate() {
            let Some(bone) = *bone else { continue };
            if !fits.iter().any(|f| f.slot == slot) {
                if let Some(stale) = self.hitbox_of(bone) {
                    self.destroy(stale);
                }
            }
        }
        Ok(made)
    }

    /// The generated hitbox under `bone`: its child named [`HITBOX_NAME`] that
    /// carries a collider. One queued for destruction, alone or with its bone or
    /// a bone above it, is already gone to every hitbox lookup (#1298, #1304), so
    /// a ragdoll or a regeneration in the same tick skips it.
    pub fn hitbox_of(&self, bone: u32) -> Option<u32> {
        self.world
            .children(bone)
            .into_iter()
            .find(|&c| self.is_generated_hitbox(c) && !self.is_doomed(c))
    }

    /// Whether `id` is a hitbox [`Scene::generate_hitboxes`] made: named
    /// [`HITBOX_NAME`] and carrying a collider.
    pub(in crate::scene::skeleton) fn is_generated_hitbox(&self, id: u32) -> bool {
        self.world.has_collider(id) && self.world.name(id).is_some_and(|n| *n == HITBOX_NAME)
    }

    /// Put `fit` on `bone`'s hitbox child, spawning it when absent.
    fn place_hitbox(&mut self, bone: u32, fit: &HitboxFit, layer: u8) -> u32 {
        let id = match self.hitbox_of(bone) {
            Some(id) => id,
            None => {
                let id = self.add_entity(HITBOX_NAME.to_string());
                let _ = self.set_parent(id, Some(bone));
                id
            }
        };
        if let Some(mut t) = self.world.transform_mut(id) {
            *t = TransformComponent {
                position: fit.center,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            };
        }
        self.world.set_layer(id, layer);
        self.world.set_collider(
            id,
            Some(ColliderComponent {
                active: true,
                shape: fit.shape.clone(),
                is_trigger: false,
                material: PhysicsMaterial::default(),
                shared_mesh: None,
                aabb_min: Vec3::ZERO,
                aabb_max: Vec3::ZERO,
            }),
        );
        id
    }

    /// The layer named `name`, or the first unnamed one claimed for it with every
    /// collision-matrix cell off — a hitbox only answers queries. An existing layer
    /// keeps whatever matrix the project gave it.
    fn hitbox_layer(&mut self, name: &str) -> Result<u8, String> {
        if let Some(layer) = self.layers.index_of(name) {
            return Ok(layer);
        }
        let free = self
            .layers
            .iter()
            .find(|&(i, n)| i > 0 && n.is_empty())
            .map(|(i, _)| i)
            .ok_or_else(|| format!("no free layer left for '{name}'"))?;
        self.layers.set_name(free, name);
        for other in 0..LAYER_COUNT as u8 {
            self.collision_matrix.set_collision(free, other, false);
        }
        Ok(free)
    }

    /// The bone a hit on `id` struck: `id` itself when it is a bone, else its
    /// nearest bone ancestor (the bone a hitbox hangs from); `None` off-skeleton.
    pub fn hit_bone(&self, id: u32) -> Option<u32> {
        self.world.self_and_ancestors(id).find(|&c| self.is_bone(c))
    }

    /// The top of `id`'s hierarchy (Unity's `Transform.root`): `id` itself when it
    /// has no parent. For a hitbox, the character it belongs to.
    pub fn root_of(&self, id: u32) -> u32 {
        self.world.ancestors(id).last().unwrap_or(id)
    }
}

#[cfg(test)]
mod persist_tests;
#[cfg(test)]
mod play_tests;
#[cfg(test)]
mod rig;
#[cfg(test)]
mod tests;
