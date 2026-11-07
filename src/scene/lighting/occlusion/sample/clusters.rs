//! The occlusion bake's clusters (#1714): the objects it can cull, grouped by
//! the [`CLUSTER`] cube their centre lies in, so one polygon that hides a
//! cluster's box from an eye spares every object in it (`baking`).

use std::collections::BTreeMap;

use glam::Vec3;

use super::baking::Targets;

/// The edge of a cluster's cube, metres: about a room.
const CLUSTER: f32 = 8.0;

/// Some objects near one another, and the box round them.
pub(super) struct Cluster {
    pub bounds: (Vec3, Vec3),
    /// Indices into the bake's objects, ascending.
    pub objects: Vec<u32>,
}

/// The objects of `targets` that have targets, clustered, in the cubes' order.
pub(super) fn clusters(targets: &[Targets]) -> Vec<Cluster> {
    let mut cubes: BTreeMap<[i32; 3], Cluster> = BTreeMap::new();
    let sampled = targets.iter().enumerate().filter(|(_, t)| t.2.is_some());
    for (i, (lo, hi, _)) in sampled {
        let key = ((*lo + *hi) * 0.5 / CLUSTER).floor().as_ivec3().to_array();
        let c = cubes.entry(key).or_insert(Cluster {
            bounds: (*lo, *hi),
            objects: Vec::new(),
        });
        c.bounds = (c.bounds.0.min(*lo), c.bounds.1.max(*hi));
        c.objects.push(i as u32);
    }
    cubes.into_values().collect()
}
