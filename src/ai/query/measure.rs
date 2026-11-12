//! The raw value each test reads off an item (#1892). The pure-maths tests are
//! here; rays are `sight`'s and the navmesh's `walk`'s.

use glam::Vec3;

use super::spec::{Anchor, Measure};
use super::{sight, walk, QueryWorld};
use crate::scene::Scene;

/// Each item's raw value for `measure`, in `items`' order. `None` fails the
/// test for that item whatever its filter (an item the querier can't reach has
/// no path length).
pub fn raw(
    world: &QueryWorld,
    querier: Anchor,
    measure: &Measure,
    items: &[Vec3],
) -> Vec<Option<f32>> {
    match measure {
        Measure::Distance { to, flat } => each(items, |p| distance(to.position, p, *flat)),
        Measure::Dot { from, flat } => each(items, |p| dot(*from, p, *flat)),
        Measure::Trace {
            from,
            from_height,
            item_height,
            mask,
            visible,
        } => {
            let ignore = [from.entity, querier.entity];
            let eye = from.position + Vec3::Y * *from_height;
            each(items, |p| {
                let seen = sight::clear(world, eye, p + Vec3::Y * *item_height, ignore, *mask);
                answer(seen == *visible)
            })
        }
        Measure::View {
            margin,
            item_height,
            inside,
        } => each(items, |p| {
            let at = p + Vec3::Y * *item_height;
            answer(sight::in_view(world.view, world.screen, at, *margin) == *inside)
        }),
        Measure::PathExists | Measure::PathLength => {
            let lengths = walk::FloodAsk::new(world, querier, items)
                .map_or_else(Vec::new, |ask| ask.lengths(world.nav));
            match measure {
                Measure::PathExists => lengths
                    .into_iter()
                    .map(|l| Some(answer(l.is_some())))
                    .collect(),
                _ => lengths,
            }
        }
        Measure::Separation { radius } => {
            let others = others(world.scene, querier.entity);
            each(items, |p| nearest(&others, p).min(*radius))
        }
        Measure::NearPoints { points } => each(items, |p| nearest(points, p)),
    }
}

/// A yes/no test's raw value: 1 for the answer it asks for.
pub(super) fn answer(matches: bool) -> f32 {
    if matches {
        1.0
    } else {
        0.0
    }
}

fn each(items: &[Vec3], f: impl Fn(Vec3) -> f32) -> Vec<Option<f32>> {
    items.iter().map(|&p| Some(f(p))).collect()
}

fn distance(a: Vec3, b: Vec3, flat: bool) -> f32 {
    let d = b - a;
    if flat {
        d.with_y(0.0).length()
    } else {
        d.length()
    }
}

/// The cosine between `from`'s facing and the way from it to `p`; 0 for an item
/// on top of it (no direction to measure).
fn dot(from: Anchor, p: Vec3, flat: bool) -> f32 {
    let facing = from.facing.unwrap_or(Vec3::Z);
    let (facing, way) = if flat {
        (facing.with_y(0.0), (p - from.position).with_y(0.0))
    } else {
        (facing, p - from.position)
    };
    match (facing.try_normalize(), way.try_normalize()) {
        (Some(f), Some(w)) => f.dot(w),
        _ => 0.0,
    }
}

/// Metres from `p` to the nearest of `points`; infinite with none.
fn nearest(points: &[Vec3], p: Vec3) -> f32 {
    points
        .iter()
        .map(|q| q.distance(p))
        .fold(f32::INFINITY, f32::min)
}

/// Every other active agent's claim on the ground (decision 2): its target (where
/// it is going) and its position (where it is), the querier's own left out. An
/// agent's target already is its claim, so no registry has to be kept or freed.
fn others(scene: &Scene, querier: Option<u32>) -> Vec<Vec3> {
    let world = &scene.world;
    let mut out = Vec::new();
    for id in world.ids_with_nav_agent() {
        if Some(id) == querier || !world.is_active_in_hierarchy(id) {
            continue;
        }
        let Some(agent) = world.nav_agent(id) else {
            continue;
        };
        if agent.active {
            out.push(agent.target);
            out.push(agent.feet(world.world_position(id)));
        }
    }
    out
}
