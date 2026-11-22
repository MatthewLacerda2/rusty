use glam::Vec3;

use crate::world::PhysicsWorld;
use rusty_scene::Scene;

impl PhysicsWorld {
    /// Runs after the Animator, which rewrites the bones it keys and drags those below:
    /// puts each bone a dynamic body carries back on it, parents first. Starts from the
    /// dynamic bodies, so a scene without a ragdoll reads no skeleton.
    pub fn pose_bones(&self, scene: &mut Scene) {
        let dynamic = self.plan.keys().filter(|id| {
            self.id_to_body
                .get(id)
                .and_then(|&h| self.bodies.get(h))
                .is_some_and(|b| b.is_dynamic())
        });
        let mut owners: Vec<(usize, u32)> = dynamic
            .filter(|&&id| scene.is_bone(id))
            .map(|&id| (depth(scene, id), id))
            .collect();
        owners.sort_unstable();
        for (_, id) in owners {
            self.write_back(scene, id);
            blend_with_animation(scene, id);
        }
    }

    /// An animated bone's body is kinematic: this is the motion rapier derived from the
    /// Animator's move, so a ragdoll switched on keeps moving as the animation was.
    pub fn body_velocity(&self, id: u32) -> Option<(Vec3, Vec3)> {
        let body = self.bodies.get(*self.id_to_body.get(&id)?)?;
        Some((body.linvel(), body.angvel()))
    }

    /// `impulse` in N·s. rapier keeps a kinematic body's mass too, so this holds on the
    /// tick a ragdoll is switched on.
    pub fn impulse_response(&self, id: u32, impulse: Vec3, point: Vec3) -> Option<(Vec3, Vec3)> {
        let body = self.bodies.get(*self.id_to_body.get(&id)?)?;
        let props = &body.mass_properties().local_mprops;
        if props.inv_mass <= 0.0 {
            return None;
        }
        let com = body.position() * props.local_com;
        let j = impulse;
        let torque = (point - com).cross(j);
        let dw = props.world_inv_inertia(body.rotation()) * torque;
        Some((impulse * props.inv_mass, dw))
    }
}

fn blend_with_animation(scene: &mut Scene, id: u32) {
    let w = scene.ragdolls.weight(id);
    let Some((pos, rot)) = scene.ragdolls.animated(id).filter(|_| w < 1.0) else {
        return;
    };
    if let Some(mut t) = scene.world.transform_mut(id) {
        scene.ragdolls.note_physics(id, (t.position, t.rotation));
        if w <= 0.0 {
            (t.position, t.rotation) = (pos, rot);
        } else {
            t.position = pos.lerp(t.position, w);
            t.rotation = rot.slerp(t.rotation, w);
        }
    }
}

fn depth(scene: &Scene, id: u32) -> usize {
    scene.world.ancestors(id).count()
}

#[cfg(test)]
#[path = "ragdoll_cost_tests.rs"]
mod cost_tests;
