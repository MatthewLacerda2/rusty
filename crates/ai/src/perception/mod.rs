//! crates/ai/src/perception/ — the AI Perception senses (#1900): Unreal's
//! `UAIPerceptionComponent` with *Sight*, *Hearing* and *Damage*, so an agent
//! acts on what it knows instead of reading the player's transform.
//!
//! - **Sight** (`sight`) is stepped every fixed tick by [`step`]: each
//!   `AIPerception` looks for the targets on its `sight_mask` inside its range and
//!   view cone, one ray each through [`ray`](super::ray). Rays are **budgeted**
//!   across all perceivers ([`SightRays`], Unreal's `MaxTracesPerTick`): the
//!   (perceiver, target) pairs that need one are served round-robin in id order,
//!   from where the last tick stopped; a pair not traced keeps last tick's answer.
//! - **Hearing** and **Damage** ([`stimulus`]) are reported, not stepped: a
//!   script's `AIPerception.ReportNoise` and every `Damage.Apply*` call them.
//!   Walls do not block hearing, as in Unreal.
//! - **Memory** ([`step`]'s ageing): an entry not sensed this tick ages by the
//!   fixed dt and is forgotten past its sense's `max_age` (0 keeps it forever).
//!
//! Every change raises an [`Update`] on the perceiver's memory, drained by
//! [`drain_updates`] for `OnPerceptionUpdated`. Everything walks ids in order and
//! keeps its maps in `BTreeMap`s, so a run is a pure function of the scene and
//! its inputs.
//!
//! **Who is a sight target**: an active, non-static entity on the perceiver's
//! `sight_mask` with a body to see (a Collider or a CharacterController), not the
//! perceiver's own body (under it, or above it: a perceiver on a `Head` child),
//! and not under another target (a character's hitboxes are the character).
//! Static entities are the level, never a target. Every last-known position is
//! where the target **stood** ([`stand::feet`](crate::stand::feet)), so an
//! environment query around it reads the ground, not a character's middle.
//!
//! **Play only**: entering Play empties every memory (`Scene::set_playing`,
//! before any `Start` runs) and [`SightRays::reset`] restarts the round; the
//! senses sense nothing outside Play, so edit mode never queues an update.

mod sight;
pub mod stimulus;

#[cfg(test)]
mod budget_tests;
// Test hook: the bench level the root's `dev::bench` times.
#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod bench;
#[cfg(all(test, feature = "dev"))]
mod perception_cost_tests;
#[cfg(test)]
mod self_tests;
#[cfg(test)]
mod sight_tests;
#[cfg(test)]
mod stimulus_tests;
// Test hook: the arena this module's tests, its `bench` and the root's `dev::bench` build
// on; only those use it, so the player never compiles it.
#[cfg(any(test, feature = "dev"))]
#[doc(hidden)]
pub mod test_world;

use std::collections::BTreeMap;

use glam::Vec3;

pub use stimulus::{forget, perceiver_of, report_damage, report_noise};

use rusty_components::{AIPerceptionComponent, Known, PerceptionUpdate as Update, Sense};
use rusty_physics::PhysicsWorld;
use rusty_scene::Scene;

/// Sight rays per tick across all perceivers, by default (decision 5, #1900).
pub const DEFAULT_SIGHT_RAYS: usize = 64;

/// The sight-ray budget and where its round-robin stands. One per world, kept in
/// the app's `Resources`; [`SightRays::reset`] on Play entry, so every session
/// starts the round from the first pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SightRays {
    /// How many sight rays one tick may cast across all perceivers.
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
    /// Start the round from the first pair again.
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

/// One perception tick: sight for every active perceiver, then each one's memory
/// ages by `dt` (the fixed tick's) and forgets what is past its max age.
pub fn step(scene: &mut Scene, physics: Option<&PhysicsWorld>, rays: &mut SightRays, dt: f32) {
    let ids = perceivers(scene);
    let seen = sight::look(scene, physics, &ids, rays);
    for (id, visible) in ids.into_iter().zip(seen) {
        if let Some(mut c) = scene.world.ai_perception_mut(id) {
            settle(&mut c, &visible, dt);
        }
    }
}

/// Fold one tick's sight into `c`'s memory, then age what was not sensed.
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

/// Take every perceiver's pending updates: `(perceiver, update)`, perceivers in id
/// order, each one's in the order they were raised.
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
