mod sight;
pub mod stimulus;

#[cfg(test)]
mod budget_tests;
// Test hook: the level this crate's and `rusty-dev`'s cost tests time; a sim crate
// reads no clock.
#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod bench;
#[cfg(test)]
mod mask_tests;
#[cfg(all(test, feature = "dev"))]
mod perception_cost_tests;
#[cfg(test)]
mod self_tests;
#[cfg(test)]
mod sight_tests;
#[cfg(test)]
mod stimulus_tests;
#[cfg(any(test, feature = "dev"))]
#[doc(hidden)]
pub mod test_world;

use std::collections::BTreeMap;

use glam::Vec3;

pub use stimulus::{forget, perceiver_of, report_damage, report_noise};

use rusty_components::{AIPerceptionComponent, Known, PerceptionUpdate as Update, Sense};
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

/// [`SightRays::per_tick`]'s default: rusty's value for Unreal's `MaxTracesPerTick`.
pub const DEFAULT_SIGHT_RAYS: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SightRays {
    /// Sight rays per tick, across all perceivers.
    pub per_tick: usize,
    /// The last (perceiver, target) pair traced; the next tick starts after it.
    last: Option<(u32, u32)>,
}

impl Default for SightRays {
    fn default() -> Self {
        Self {
            per_tick: DEFAULT_SIGHT_RAYS,
            last: None,
        }
    }
}

impl SightRays {
    pub fn reset(&mut self) {
        self.last = None;
    }
}

/// The active entities carrying an `AIPerception`, in id order.
pub fn perceivers(scene: &Scene) -> Vec<u32> {
    let w = &scene.world;
    let mut ids: Vec<u32> = w
        .ids_with_ai_perception()
        .into_iter()
        .filter(|&id| w.is_active_in_hierarchy(id))
        .collect();
    ids.sort_unstable();
    ids
}

pub fn step(scene: &mut Scene, physics: Option<&PhysicsWorld>, rays: &mut SightRays, dt: f32) {
    let ids = perceivers(scene);
    if ids.is_empty() {
        return; // no memory to age, no ray to schedule: sight walks nothing
    }
    let seen = sight::look(scene, physics, &ids, rays);
    for (id, visible) in ids.into_iter().zip(seen) {
        if let Some(mut c) = scene.world.ai_perception_mut(id) {
            settle(&mut c, &visible, dt);
        }
    }
}

/// Fold one tick's sight into `c`'s memory; age the rest.
fn settle(c: &mut AIPerceptionComponent, visible: &BTreeMap<u32, Vec3>, dt: f32) {
    let (sight, hearing, damage) = (c.sight_max_age, c.hearing_max_age, c.damage_max_age);
    let limit_of = |sense| match sense {
        Sense::Sight => sight,
        Sense::Hearing => hearing,
        Sense::Damage => damage,
    };
    let m = &mut c.memory;
    for (&target, &position) in visible {
        let entry = Known::now(Sense::Sight, position);
        if !m.in_sight.contains(&target) {
            m.updates.push(update(target, &entry, false));
        }
        m.known.insert(target, entry);
    }
    let lost: Vec<u32> = m
        .in_sight
        .iter()
        .copied()
        .filter(|t| !visible.contains_key(t))
        .collect();
    for target in lost {
        if let Some(k) = m.known.get_mut(&target) {
            k.sensed = false;
            let k = k.clone();
            m.updates.push(update(target, &k, false));
        }
    }
    m.in_sight = visible.keys().copied().collect();
    let mut forgotten = Vec::new();
    for (&target, k) in m.known.iter_mut() {
        if visible.contains_key(&target) {
            continue;
        }
        if std::mem::take(&mut k.fresh) {
            continue; // reported since the last step: sensed this tick
        }
        k.sensed = false;
        k.age += dt;
        let limit = limit_of(k.sense);
        if limit > 0.0 && k.age > limit {
            forgotten.push(target);
        }
    }
    for target in forgotten {
        if let Some(k) = m.known.remove(&target) {
            m.updates.push(update(target, &k, true));
        }
    }
}

/// An update about `target`, as `entry` stands.
fn update(target: u32, entry: &Known, forgotten: bool) -> Update {
    Update {
        target: Some(target),
        info: entry.clone(),
        forgotten,
    }
}

pub fn drain_updates(scene: &mut Scene) -> Vec<(u32, Update)> {
    let mut ids = scene.world.ids_with_ai_perception();
    ids.sort_unstable();
    let mut out = Vec::new();
    for id in ids {
        // Read first: a perceiver with nothing pending is not written (no stamp).
        if scene
            .world
            .ai_perception(id)
            .is_none_or(|c| c.memory.updates.is_empty())
        {
            continue;
        }
        let Some(mut c) = scene.world.ai_perception_mut(id) else {
            continue;
        };
        out.extend(c.memory.updates.drain(..).map(|u| (id, u)));
    }
    out
}
