//! src/components/ai/memory.rs — what an AIPerception knows (#1900).
//!
//! One [`Known`] entry per target entity, keyed by id in a `BTreeMap` so every
//! walk over it is in id order (determinism). The updates a step or a report
//! raised wait in [`Memory::updates`] until the tick's dispatch hands them to
//! `OnPerceptionUpdated`. Runtime state only: none of it is saved.

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;

/// Which sense last placed a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    Sight,
    Hearing,
    Damage,
}

impl Sense {
    /// The name scripts see (`"sight"`, `"hearing"`, `"damage"`).
    pub fn name(self) -> &'static str {
        match self {
            Sense::Sight => "sight",
            Sense::Hearing => "hearing",
            Sense::Damage => "damage",
        }
    }
}

/// One known target: where it was last sensed, and how long ago.
#[derive(Clone, Debug, PartialEq)]
pub struct Known {
    /// The sense that placed `position`.
    pub sense: Sense,
    /// The last-known position: where it was seen, the noise's point, or the
    /// instigator's position at the hit.
    pub position: Vec3,
    /// Seconds since it was last sensed, advanced on the fixed tick.
    pub age: f32,
    /// Sensed this tick: in view right now, or heard / hit this tick.
    pub sensed: bool,
    /// The noise's tag, or the damage type; empty for sight.
    pub tag: String,
    /// The damage dealt by a damage stimulus; 0 otherwise.
    pub amount: f64,
    /// Reported since the last step (a noise or a hit), so that step keeps it
    /// sensed instead of ageing it.
    pub fresh: bool,
}

impl Known {
    /// A stimulus sensed now, by `sense`, at `position`.
    pub fn now(sense: Sense, position: Vec3) -> Self {
        Self {
            sense,
            position,
            age: 0.0,
            sensed: true,
            tag: String::new(),
            amount: 0.0,
            fresh: false,
        }
    }
}

/// One `OnPerceptionUpdated(id, target, info)` waiting to be dispatched.
#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    /// The target entity, or `None` for a noise or hit with no instigator.
    pub target: Option<u32>,
    /// The entry as it stood when the update was raised.
    pub info: Known,
    /// The entry aged out and is gone.
    pub forgotten: bool,
}

/// An agent's memory: its known targets, who it sees, and its pending updates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// Known targets by entity id.
    pub known: BTreeMap<u32, Known>,
    /// The targets seen at the last sight step; a target the ray budget skips
    /// keeps its entry here.
    pub in_sight: BTreeSet<u32>,
    /// Updates raised since the last dispatch, in the order they were raised.
    pub updates: Vec<Update>,
}

impl Memory {
    /// Whether it knows nothing, sees nothing and has nothing pending.
    pub fn is_empty(&self) -> bool {
        self.known.is_empty() && self.in_sight.is_empty() && self.updates.is_empty()
    }
}
