use glam::{Mat4, Quat, Vec3};

use crate::animation;
use crate::resources::Resources;
use crate::world::World;
use apply::apply_root_motion;
use rusty_asset::animation_graph::AnimationGraph;
use rusty_asset::mesh_data::SkinData;
use rusty_components::{root_chain, AnimatorComponent, MeshComponent, RootMotion, StateCallback};
use rusty_scene::Scene;
use std::collections::BTreeSet;

mod apply;

pub(crate) fn animate(world: &mut World, res: &mut Resources) {
    let dt = res.frame_dt;
    let (ids, hears) = listening(world, res);
    let mut s = world.scene.borrow_mut();
    for id in ids {
        if !s.world.is_active_in_hierarchy(id) {
            settle(&mut s, id);
            continue;
        }
        let path = s.world.animator(id).and_then(|a| a.graph.clone());
        let graph = path.and_then(|p| res.animation_graphs.get_or_load(&p, &res.console));
        let frame = (s.world.world_rotation(id), s.world.lossy_scale(id));
        let stepped = s.world.with_animator_mut_and_mesh(id, |anim, mesh| {
            let stepped = animation::step(anim, mesh, graph, dt);
            let states = states_of(anim, mesh, graph, hears.contains(&id));
            let skin = mesh.and_then(|m| m.skin.as_ref());
            let model = model_in_entity(skin);
            let motion = stepped
                .root
                .map_or(RootMotion::ZERO, |m| to_world(m, frame, model));
            let drives = anim.drives_root_motion(skin);
            measured(anim, motion, drives);
            let unknown = skin.and_then(|skin| unknown_bone(anim, skin));
            let writes = mesh.map(|m| animation::bone_writes(anim, m, graph));
            let events = (states, stepped.events);
            (events, writes, drives.then_some((motion, model)), unknown)
        });
        let Some(((states, crossed), writes, applied, unknown)) = stepped else {
            continue;
        };
        if let Some(bone) = unknown {
            let name = s.world.name(id).map(|n| n.clone()).unwrap_or_default();
            res.console.borrow_mut().warn(format!(
                "Animator on '{name}' ({id}): root bone '{bone}' is not in its skeleton; root motion moves nothing"
            ));
        }
        let queue = &mut res.animation_events;
        queue.states.extend(states.into_iter().map(|c| (id, c)));
        let events = crossed.into_iter().map(|e| (id, e.name, e.montage));
        queue.events.extend(events);
        write_pose(&mut s, writes.unwrap_or_default());
        if let Some((motion, model)) = applied {
            let hits = apply_root_motion(&mut s, res, id, motion, model, dt);
            let touched = (!hits.is_empty()).then_some((id, hits));
            res.controller_hits.extend(touched);
        }
    }
}

fn write_pose(s: &mut Scene, writes: Vec<(u32, rusty_components::TransformComponent)>) {
    for (bone, local) in writes {
        // A partial ragdoll's bone: its animated pose is its drive's target and blend side.
        if !s.ragdolls.is_idle() {
            s.ragdolls
                .note_animated(bone, (local.position, local.rotation));
        }
        if let Some(mut t) = s.world.transform_mut(bone) {
            *t = local;
        }
    }
}

fn listening(world: &World, res: &Resources) -> (Vec<u32>, BTreeSet<u32>) {
    let ids = world.scene.borrow().world.ids_with_animator();
    let hears = ids.iter().copied();
    let hears = hears.filter(|&id| res.script_manager.hears_states(id));
    let hears = hears.collect();
    (ids, hears)
}

fn measured(anim: &mut AnimatorComponent, motion: RootMotion, drives: bool) {
    let state = &mut anim.root_motion;
    state.last = motion;
    state.moved = Vec3::ZERO;
    if !drives {
        state.carried = Vec3::ZERO;
    }
}

/// An inactive animator measures nothing: `GetRootMotion` reports no step, and its states
/// are forgotten, so a reactivation enters them afresh, as Unity's does.
fn settle(s: &mut Scene, id: u32) {
    let idle = |a: &AnimatorComponent| {
        a.root_motion.last.is_zero() && a.root_motion.moved == Vec3::ZERO && a.states.is_empty()
    };
    if s.world.animator(id).is_some_and(|a| idle(&a)) {
        return;
    }
    if let Some(mut a) = s.world.animator_mut(id) {
        a.root_motion.last = RootMotion::ZERO;
        a.root_motion.moved = Vec3::ZERO;
        forget_states(&mut a);
    }
}

fn states_of(
    anim: &mut AnimatorComponent,
    mesh: Option<&MeshComponent>,
    graph: Option<&AnimationGraph>,
    hear: bool,
) -> Vec<StateCallback> {
    match hear {
        true => animation::state_callbacks(anim, mesh.map_or(&[][..], |m| &m.clips[..]), graph),
        false => forget_states(anim), // entered afresh once a script listens
    }
}

fn forget_states(anim: &mut AnimatorComponent) -> Vec<StateCallback> {
    anim.states.clear();
    Vec::new()
}

fn unknown_bone(
    anim: &mut AnimatorComponent,
    skin: &rusty_asset::mesh_data::SkinData,
) -> Option<String> {
    let missing = anim.apply_root_motion && root_chain(skin, &anim.root_bone).is_none();
    let state = &mut anim.root_motion;
    if !missing || state.warned_bone.as_deref() == Some(anim.root_bone.as_str()) {
        return None;
    }
    state.warned_bone = Some(anim.root_bone.clone());
    Some(anim.root_bone.clone())
}

fn model_in_entity(skin: Option<&SkinData>) -> Option<Mat4> {
    skin?.mesh_to_model_axes().map(|m| m.inverse())
}

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
