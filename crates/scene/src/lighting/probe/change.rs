//! Which points a change to some probe sets can reach: why the test holds is
//! `docs/api/Lighting.md`, *Probes per loaded scene*.

use glam::Vec3;

use super::{ProbeField, ProbeVolume};

pub type Bounds = (Vec3, Vec3);

/// `key` moves when the set changes: its revision, or a content hash.
#[derive(Clone, Debug, PartialEq)]
pub struct SetStamp<K> {
    pub handle: u32,
    pub key: K,
    pub bounds: Option<Bounds>,
}

impl<K> SetStamp<K> {
    pub fn of(handle: u32, key: K, set: &ProbeVolume) -> Self {
        Self {
            handle,
            key,
            bounds: set.bounds(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldChange {
    handles: Vec<u32>,
    boxes: Vec<Bounds>,
}

impl FieldChange {
    pub fn between<K: PartialEq>(old: &[SetStamp<K>], new: &[SetStamp<K>]) -> Option<Self> {
        let mut change = Self::default();
        let find = |list: &[SetStamp<K>], h: u32| list.iter().position(|s| s.handle == h);
        for (side, other) in [(old, new), (new, old)] {
            for stamp in side {
                let same = find(other, stamp.handle).is_some_and(|i| other[i].key == stamp.key);
                if same {
                    continue;
                }
                if !change.handles.contains(&stamp.handle) {
                    change.handles.push(stamp.handle);
                }
                change.boxes.extend(stamp.bounds);
            }
        }
        (!change.handles.is_empty()).then_some(change)
    }

    pub fn touches(&self, handle: u32) -> bool {
        self.handles.contains(&handle)
    }

    /// Whether anything within `radius` of `centre` may sample differently.
    pub fn reaches(&self, field: &ProbeField<'_>, centre: Vec3, radius: f32) -> bool {
        if self.boxes.is_empty() {
            return false;
        }
        if !centre.is_finite() || !radius.is_finite() {
            return true;
        }
        let reach = field.reach_without(centre, |h| !self.touches(h));
        let gap = |&(lo, hi): &Bounds| centre.clamp(lo, hi).distance(centre);
        self.boxes.iter().any(|b| gap(b) <= reach + 2.0 * radius)
    }
}

#[cfg(test)]
#[path = "change_tests.rs"]
mod tests;
