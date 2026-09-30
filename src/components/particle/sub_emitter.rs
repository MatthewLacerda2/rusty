//! src/components/particle/sub_emitter.rs — sub-emitter references (#439).
//!
//! A sub-emitter is another entity's `ParticleEmitterComponent` (typically a child,
//! Unity-style, left inactive so it only fires on demand). When one of this
//! emitter's particles is born, dies or hits a collider, the target fires its
//! `burst_count` at that particle's position, inheriting a fraction of its
//! velocity: spark → smoke puff, debris → dust on landing.
//!
//! The targets are entity ids, so they follow the entity through prefab save /
//! stamp ([`SubEmitters::remap_refs`], the same mechanism as a Joint's body).

use serde::{Deserialize, Serialize};

/// The moment in a particle's life that fires a sub-emitter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubEmitTrigger {
    Birth,
    Death,
    Collision,
}

impl SubEmitTrigger {
    pub const ALL: [Self; 3] = [Self::Birth, Self::Death, Self::Collision];

    /// Lowercase name (`"birth"` / `"death"` / `"collision"`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Birth => "birth",
            Self::Death => "death",
            Self::Collision => "collision",
        }
    }

    /// Parse a trigger name, case-insensitively.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(name))
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

    /// Rewrite every target through `map`; a target `map` drops is cleared.
    pub fn remap_refs(&mut self, map: &dyn Fn(u32) -> Option<u32>) {
        for t in SubEmitTrigger::ALL {
            self.set(t, self.get(t).and_then(map));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remap_rewrites_and_drops_targets() {
        let mut s = SubEmitters {
            birth: Some(1),
            death: Some(2),
            collision: None,
            inherit_velocity: 0.5,
        };
        s.remap_refs(&|id| (id != 2).then_some(id + 10));
        assert_eq!((s.birth, s.death, s.collision), (Some(11), None, None));
        assert_eq!(SubEmitTrigger::parse("DEATH"), Some(SubEmitTrigger::Death));
        assert_eq!(SubEmitTrigger::parse("landing"), None);
    }
}
