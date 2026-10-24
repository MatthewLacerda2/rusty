//! src/components/particle/sub_emitter.rs — sub-emitter references (#439).
//!
//! A sub-emitter is another entity's `ParticleEmitterComponent` (typically a child,
//! Unity-style, left inactive so it only fires on demand). When one of this
//! emitter's particles is born, dies or hits a collider, the target fires its
//! `burst_count` at that particle's position, inheriting a fraction of its
//! velocity: spark → smoke puff, debris → dust on landing.
//!
//! The targets are entity ids, so they follow the entity through prefab save /
//! stamp ([`SubEmitters::visit_refs`], the one id-field walk, `components::entity::refs`).

use serde::{Deserialize, Serialize};

use crate::components::RefVisit;

crate::core::named_enum! {
    /// The moment in a particle's life that fires a sub-emitter.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SubEmitTrigger {
        Birth = "birth",
        Death = "death",
        Collision = "collision",
    }
}

/// Which emitter (entity id) fires at each trigger, plus velocity inheritance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubEmitters {
    #[serde(default)]
    pub birth: Option<u32>,
    #[serde(default)]
    pub death: Option<u32>,
    #[serde(default)]
    pub collision: Option<u32>,
    /// Fraction of the parent particle's velocity each sub-particle inherits.
    #[serde(default)]
    pub inherit_velocity: f32,
}

impl SubEmitters {
    /// The JSON pointers (inside the component) of its entity references — how
    /// prefab write-back finds the override leaves holding entity ids.
    pub const REF_POINTERS: [&'static str; 3] = [
        "/sub_emitters/birth",
        "/sub_emitters/death",
        "/sub_emitters/collision",
    ];

    /// The target for `trigger`, if one is set.
    pub fn get(&self, trigger: SubEmitTrigger) -> Option<u32> {
        match trigger {
            SubEmitTrigger::Birth => self.birth,
            SubEmitTrigger::Death => self.death,
            SubEmitTrigger::Collision => self.collision,
        }
    }

    /// Set (or clear) the target for `trigger`.
    pub fn set(&mut self, trigger: SubEmitTrigger, target: Option<u32>) {
        match trigger {
            SubEmitTrigger::Birth => self.birth = target,
            SubEmitTrigger::Death => self.death = target,
            SubEmitTrigger::Collision => self.collision = target,
        }
    }

    /// Visit every target slot with its pointer (under the emitter component); a
    /// dropped target is cleared.
    pub fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        let slots = [&mut self.birth, &mut self.death, &mut self.collision];
        for (p, r) in Self::REF_POINTERS.into_iter().zip(slots) {
            visit(p, r);
        }
    }

    /// Every target set, with its pointer.
    pub fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        let slots = [self.birth, self.death, self.collision];
        for (p, r) in Self::REF_POINTERS.into_iter().zip(slots) {
            r.inspect(|&r| visit(p, r));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_walk_rewrites_and_drops_targets() {
        let mut s = SubEmitters {
            birth: Some(1),
            death: Some(2),
            collision: None,
            inherit_velocity: 0.5,
        };
        s.visit_refs(&mut |_, r| *r = r.and_then(|id| (id != 2).then_some(id + 10)));
        assert_eq!((s.birth, s.death, s.collision), (Some(11), None, None));
    }
}
