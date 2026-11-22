use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;

use super::SightRays;
use crate::{ray, stand};
use rusty_physics::PhysicsWorld;
use rusty_scene::{layer_in_mask, Scene};

pub(super) struct Target {
    pub(super) id: u32,
    layer: u8,
    aim: Vec3,
    position: Vec3,
}

/// A pair that needs a ray, and its answer at the last sight step.
struct Pair {
    perceiver: usize,
    target: usize,
    eye: Vec3,
    /// The perceiver's own body, which never blocks its sight: its outermost
    /// ancestor-or-self that is a moving body, a target or not, else itself.
    body: u32,
    mask: u32,
    was_seen: bool,
}

pub(super) fn look(
    scene: &Scene,
    physics: Option<&PhysicsWorld>,
    perceivers: &[u32],
    rays: &mut SightRays,
) -> Vec<BTreeMap<u32, Vec3>> {
    let w = &scene.world;
    let mask = perceivers
        .iter()
        .filter_map(|&p| w.ai_perception(p).map(|c| c.sight_mask))
        .fold(0, |a, m| a | m);
    if mask == 0 {
        return vec![BTreeMap::new(); perceivers.len()];
    }
    let targets = targets(scene, mask);
    look_at(scene, physics, perceivers, rays, &targets, |p| {
        body_of(scene, p)
    })
}

pub(super) fn look_at(
    scene: &Scene,
    physics: Option<&PhysicsWorld>,
    perceivers: &[u32],
    rays: &mut SightRays,
    targets: &[Target],
    body: impl Fn(u32) -> u32,
) -> Vec<BTreeMap<u32, Vec3>> {
    let pairs = pairs(scene, perceivers, targets, body);
    let traced = schedule(&pairs, perceivers, targets, rays);
    let mut seen = vec![BTreeMap::new(); perceivers.len()];
    for (pair, traced) in pairs.iter().zip(traced) {
        let target = &targets[pair.target];
        let visible = if traced {
            let ignore = [Some(pair.body), Some(target.id)];
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

fn targets(scene: &Scene, mask: u32) -> Vec<Target> {
    let w = &scene.world;
    let all = w.moving_bodies();
    let moving = |a: u32| all.binary_search_by_key(&a, |&(id, _)| id).is_ok();
    let keep = |&&(id, layer): &&(u32, u8)| {
        layer_in_mask(layer, mask) && w.is_active_in_hierarchy(id) && !w.ancestors(id).any(moving)
    };
    let ids: Vec<u32> = all.iter().filter(keep).map(|&(id, _)| id).collect();
    ids.into_iter().map(|id| Target::of(scene, id)).collect()
}

impl Target {
    /// `id` as a target: its layer, where a ray aims at it and where it stands.
    pub(super) fn of(scene: &Scene, id: u32) -> Self {
        Self {
            id,
            layer: scene.world.layer(id),
            aim: aim(scene, id),
            position: stand::feet(scene, id),
        }
    }
}

fn body_of(scene: &Scene, p: u32) -> u32 {
    let w = &scene.world;
    w.self_and_ancestors(p)
        .filter(|&a| w.is_moving_body(a))
        .last()
        .unwrap_or(p)
}

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
fn pairs(
    scene: &Scene,
    perceivers: &[u32],
    targets: &[Target],
    body: impl Fn(u32) -> u32,
) -> Vec<Pair> {
    let w = &scene.world;
    let mut out = Vec::new();
    for (pi, &p) in perceivers.iter().enumerate() {
        let Some(c) = w.ai_perception(p) else {
            continue;
        };
        let eye = w.world_position(p) + Vec3::Y * c.eye_height;
        let forward = w.world_rotation(p) * Vec3::Z;
        let half = c.sight_angle.to_radians();
        let lineage: BTreeSet<u32> = w.self_and_ancestors(p).collect();
        let body = body(p);
        for (ti, t) in targets.iter().enumerate() {
            let own = lineage.contains(&t.id) || w.self_and_ancestors(t.id).any(|a| a == p);
            if own || !layer_in_mask(t.layer, c.sight_mask) {
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
                body,
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
