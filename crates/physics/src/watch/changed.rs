use std::collections::BTreeSet;

use super::Owners;
use rusty_ecs::{Changes, Channel, Cursor, World};

#[derive(Debug, Default)]
pub(crate) struct Changed {
    pub all: Vec<u32>,
    pub collider: Vec<u32>,
    pub rigidbody: Vec<u32>,
    pub character: Vec<u32>,
    pub activity: Vec<u32>,
    pub layer: Vec<u32>,
    pub statics: Vec<u32>,
    pub mesh: Vec<u32>,
    /// Teleported (`World::cut_pose`): a dynamic body's pose follows its Transform.
    pub cut: Vec<u32>,
    pub joints: bool,
}

impl Changed {
    pub(crate) fn carriers(&self) -> impl Iterator<Item = &u32> {
        self.rigidbody.iter().chain(&self.character)
    }

    pub(crate) fn homes(&self) -> impl Iterator<Item = &u32> {
        self.collider
            .iter()
            .chain(self.carriers())
            .chain(&self.activity)
    }

    pub(crate) fn reshaped(&self) -> BTreeSet<u32> {
        self.collider.iter().chain(&self.mesh).copied().collect()
    }
}

pub(super) fn take<T: Channel>(
    world: &mut World,
    cursor: &mut Cursor<T>,
    ids: &mut Vec<u32>,
) -> bool {
    match world.changes(cursor) {
        Changes::Rebuild => false,
        Changes::Ids(found) => {
            // `Log::since` already returns its ids ascending and each once, and
            // every cursor reads into a list of its own.
            *ids = found;
            true
        }
    }
}

pub(crate) struct Named {
    pub every: Owners,
    pub activity: Owners,
    pub collider: Owners,
    pub layer: Owners,
    pub fits: Owners,
}

impl Named {
    pub(crate) fn all() -> Self {
        Self {
            every: Owners::All,
            activity: Owners::All,
            collider: Owners::All,
            layer: Owners::All,
            fits: Owners::All,
        }
    }
}

#[cfg(test)]
#[derive(Default, Debug)]
pub(crate) struct Visits {
    pub members: std::cell::Cell<u64>,
    pub changed: usize,
    pub every: usize,
    pub activity: usize,
    pub collider: usize,
    pub layer: usize,
    pub fits: usize,
}

#[cfg(test)]
impl Visits {
    pub(crate) fn record(&mut self, named: &Named, plan: &crate::compound::BodyPlan) {
        let n = |o: &Owners| o.pick(plan).count();
        self.every = n(&named.every);
        self.activity = n(&named.activity);
        self.collider = n(&named.collider);
        self.layer = n(&named.layer);
        self.fits = n(&named.fits);
    }
}
