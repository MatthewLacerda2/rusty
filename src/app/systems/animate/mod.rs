//! src/app/systems/animate/ — the `animate` system (#80, #453) and root
//! motion's application (#1745).
//!
//! Each active animator's playheads advance, the sampled pose is written onto
//! its bone GameObjects, and the step's root motion is stored world-space on
//! the component (`Animator.GetRootMotion`; zeroed while the entity is
//! inactive). When root motion drives the entity
//! (`AnimatorComponent::drives_root_motion`: *Apply Root Motion* on, the
//! animator running, its root bone in the skeleton), that motion then moves the
//! animator's own entity, Unity's order of preference:
//! - a **CharacterController** takes it through its `Move`, so walls and steps
//!   stop and lift it as they would a script's move; a step shorter than its
//!   `min_move_distance` is carried into the next rather than dropped;
//! - else an active **NavMeshAgent** slides it along the navmesh, the way the
//!   agent's own steering would (the agent stops moving itself: `tick_nav`
//!   leaves its position to this). On an off-mesh link the link decides: an
//!   auto-traversed link carries the agent and the clip's travel is dropped;
//!   a link a script traverses takes the clip's travel on the Transform;
//! - else a **dynamic Rigidbody** takes it as velocity (Unity's
//!   `OnAnimatorMove` pattern): its horizontal velocity becomes `delta / dt`,
//!   its vertical one is kept so gravity still pulls, and the turn becomes its
//!   angular velocity about +Y, since the solver owns a dynamic body's pose;
//! - else the **Transform** moves by it outright (a kinematic body follows).
//!
//! The step is measured in the model's Y-up space (`animation::root_motion`)
//! and carried into the world through the skinned entity and whatever sits
//! between its mesh and the model in the file ([`SkinData::mesh_to_model`]: a
//! Blender export's Z-up, centimetre `Armature`, #1822), so the turn is about
//! the model's up axis as the entity shows it. An unknown root bone warns once
//! in the console and moves nothing.

use glam::{Mat4, Quat, Vec3};

use crate::app::animation;
use crate::app::resources::Resources;
use crate::app::world::World;
use crate::asset::mesh_data::SkinData;
use crate::components::{root_chain, AnimatorComponent, RootMotion};
use crate::scene::Scene;
use apply::apply_root_motion;

mod apply;

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
            settle(&mut s, id);
            continue;
        }
        let path = s.world.animator(id).and_then(|a| a.graph.clone());
        let graph = path.and_then(|p| res.animation_graphs.get_or_load(&p, &res.console));
        let frame = (s.world.world_rotation(id), s.world.lossy_scale(id));
        // The animator + mesh split borrow goes through the facade's sanctioned
        // helper (#344) — the animate system's re-pose step.
        let stepped = s.world.with_animator_mut_and_mesh(id, |anim, mesh| {
            let stepped = animation::step(anim, mesh, graph, dt);
            let skin = mesh.and_then(|m| m.skin.as_ref());
            let model = model_in_entity(skin);
            let motion = stepped
                .root
                .map_or(RootMotion::ZERO, |m| to_world(m, frame, model));
            let drives = anim.drives_root_motion(skin);
            measured(anim, motion, drives);
            let unknown = skin.and_then(|skin| unknown_bone(anim, skin));
            let writes = mesh.map(|m| animation::bone_writes(anim, m, graph));
            (
                stepped.events,
                writes,
                drives.then_some((motion, model)),
                unknown,
            )
        });
        let Some((crossed, writes, applied, unknown)) = stepped else {
            continue;
        };
        if let Some(bone) = unknown {
            let name = s.world.name(id).map(|n| n.clone()).unwrap_or_default();
            res.console.borrow_mut().warn(format!(
                "Animator on '{name}' ({id}): root bone '{bone}' is not in its skeleton; root motion moves nothing"
            ));
        }
        let events = crossed.into_iter().map(|e| (id, e.name, e.montage));
        res.animation_events.extend(events);
        for (bone, local) in writes.unwrap_or_default() {
            if let Some(mut t) = s.world.transform_mut(bone) {
                *t = local;
            }
        }
        if let Some((motion, model)) = applied {
            let hits = apply_root_motion(&mut s, res, id, motion, model, dt);
            res.controller_hits.push((id, hits)); // empty unless a controller hit something
        }
    }
}

/// Keep this step's world-space root `motion` on `anim`, before anything
/// moves; travel carried for a CharacterController lapses when root motion
/// no longer `drives` the entity.
fn measured(anim: &mut AnimatorComponent, motion: RootMotion, drives: bool) {
    let state = &mut anim.root_motion;
    state.last = motion;
    state.moved = Vec3::ZERO;
    if !drives {
        state.carried = Vec3::ZERO;
    }
}

/// An inactive entity's animator measures nothing: its last root motion is
/// zeroed, so `GetRootMotion` doesn't report a step it no longer takes.
fn settle(s: &mut Scene, id: u32) {
    let idle =
        |a: &AnimatorComponent| a.root_motion.last.is_zero() && a.root_motion.moved == Vec3::ZERO;
    if s.world.animator(id).is_some_and(|a| idle(&a)) {
        return; // compared first: a borrow stamps the animator as edited
    }
    if let Some(mut a) = s.world.animator_mut(id) {
        a.root_motion.last = RootMotion::ZERO;
        a.root_motion.moved = Vec3::ZERO;
    }
}

/// The root bone `anim` names when root motion is on and `skin` lacks it,
/// the first time: it is remembered so the console hears of it once.
fn unknown_bone(
    anim: &mut AnimatorComponent,
    skin: &crate::asset::mesh_data::SkinData,
) -> Option<String> {
    let missing = anim.apply_root_motion && root_chain(skin, &anim.root_bone).is_none();
    let state = &mut anim.root_motion;
    if !missing || state.warned_bone.as_deref() == Some(anim.root_bone.as_str()) {
        return None;
    }
    state.warned_bone = Some(anim.root_bone.clone());
    Some(anim.root_bone.clone())
}

/// Model space in the skinned entity's own (mesh-local) space: `skin`'s
/// [`SkinData::mesh_to_model_axes`] inverted, `None` when their axes coincide
/// (a native Y-up rig, or no skin), so such a rig's motion is carried exactly
/// as before.
fn model_in_entity(skin: Option<&SkinData>) -> Option<Mat4> {
    skin?.mesh_to_model_axes().map(|m| m.inverse())
}

/// `motion`, measured in model space, in world space: its displacement taken
/// into the entity's space by `model` ([`model_in_entity`]), then under the
/// entity's world rotation and scale. The yaw stays a turn about the model's
/// up axis. A non-finite one (a clip keyed with a NaN) moves nothing.
fn to_world(
    motion: RootMotion,
    (rotation, scale): (Quat, Vec3),
    model: Option<Mat4>,
) -> RootMotion {
    let local = model.map_or(motion.position, |m| m.transform_vector3(motion.position));
    let world = RootMotion {
        position: rotation * (scale * local),
        yaw: motion.yaw,
    };
    match world.position.is_finite() && world.yaw.is_finite() {
        true => world,
        false => RootMotion::ZERO,
    }
}

#[cfg(test)]
mod agent_tests;
#[cfg(test)]
mod armature_tests;
#[cfg(test)]
mod body_tests;
#[cfg(test)]
mod hits_tests;
#[cfg(test)]
mod tests;
