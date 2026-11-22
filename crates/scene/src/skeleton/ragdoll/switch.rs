use glam::Vec3;

use super::RAGDOLL_LAYER;
use crate::layers::LAYER_COUNT;
use crate::skeleton::HitboxOptions;
use crate::Scene;

impl Scene {
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
        self.ragdolls.forget(&bones);
        for &bone in &bones {
            let velocity = on.then(|| velocity(bone).unwrap_or_default());
            self.switch_body(bone, velocity, false);
            if let (Some(from), Some(to)) = (from, to) {
                self.move_part_layers(bone, from, to);
            }
        }
        Ok(bones.len())
    }

    pub(super) fn switch_body(&mut self, bone: u32, velocity: Option<(Vec3, Vec3)>, track: bool) {
        let (v, w) = velocity.unwrap_or_default();
        if let Some(mut rb) = self.world.rigidbody_mut(bone) {
            rb.is_kinematic = velocity.is_none();
            rb.velocity = v;
            rb.angular_velocity = w;
        }
        let driven = self
            .world
            .joint(bone)
            .is_some_and(|j| j.drive_strength > 0.0);
        let needed = track || driven || self.ragdolls.has_weight(bone);
        match velocity {
            Some(_) if !needed => self.ragdolls.untrack(bone),
            Some(_) => {
                let pose = self.world.transform(bone).map(|t| (t.position, t.rotation));
                self.ragdolls.track(bone, pose.unwrap_or_default());
            }
            None => self.ragdolls.untrack(bone),
        }
    }

    pub fn ragdoll_enabled(&self, id: u32) -> bool {
        let Some(layer) = self.layers.index_of(RAGDOLL_LAYER) else {
            return false;
        };
        self.ragdoll_bones(id).into_iter().any(|b| {
            self.is_simulated_body(b) && self.body_parts(b).any(|p| self.world.layer(p) == layer)
        })
    }

    fn body_parts(&self, bone: u32) -> impl Iterator<Item = u32> + '_ {
        let children = self
            .world
            .children(bone)
            .into_iter()
            .filter(|&c| !self.is_bone(c));
        std::iter::once(bone)
            .chain(children)
            .filter(|&p| self.world.has_collider(p))
    }

    pub(super) fn move_part_layers(&mut self, bone: u32, from: u8, to: u8) {
        let parts: Vec<u32> = self
            .body_parts(bone)
            .filter(|&p| self.world.layer(p) == from)
            .collect();
        for p in parts {
            self.world.set_layer(p, to);
        }
    }

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
