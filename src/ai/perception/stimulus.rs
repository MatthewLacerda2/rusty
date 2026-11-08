//! The reported senses (#1900): hearing (`ReportNoise`) and damage.
//!
//! A stimulus is remembered under its instigator, the entity that made the noise
//! or dealt the hit, sensed now; a target in sight right now keeps its sight
//! entry (sight is the sharpest sense), but the update still fires, so a script
//! hears every gunshot. A stimulus with no instigator is still sensed: it raises
//! an update with no target and is stored under nobody, since a known target is
//! an entity.

use glam::Vec3;

use super::perceivers;
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

/// A noise of `loudness` at `point` (Unreal's `ReportNoiseEvent`): every active
/// perceiver within `hearing_range × loudness` of it hears it, walls or not; the
/// instigator does not hear its own noise. Returns how many heard it.
pub fn report_noise(
    scene: &mut Scene,
    point: Vec3,
    loudness: f32,
    instigator: Option<u32>,
    tag: &str,
) -> usize {
    let mut heard = 0;
    for id in perceivers(scene) {
        if Some(id) == instigator {
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

/// A hit on `struck` (Unreal's `ReportDamageEvent`): the nearest active
/// ancestor-or-self of it with an `AIPerception` (a hitbox under a bone is the
/// character's) senses `amount` of damage of type `tag`, placed at the
/// instigator's position when the instigator is alive, else at `at` (the hit
/// point, the blast origin, or the struck entity). Returns the perceiver, if any.
pub fn report_damage(
    scene: &mut Scene,
    struck: u32,
    instigator: Option<u32>,
    at: Vec3,
    (amount, tag): (f64, &str),
) -> Option<u32> {
    let w = &scene.world;
    let perceiver = w
        .self_and_ancestors(struck)
        .find(|&id| w.has_ai_perception(id))
        .filter(|&id| w.is_active_in_hierarchy(id))?;
    let alive = instigator.filter(|&i| w.contains(i));
    let position = alive.map_or(at, |i| w.world_position(i));
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
