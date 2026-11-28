//! During Play the collider diff runs only when the change log names a static entity: a new way to
//! write a static entity's pose, Collider or flags must go through the World's write funnels
//! (`ecs::changes`), or Play's navmesh misses it.

use glam::Mat4;

use super::super::bounds::static_collider_ids;
use super::super::object_modifier::bake_as;
use super::super::obstacle::{carving_volumes, ObstacleVolume};
use super::super::offmesh::AuthoredKey;
use super::super::NavMeshSettings;
use super::modifiers::{modifier_keys, ModifierVolume};
use super::region::CellRect;
use crate::offmesh::authored_keys;
use rusty_components::ColliderShape;
use rusty_scene::Scene;

#[derive(Clone, Debug)]
pub(super) struct Source<K> {
    pub id: u32,
    pub key: K,
    pub rect: Option<CellRect>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ColliderKey {
    shape: ColliderShape,
    pose: Mat4,
    pub area: Option<u8>,
}

pub(super) struct BakeInputs {
    pub settings: NavMeshSettings,
    pub colliders: Vec<Source<ColliderKey>>,
    pub obstacles: Vec<Source<ObstacleVolume>>,
    pub modifiers: Vec<Source<ModifierVolume>>,
    pub links: Vec<Source<AuthoredKey>>,
}

impl BakeInputs {
    pub fn collider_ids_in(&self, region: CellRect) -> Vec<u32> {
        touching(&self.colliders, region).map(|s| s.id).collect()
    }

    pub fn link_keys(&self) -> Vec<(u32, AuthoredKey)> {
        self.links.iter().map(|s| (s.id, s.key)).collect()
    }

    pub fn obstacles_in(&self, region: CellRect) -> Vec<&ObstacleVolume> {
        touching(&self.obstacles, region).map(|s| &s.key).collect()
    }

    /// Every rect moved by a change of the graph's origin.
    pub fn shift(&mut self, dx: i32, dz: i32) {
        let by = |r: &mut Option<CellRect>| *r = r.map(|r| r.shifted(dx, dz));
        self.colliders.iter_mut().for_each(|s| by(&mut s.rect));
        self.obstacles.iter_mut().for_each(|s| by(&mut s.rect));
        self.modifiers.iter_mut().for_each(|s| by(&mut s.rect));
        self.links.iter_mut().for_each(|s| by(&mut s.rect));
    }

    pub fn modifiers_in(&self, region: CellRect) -> Vec<&ModifierVolume> {
        touching(&self.modifiers, region).map(|s| &s.key).collect()
    }
}

pub(super) struct InputKeys {
    /// `None` when the change log saw no static entity change since the last
    /// sync (`still`): the colliders are known unchanged and left unread.
    pub colliders: Option<Vec<(u32, ColliderKey)>>,
    pub obstacles: Vec<(u32, ObstacleVolume)>,
    pub modifiers: Vec<(u32, ModifierVolume)>,
    pub links: Vec<(u32, AuthoredKey)>,
}

impl InputKeys {
    pub fn of(scene: &Scene, statics_still: bool) -> Self {
        Self {
            colliders: (!statics_still).then(|| collider_keys(scene)),
            obstacles: obstacle_keys(scene),
            modifiers: modifier_keys(scene),
            links: authored_keys(scene),
        }
    }
}

impl BakeInputs {
    pub fn are(&self, keys: InputKeys) -> bool {
        let InputKeys {
            colliders,
            obstacles,
            modifiers,
            links,
        } = keys;
        colliders.is_none_or(|c| same(&self.colliders, c))
            && same(&self.obstacles, obstacles)
            && same(&self.modifiers, modifiers)
            && same(&self.links, links)
    }
}

fn same<K: PartialEq>(old: &[Source<K>], new: Vec<(u32, K)>) -> bool {
    old.len() == new.len()
        && old
            .iter()
            .zip(new)
            .all(|(s, (id, k))| s.id == id && s.key == k)
}

fn touching<K>(sources: &[Source<K>], region: CellRect) -> impl Iterator<Item = &Source<K>> {
    sources
        .iter()
        .filter(move |s| s.rect.and_then(|r| r.intersection(region)).is_some())
}

pub(super) fn collider_keys(scene: &Scene) -> Vec<(u32, ColliderKey)> {
    let mut out: Vec<_> = static_collider_ids(scene)
        .filter_map(|id| {
            let shape = scene.world.collider(id)?.shape.clone();
            let pose = scene.world.local_to_world(id);
            let area = bake_as(&scene.world, id).area;
            Some((id, ColliderKey { shape, pose, area }))
        })
        .collect();
    out.sort_by_key(|&(id, _)| id);
    out
}

pub(super) fn obstacle_keys(scene: &Scene) -> Vec<(u32, ObstacleVolume)> {
    carving_volumes(scene)
}

pub(super) fn sources<K>(
    keys: Vec<(u32, K)>,
    mut rect_of: impl FnMut(&K) -> Option<CellRect>,
) -> Vec<Source<K>> {
    keys.into_iter()
        .map(|(id, key)| Source {
            id,
            rect: rect_of(&key),
            key,
        })
        .collect()
}

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
