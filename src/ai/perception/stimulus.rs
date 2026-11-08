//! The reported senses (#1900): hearing (`ReportNoise`) and damage.
//!
//! A stimulus is remembered under its instigator, the entity that made the noise
//! or dealt the hit, sensed now; a target in sight right now keeps its sight
//! entry (sight is the sharpest sense), but the update still fires, so a script
//! hears every gunshot. A stimulus with no live instigator (none named, or
//! destroyed since) is still sensed: it raises an update with no target and is
//! stored under nobody, since a known target is an entity.
//!
//! **A perceiver never records itself**: an instigator that is the perceiver,
//! or one of its ancestors or descendants (its own body, its own gun), is not
//! heard and does not register a hit. **Only during Play**: outside it (a
//! console call in edit mode) nothing is sensed, so no update waits for a
//! dispatch that never runs.

use glam::Vec3;

use super::perceivers;
use crate::ai::stand;
use crate::components::{AIPerceptionComponent, Known, PerceptionUpdate, Sense};
use crate::scene::Scene;

/// Remember `stimulus` from `target` on `c`, and raise its update.
fn remember(c: &mut AIPerceptionComponent, target: Option<u32>, mut stimulus: Known) {
    stimulus.sensed = true;
    stimulus.age = 0.0;
    stimulus.fresh = true;
    let m = &mut c.memory;
    m.updates.push(PerceptionUpdate {
        target,
        info: stimulus.clone(),
        forgotten: false,
    });
    if let Some(t) = target.filter(|t| !m.in_sight.contains(t)) {
        m.known.insert(t, stimulus);
    }
}

/// Whether `a` and `b` are one hierarchy line: the same entity, or one above
/// the other.
fn related(scene: &Scene, a: u32, b: u32) -> bool {
    let w = &scene.world;
    w.self_and_ancestors(a).any(|x| x == b) || w.self_and_ancestors(b).any(|x| x == a)
}

/// `instigator`, when it names a live entity.
fn alive(scene: &Scene, instigator: Option<u32>) -> Option<u32> {
    instigator.filter(|&i| scene.world.contains(i))
}

/// A noise of `loudness` at `point` (Unreal's `ReportNoiseEvent`): every active
/// perceiver within `hearing_range × loudness` of it hears it, walls or not,
/// placed at `point`; the instigator's own hierarchy does not hear it. Returns
/// how many heard it (0 outside Play).
pub fn report_noise(
    scene: &mut Scene,
    point: Vec3,
    loudness: f32,
    instigator: Option<u32>,
    tag: &str,
) -> usize {
    if !scene.is_playing() {
        return 0;
    }
    let instigator = alive(scene, instigator);
    let mut heard = 0;
    for id in perceivers(scene) {
        if instigator.is_some_and(|i| related(scene, id, i)) {
            continue;
        }
        let at = scene.world.world_position(id);
        let Some(range) = scene.world.ai_perception(id).map(|c| c.hearing_range) else {
            continue;
        };
        if at.distance(point) > range * loudness {
            continue;
        }
        if let Some(mut c) = scene.world.ai_perception_mut(id) {
            let mut k = Known::now(Sense::Hearing, point);
            k.tag = tag.to_string();
            remember(&mut c, instigator, k);
            heard += 1;
        }
    }
    heard
}

/// The perceiver a hit on `struck` reaches: the nearest ancestor-or-self of it
/// with an `AIPerception` (a hitbox under a bone is the character's), when it is
/// active.
pub fn perceiver_of(scene: &Scene, struck: u32) -> Option<u32> {
    let w = &scene.world;
    w.self_and_ancestors(struck)
        .find(|&id| w.has_ai_perception(id))
        .filter(|&id| w.is_active_in_hierarchy(id))
}

/// A hit on `struck` (Unreal's `ReportDamageEvent`): its perceiver
/// ([`perceiver_of`]) senses `amount` of damage of type `tag`, placed where the
/// instigator stands ([`stand::feet`]) when it is alive, else at `at` (the hit
/// point, the blast origin, or the struck entity). Returns the perceiver, if
/// one sensed it: none outside Play or when the instigator is the perceiver's
/// own hierarchy.
pub fn report_damage(
    scene: &mut Scene,
    struck: u32,
    instigator: Option<u32>,
    at: Vec3,
    (amount, tag): (f64, &str),
) -> Option<u32> {
    if !scene.is_playing() {
        return None;
    }
    let perceiver = perceiver_of(scene, struck)?;
    let instigator = alive(scene, instigator);
    if instigator.is_some_and(|i| related(scene, perceiver, i)) {
        return None;
    }
    let position = instigator.map_or(at, |i| stand::feet(scene, i));
    let mut c = scene.world.ai_perception_mut(perceiver)?;
    let mut k = Known::now(Sense::Damage, position);
    k.tag = tag.to_string();
    k.amount = amount;
    remember(&mut c, instigator, k);
    Some(perceiver)
}

/// Drop `target` from what `id` knows, or everything with no target. Raises no
/// update (Unreal's `ForgetActor`); a target still in view is seen again next step.
pub fn forget(scene: &mut Scene, id: u32, target: Option<u32>) {
    let Some(mut c) = scene.world.ai_perception_mut(id) else {
        return;
    };
    let m = &mut c.memory;
    match target {
        Some(t) => {
            m.known.remove(&t);
            m.in_sight.remove(&t);
        }
        None => {
            m.known.clear();
            m.in_sight.clear();
        }
    }
}
