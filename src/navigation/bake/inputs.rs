//! src/navigation/bake/inputs.rs — what the last bake read, to find what changed (#456).
//!
//! The bake's inputs are the static colliders (shape + world pose), the carving
//! obstacles (their [`ObstacleVolume`]) and the scene's `nav_settings`. The bake
//! keeps a record of each input with the cells it covered. Diffing that record
//! against the scene marks a collider or obstacle that was added, removed, moved,
//! resized or toggled: its old cells and its new cells are dirty. Records are kept
//! by ascending entity id, so the diff is a merge-join and its order is fixed.
//!
//! Change tracking is a per-tick comparison, not a hook on every writer: a script,
//! the physics write-back, the editor and a parent's move all change the pose the
//! same way, and none of them can forget to report it.

use glam::Mat4;

use super::super::bounds::static_collider_ids;
use super::super::obstacle::{carving_volumes, ObstacleVolume};
use super::super::NavMeshSettings;
use super::region::CellRect;
use crate::components::ColliderShape;
use crate::scene::Scene;

/// One input as last baked: whose it is, what it was, and the cells it covered.
#[derive(Clone, Debug)]
pub(super) struct Source<K> {
    pub id: u32,
    pub key: K,
    pub rect: Option<CellRect>,
}

/// A static collider's identity for the bake: its shape at its world pose.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ColliderKey {
    shape: ColliderShape,
    pose: Mat4,
}

/// Everything the last bake read.
pub(super) struct BakeInputs {
    pub settings: NavMeshSettings,
    pub colliders: Vec<Source<ColliderKey>>,
    pub obstacles: Vec<Source<ObstacleVolume>>,
}

impl BakeInputs {
    /// The colliders whose recorded cells reach `region`.
    pub fn collider_ids_in(&self, region: CellRect) -> Vec<u32> {
        touching(&self.colliders, region).map(|s| s.id).collect()
    }

    /// The carving obstacles whose recorded cells reach `region`.
    pub fn obstacles_in(&self, region: CellRect) -> Vec<&ObstacleVolume> {
        touching(&self.obstacles, region).map(|s| &s.key).collect()
    }
}

fn touching<K>(sources: &[Source<K>], region: CellRect) -> impl Iterator<Item = &Source<K>> {
    sources
        .iter()
        .filter(move |s| s.rect.and_then(|r| r.intersection(region)).is_some())
}

/// The bake's static colliders with their keys, by ascending id.
pub(super) fn collider_keys(scene: &Scene) -> Vec<(u32, ColliderKey)> {
    let mut out: Vec<_> = static_collider_ids(scene)
        .filter_map(|id| {
            let shape = scene.world.collider(id)?.shape.clone();
            let pose = scene.compute_world_matrix(id);
            Some((id, ColliderKey { shape, pose }))
        })
        .collect();
    out.sort_by_key(|&(id, _)| id);
    out
}

/// The carving obstacles with their volumes, by ascending id.
pub(super) fn obstacle_keys(scene: &Scene) -> Vec<(u32, ObstacleVolume)> {
    carving_volumes(scene)
}

/// Diff the records `old` against the current keys `new` (both by ascending id).
/// Returns the new records; a changed, added or removed input pushes its old and
/// new cells onto `dirty`. `rect_of` measures a new or changed input's cells.
pub(super) fn diff<K: PartialEq>(
    old: &[Source<K>],
    new: Vec<(u32, K)>,
    mut rect_of: impl FnMut(u32, &K) -> Option<CellRect>,
    dirty: &mut Vec<CellRect>,
) -> Vec<Source<K>> {
    let mut out = Vec::with_capacity(new.len());
    let mut old_iter = old.iter().peekable();
    for (id, key) in new {
        while let Some(gone) = old_iter.next_if(|s| s.id < id) {
            dirty.extend(gone.rect);
        }
        let rect = match old_iter.next_if(|s| s.id == id) {
            Some(prev) if prev.key == key => prev.rect,
            Some(prev) => {
                dirty.extend(prev.rect);
                let rect = rect_of(id, &key);
                dirty.extend(rect);
                rect
            }
            None => {
                let rect = rect_of(id, &key);
                dirty.extend(rect);
                rect
            }
        };
        out.push(Source { id, key, rect });
    }
    dirty.extend(old_iter.filter_map(|gone| gone.rect));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: i32) -> Option<CellRect> {
        Some(CellRect {
            x0: x,
            z0: 0,
            x1: x,
            z1: 0,
        })
    }

    #[test]
    fn diff_marks_added_removed_and_changed_inputs() {
        let src = |id: u32, key: i32| Source {
            id,
            key,
            rect: rect(id as i32),
        };
        let old = vec![src(1, 10), src(2, 20), src(3, 30)];
        let new = vec![(2, 20), (3, 31), (4, 40)];
        let mut dirty = Vec::new();
        let out = diff(&old, new, |id, _| rect(id as i32 + 100), &mut dirty);
        let xs: Vec<i32> = dirty.iter().map(|r| r.x0).collect();
        assert_eq!(
            xs,
            vec![1, 3, 103, 104],
            "removed 1, moved 3 (old + new), added 4"
        );
        assert_eq!(out.iter().map(|s| s.id).collect::<Vec<_>>(), vec![2, 3, 4]);
        assert_eq!(out[0].rect, rect(2), "an unchanged input keeps its record");
    }
}
