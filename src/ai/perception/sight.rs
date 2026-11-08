//! The sight sense (#1900): range, view cone and one budgeted ray per pair.
//!
//! For each perceiver, every sight target (see the module docs) within
//! `sight_range` (or `lose_sight_range` when it is already seen) and inside the
//! cone around the entity's forward (+Z) needs a ray from the eye to the
//! target's bounds centre. Those pairs, in (perceiver, target) id order, share
//! [`SightRays::per_tick`] rays a tick, round-robin from the pair after the last
//! one traced; an untraced pair keeps last tick's answer. Out of range or out of
//! the cone needs no ray: the target is not seen.

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;

use super::SightRays;
use crate::ai::ray;
use crate::physics::PhysicsWorld;
use crate::scene::{layer_in_mask, Scene};

/// A sight target: where a ray aims at it and where it stands.
struct Target {
    id: u32,
    layer: u8,
    aim: Vec3,
    position: Vec3,
}

/// A pair that needs a ray, and its answer at the last sight step.
struct Pair {
    perceiver: usize,
    target: usize,
    eye: Vec3,
    mask: u32,
    was_seen: bool,
}

/// Every perceiver's visible targets this tick (target → where it stands), in
/// the order of `perceivers`.
pub(super) fn look(
    scene: &Scene,
    physics: Option<&PhysicsWorld>,
    perceivers: &[u32],
    rays: &mut SightRays,
) -> Vec<BTreeMap<u32, Vec3>> {
    let targets = targets(scene);
    let pairs = pairs(scene, perceivers, &targets);
    let traced = schedule(&pairs, perceivers, &targets, rays);
    let mut seen = vec![BTreeMap::new(); perceivers.len()];
    for (pair, traced) in pairs.iter().zip(traced) {
        let target = &targets[pair.target];
        let visible = if traced {
            let ignore = [Some(perceivers[pair.perceiver]), Some(target.id)];
            ray::clear(
                scene,
                physics,
                (pair.eye, target.aim),
                ignore,
                Some(pair.mask),
            )
        } else {
            pair.was_seen
        };
        if visible {
            seen[pair.perceiver].insert(target.id, target.position);
        }
    }
    seen
}

/// Every entity sight may target, in id order (who qualifies: the module docs).
fn targets(scene: &Scene) -> Vec<Target> {
    let w = &scene.world;
    let mut ids: Vec<u32> = w.ids_with_collider();
    ids.extend(w.ids_with_character_controller());
    ids.sort_unstable();
    ids.dedup();
    ids.retain(|&id| w.is_active_in_hierarchy(id) && !w.is_static(id));
    let bodies: BTreeSet<u32> = ids.iter().copied().collect();
    ids.into_iter()
        .filter(|&id| {
            !w.self_and_ancestors(id)
                .skip(1)
                .any(|a| bodies.contains(&a))
        })
        .map(|id| Target {
            id,
            layer: w.layer(id),
            aim: aim(scene, id),
            position: w.world_position(id),
        })
        .collect()
}

/// Where a sight ray aims at `id`: its collider's world bounds centre, else its
/// character capsule's centre, else its origin.
fn aim(scene: &Scene, id: u32) -> Vec3 {
    let w = &scene.world;
    if let Some((lo, hi)) = w.collider_bounds(id) {
        return (lo + hi) * 0.5;
    }
    match w.character_controller(id) {
        Some(c) => w.local_to_world(id).transform_point3(c.center),
        None => w.world_position(id),
    }
}

/// The pairs inside range and cone, in (perceiver, target) order.
fn pairs(scene: &Scene, perceivers: &[u32], targets: &[Target]) -> Vec<Pair> {
    let w = &scene.world;
    let mut out = Vec::new();
    for (pi, &p) in perceivers.iter().enumerate() {
        let Some(c) = w.ai_perception(p) else {
            continue;
        };
        let eye = w.world_position(p) + Vec3::Y * c.eye_height;
        let forward = w.world_rotation(p) * Vec3::Z;
        let half = c.sight_angle.to_radians();
        for (ti, t) in targets.iter().enumerate() {
            if !layer_in_mask(t.layer, c.sight_mask) || w.self_and_ancestors(t.id).any(|a| a == p) {
                continue;
            }
            let was_seen = c.memory.in_sight.contains(&t.id);
            let range = if was_seen {
                c.lose_sight_range
            } else {
                c.sight_range
            };
            let to = t.aim - eye;
            if to.length() > range || !in_cone(forward, to, half) {
                continue;
            }
            out.push(Pair {
                perceiver: pi,
                target: ti,
                eye,
                mask: c.blocking_mask,
                was_seen,
            });
        }
    }
    out
}

/// Whether `to` lies within `half` radians of `forward` (a point at the eye does).
fn in_cone(forward: Vec3, to: Vec3, half: f32) -> bool {
    if half >= std::f32::consts::PI {
        return true;
    }
    match (forward.try_normalize(), to.try_normalize()) {
        (Some(f), Some(t)) => f.dot(t).clamp(-1.0, 1.0).acos() <= half,
        _ => true,
    }
}

/// Which pairs get a ray this tick: up to the budget, round-robin from the pair
/// after the last one traced. Moves the round on.
fn schedule(
    pairs: &[Pair],
    perceivers: &[u32],
    targets: &[Target],
    rays: &mut SightRays,
) -> Vec<bool> {
    let key = |p: &Pair| (perceivers[p.perceiver], targets[p.target].id);
    let n = pairs.len();
    let mut traced = vec![false; n];
    let budget = rays.per_tick.min(n);
    if budget == 0 {
        return traced;
    }
    let start = rays
        .last
        .map_or(0, |last| pairs.partition_point(|p| key(p) <= last) % n);
    for i in 0..budget {
        traced[(start + i) % n] = true;
    }
    rays.last = Some(key(&pairs[(start + budget - 1) % n]));
    traced
}
