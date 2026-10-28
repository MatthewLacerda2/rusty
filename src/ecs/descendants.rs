//! src/ecs/descendants.rs — the one bounded downward hierarchy walk (#991).
//!
//! The counterpart of [`World::ancestors`]: every subtree walk (prefab extract and
//! instance membership, skeleton despawn, ragdoll bones, selection bounds, the
//! load survivors) goes through here. Each id is visited at most once, so a
//! malformed `children` list — a cycle, or a child listed twice — ends the walk
//! instead of hanging it, and a child id no longer in the world is skipped.
//!
//! It also holds the hierarchy *paths* (#1518): names joined by `/` from a root
//! down, the form `Scene.FindChild` takes below an entity, Unity's
//! `transform.Find`. Each segment takes the first child of that name in hierarchy
//! order, so a path names one entity even among duplicate names.

use std::collections::BTreeSet;

use super::bundle::Core;
use super::world::World;

/// A pre-order walk of a subtree, children in their declared order. Built by
/// [`World::self_and_descendants`] / [`World::self_and_descendants_while`].
pub struct Descendants<'w, F> {
    world: &'w World,
    stack: Vec<u32>,
    seen: BTreeSet<u32>,
    descend: F,
}

impl<F: FnMut(u32) -> bool> Iterator for Descendants<'_, F> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        loop {
            let id = self.stack.pop()?;
            if !self.world.contains(id) || !self.seen.insert(id) {
                continue;
            }
            if (self.descend)(id) {
                // Reversed onto the stack so they come off in declared order.
                self.stack.extend(self.world.children(id).into_iter().rev());
            }
            return Some(id);
        }
    }
}

impl World {
    /// Push `id`'s child ids onto `out`, allocating nothing once `out` is warm: the
    /// world store's dirty walk, which bounds itself (`world_space::store`).
    pub(in crate::ecs) fn push_children(&self, id: u32, out: &mut Vec<u32>) {
        if let Some(core) = self.component::<Core>(id) {
            out.extend_from_slice(&core.children);
        }
    }

    /// `id`, then every descendant, pre-order in declared child order; nothing for a
    /// dead id. See the module note for what a malformed hierarchy does.
    pub fn self_and_descendants(&self, id: u32) -> Descendants<'_, fn(u32) -> bool> {
        self.self_and_descendants_while(id, |_| true)
    }

    /// [`World::self_and_descendants`], but the walk goes below an entity only when
    /// `descend` says so — the entity itself is still yielded. A prefab instance uses
    /// it to stop at entities that are not its own.
    pub fn self_and_descendants_while<F: FnMut(u32) -> bool>(
        &self,
        id: u32,
        descend: F,
    ) -> Descendants<'_, F> {
        Descendants {
            world: self,
            stack: vec![id],
            seen: BTreeSet::new(),
            descend,
        }
    }

    /// The descendant of `id` at `path` (`"Viewport/Content"`), one level per
    /// `/`-separated segment, first child of that name; `id` itself for an empty
    /// path, `None` for a dead `id` or a missing segment. `Scene.FindChild`.
    pub fn find_child(&self, id: u32, path: &str) -> Option<u32> {
        let at = Some(id).filter(|&id| self.contains(id));
        segments(path).try_fold(at?, |p, name| self.child_named(self.children(p), name))
    }

    /// The entity at the full hierarchy `path` from a root (`"Arena/Crate_1"`): the
    /// first root of that name, in insertion order, then [`World::find_child`].
    pub fn find_path(&self, path: &str) -> Option<u32> {
        let mut segments = segments(path);
        let roots = self
            .ids()
            .iter()
            .copied()
            .filter(|&id| self.ancestors(id).next().is_none());
        let root = self.child_named(roots, segments.next()?)?;
        segments.try_fold(root, |p, name| self.child_named(self.children(p), name))
    }

    /// `id`'s full hierarchy path, root first, which [`World::find_path`] reads back;
    /// `None` for a dead id.
    pub fn path_of(&self, id: u32) -> Option<String> {
        let mut names = self
            .self_and_ancestors(id)
            .map(|e| self.name(e).map(|n| n.clone()))
            .collect::<Option<Vec<String>>>()?;
        names.reverse();
        Some(names.join("/"))
    }

    /// The first of `candidates` named `name`.
    fn child_named(&self, candidates: impl IntoIterator<Item = u32>, name: &str) -> Option<u32> {
        candidates
            .into_iter()
            .find(|&c| self.name(c).is_some_and(|n| *n == *name))
    }
}

/// A path's non-empty `/`-separated segments.
fn segments(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|s| !s.is_empty())
}
