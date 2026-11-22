use glam::{Mat4, Quat, Vec3};

use crate::resources::Resources;
use rusty_components::RootMotion;
use rusty_physics::ControllerHit;
use rusty_scene::Scene;

/// What takes entity `id`'s root motion, in Unity's order of preference.
enum Mover {
    Character(f32),
    Agent,
    AutoLink,
    Body,
    Transform,
}

fn mover(s: &Scene, id: u32) -> Mover {
    if let Some(cc) = s.world.character_controller(id) {
        return Mover::Character(cc.min_move_distance);
    }
    if let Some(agent) = s.world.nav_agent(id).filter(|a| a.active) {
        return match (
            agent.off_mesh_link.is_some(),
            agent.auto_traverse_off_mesh_link,
        ) {
            (false, _) => Mover::Agent,
            (true, true) => Mover::AutoLink,
            (true, false) => Mover::Transform,
        };
    }
    let dynamic = s.world.rigidbody(id).is_some_and(|rb| !rb.is_kinematic);
    match dynamic && !s.world.is_static(id) {
        true => Mover::Body,
        false => Mover::Transform,
    }
}

pub(super) fn apply_root_motion(
    s: &mut Scene,
    res: &Resources,
    id: u32,
    motion: RootMotion,
    model: Option<Mat4>,
    dt: f32,
) -> Vec<ControllerHit> {
    let carried = s
        .world
        .animator(id)
        .map_or(Vec3::ZERO, |a| a.root_motion.carried);
    let before = s.world.world_position(id);
    let (mut moved, mut carry) = (None, Vec3::ZERO);
    let mut hits = Vec::new();
    let turn = match mover(s, id) {
        Mover::Character(min) => {
            let step = carried + motion.position;
            if step.length() < min {
                carry = step;
            } else {
                let physics = res.physics.borrow();
                let m = rusty_physics::move_character(physics.as_ref(), s, id, step);
                hits = m.map(|m| m.hits).unwrap_or_default();
            }
            true
        }
        Mover::Agent => {
            if motion.position != Vec3::ZERO {
                res.nav.borrow().move_agent(s, id, motion.position);
            }
            true
        }
        Mover::AutoLink => false,
        Mover::Body => {
            moved = Some(motion.position);
            drive_body(s, id, motion, dt);
            false
        }
        Mover::Transform => {
            if motion.position != Vec3::ZERO {
                s.world.set_world_position(id, before + motion.position);
            }
            true
        }
    };
    if turn && motion.yaw != 0.0 {
        let turn = Quat::from_rotation_y(motion.yaw);
        // lint: allow(world-space) the model's axes in mesh-local space, rig data, not the scene hierarchy
        let tilt = model.map(|m| m.to_scale_rotation_translation().1);
        let turn = tilt.map_or(turn, |t| t * turn * t.inverse());
        let facing = s.world.world_rotation(id) * turn;
        s.world.set_world_rotation(id, facing);
    }
    let moved = moved.unwrap_or_else(|| s.world.world_position(id) - before);
    record(s, id, moved, carry);
    hits
}

fn record(s: &mut Scene, id: u32, moved: Vec3, carry: Vec3) {
    let unchanged = s
        .world
        .animator(id)
        .is_some_and(|a| a.root_motion.moved == moved && a.root_motion.carried == carry);
    if !unchanged {
        if let Some(mut a) = s.world.animator_mut(id) {
            a.root_motion.moved = moved;
            a.root_motion.carried = carry;
        }
    }
}

fn drive_body(s: &mut Scene, id: u32, motion: RootMotion, dt: f32) {
    if dt <= 0.0 {
        return;
    }
    let (velocity, spin) = (motion.position / dt, motion.yaw / dt);
    let same = s.world.rigidbody(id).is_some_and(|rb| {
        rb.velocity.x == velocity.x && rb.velocity.z == velocity.z && rb.angular_velocity.y == spin
    });
    if same {
        return;
    }
    if let Some(mut rb) = s.world.rigidbody_mut(id) {
        rb.velocity.x = velocity.x;
        rb.velocity.z = velocity.z;
        rb.angular_velocity.y = spin;
    }
}
