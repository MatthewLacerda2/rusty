//! One [`Known`] entry per target, in a `BTreeMap` so every walk is in id order (determinism).

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sense {
    Sight,
    Hearing,
    Damage,
}

impl Sense {
    pub fn name(self) -> &'static str {
        match self {
            Sense::Sight => "sight",
            Sense::Hearing => "hearing",
            Sense::Damage => "damage",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Known {
    pub sense: Sense,
    pub position: Vec3,
    pub age: f32,
    pub sensed: bool,
    /// The noise's tag, or the damage type; empty for sight.
    pub tag: String,
    /// The damage dealt by a damage stimulus; 0 otherwise.
    pub amount: f64,
    /// Reported since the last step, so that step keeps it sensed instead of ageing it.
    pub fresh: bool,
}

impl Known {
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

#[derive(Clone, Debug, PartialEq)]
pub struct Update {
    pub target: Option<u32>,
    pub info: Known,
    pub forgotten: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    pub known: BTreeMap<u32, Known>,
    /// The targets seen at the last sight step; a target the ray budget skips
    /// keeps its entry here.
    pub in_sight: BTreeSet<u32>,
    /// Updates raised since the last dispatch, in the order they were raised.
    pub updates: Vec<Update>,
}

impl Memory {
    pub fn is_empty(&self) -> bool {
        self.known.is_empty() && self.in_sight.is_empty() && self.updates.is_empty()
    }
}
