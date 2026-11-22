mod fit;

pub use fit::{fit_hitboxes, HitboxFit, HitboxOptions};

use glam::{Quat, Vec3};

use crate::layers::LAYER_COUNT;
use crate::Scene;
use rusty_components::{ColliderComponent, TransformComponent};

pub const HITBOX_NAME: &str = "Hitbox";

impl Scene {
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
            let Some(bone) = bones.get(fit.slot).copied().flatten() else {
                continue;
            };
            made.push((names[fit.slot].clone(), self.place_hitbox(bone, fit, layer)));
        }
        for (slot, bone) in bones.iter().enumerate() {
            let Some(bone) = *bone else { continue };
            if !fits.iter().any(|f| f.slot == slot) {
                if let Some(stale) = self.hitbox_of(bone) {
                    // Queued in Play, so a script under it is torn down first.
                    self.destroy(stale);
                }
            }
        }
        Ok(made)
    }

    pub fn hitbox_of(&self, bone: u32) -> Option<u32> {
        self.world
            .children(bone)
            .into_iter()
            // One queued for destruction is already gone to every lookup.
            .find(|&c| self.is_generated_hitbox(c) && !self.is_doomed(c))
    }

    pub fn is_generated_hitbox(&self, id: u32) -> bool {
        self.world.has_collider(id) && self.world.name(id).is_some_and(|n| *n == HITBOX_NAME)
    }

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
        let (material, material_asset) = self
            .world
            .collider(id)
            .map(|c| (c.material, c.material_asset.clone()))
            .unwrap_or_default();
        self.world.set_collider(
            id,
            Some(ColliderComponent {
                active: true,
                shape: fit.shape.clone(),
                is_trigger: false,
                material,
                material_asset,
                shared_mesh: None,
                collision_mesh: None,
                aabb_min: Vec3::ZERO,
                aabb_max: Vec3::ZERO,
            }),
        );
        id
    }

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

    pub fn hit_bone(&self, id: u32) -> Option<u32> {
        self.world.self_and_ancestors(id).find(|&c| self.is_bone(c))
    }

    pub fn root_of(&self, id: u32) -> u32 {
        self.world.ancestors(id).last().unwrap_or(id)
    }
}

#[cfg(test)]
mod armature_tests;
#[cfg(test)]
mod play_tests;
#[cfg(test)]
mod rig;
#[cfg(test)]
mod tests;
