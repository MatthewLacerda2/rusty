//! src/app/systems/animate.rs — the `animate` system (#80, #453) and root
//! motion's application (#1745).
//!
//! Each active animator's playheads advance, the sampled pose is written onto
//! its bone GameObjects, and the step's root motion is stored world-space on
//! the component (`Animator.GetRootMotion`). With *Apply Root Motion* on, that
//! motion then moves the animator's own entity, Unity's order of preference:
//! - a **CharacterController** takes it through its `Move`, so walls and steps
//!   stop and lift it as they would a script's move;
//! - else an active **NavMeshAgent** slides it along the navmesh, the way the
//!   agent's own steering would (the agent stops moving itself: `tick_nav`
//!   leaves its position to this);
//! - else the **Transform** moves by it outright.
//!
//! The turn is applied about the entity's own up axis.

use glam::{Quat, Vec3};

use crate::app::animation;
use crate::app::resources::Resources;
use crate::app::world::World;
use crate::components::RootMotion;
use crate::scene::Scene;

/// Advance every active entity's animator and write the sampled pose onto its
/// bone GameObjects (#80, #453). Blend-tree nodes and extra layers (#457) resolve
/// against the animator's graph asset, from the cache the evaluator just filled.
/// The sampler is a pure function of (skin, clips, graph, parameters, time), so
/// stepping at the fixed dt is deterministic. Writers after this one (`LateUpdate`
/// scripts, physics) may override a bone before the palette build.
pub(in crate::app) fn animate(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let mut s = world.scene.borrow_mut();
    for id in s.world.ids_with_animator() {
        if !s.world.is_active_in_hierarchy(id) {
            continue;
        }
        let path = s.world.animator(id).and_then(|a| a.graph.clone());
        let graph = path.and_then(|p| res.animation_graphs.get_or_load(&p, &res.console));
        let frame = (s.world.world_rotation(id), s.world.lossy_scale(id));
        // The animator + mesh split borrow goes through the facade's sanctioned
        // helper (#344) — the animate system's re-pose step.
        let stepped = s.world.with_animator_mut_and_mesh(id, |anim, mesh| {
            let stepped = animation::step(anim, mesh, graph, dt);
            let motion = stepped
                .root
                .map_or(RootMotion::ZERO, |m| to_world(m, frame));
            anim.root_motion = motion;
            let writes = mesh.map(|m| animation::bone_writes(anim, m, graph));
            (
                stepped.events,
                writes,
                anim.apply_root_motion.then_some(motion),
            )
        });
        let Some((crossed, writes, applied)) = stepped else {
            continue;
        };
        let events = crossed.into_iter().map(|e| (id, e.name));
        res.animation_events.extend(events);
        for (bone, local) in writes.unwrap_or_default() {
            if let Some(mut t) = s.world.transform_mut(bone) {
                *t = local;
            }
        }
        if let Some(motion) = applied.filter(|m| !m.is_zero()) {
            apply_root_motion(&mut s, res, id, motion);
        }
    }
}

/// `motion`, measured in the skinned entity's space, in world space: its
/// displacement under the entity's world rotation and scale. The yaw stays a
/// turn about the entity's own up axis.
/// A non-finite one (a clip keyed with a NaN) moves nothing.
fn to_world(motion: RootMotion, (rotation, scale): (Quat, Vec3)) -> RootMotion {
    let world = RootMotion {
        position: rotation * (scale * motion.position),
        yaw: motion.yaw,
    };
    match world.position.is_finite() && world.yaw.is_finite() {
        true => world,
        false => RootMotion::ZERO,
    }
}

/// Move entity `id` by the world-space root `motion`: through its
/// CharacterController, else its active NavMeshAgent, else its Transform.
fn apply_root_motion(s: &mut Scene, res: &Resources, id: u32, motion: RootMotion) {
    if s.world.character_controller(id).is_some() {
        let physics = res.physics.borrow();
        crate::physics::move_character(physics.as_ref(), s, id, motion.position);
    } else if s.world.nav_agent(id).is_some_and(|a| a.active) {
        res.nav.borrow().move_agent(s, id, motion.position);
    } else {
        let at = s.world.world_position(id);
        s.world.set_world_position(id, at + motion.position);
    }
    if motion.yaw != 0.0 {
        let facing = s.world.world_rotation(id) * Quat::from_rotation_y(motion.yaw);
        s.world.set_world_rotation(id, facing);
    }
}

#[cfg(test)]
#[path = "animate_tests.rs"]
mod animate_tests;
