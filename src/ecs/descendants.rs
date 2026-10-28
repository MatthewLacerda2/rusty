//! src/ecs/descendants.rs — the one bounded downward hierarchy walk (#991).
//!
//! The counterpart of [`World::ancestors`]: every subtree walk (prefab extract and
//! instance membership, skeleton despawn, ragdoll bones, selection bounds, the
//! load survivors) goes through here. Each id is visited at most once, so a
//! malformed `children` list — a cycle, or a child listed twice — ends the walk
//! instead of hanging it, and a child id no longer in the world is skipped.
//!
//! It also holds the two ways to name an entity by where it sits:
//! - `Scene.FindChild`'s path below an entity (Unity's `transform.Find`): names
//!   joined by `/`, each segment the first child of that name;
//! - a [`HierarchyPath`] (#1518), root first, each segment a name *and* its
//!   ordinal among same-named siblings, so five `Crate`s under one `Arena` are five
//!   paths. It is a value, never a string, so a `/` in a name needs no escaping.

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

    /// The entity at `path`, which [`World::path_of`] gave; `None` when the
    /// hierarchy has no entity there.
    pub fn find_path(&self, path: &HierarchyPath) -> Option<u32> {
        let mut at: Option<u32> = None;
        for (name, ordinal) in &path.0 {
            let siblings = at.map_or_else(|| self.roots(), |p| self.children(p));
            let mut same = siblings.into_iter().filter(|&c| self.is_named(c, name));
            at = Some(same.nth(*ordinal)?);
        }
        at
    }

    /// `id`'s [`HierarchyPath`], root first; `None` for a dead or unnamed id.
    pub fn path_of(&self, id: u32) -> Option<HierarchyPath> {
        let mut steps = Vec::new();
        for e in self.self_and_ancestors(id) {
            let name = self.name(e)?.clone();
            let parent = self.ancestors(e).next();
            let siblings = parent.map_or_else(|| self.roots(), |p| self.children(p));
            let before = siblings.into_iter().take_while(|&s| s != e);
            let ordinal = before.filter(|&s| self.is_named(s, &name)).count();
            steps.push((name, ordinal));
        }
        steps.reverse();
        Some(HierarchyPath(steps))
    }

    /// The entities with no live parent, in insertion order.
    fn roots(&self) -> Vec<u32> {
        let ids = self.ids().iter().copied();
        ids.filter(|&id| self.ancestors(id).next().is_none())
            .collect()
    }

    fn is_named(&self, id: u32, name: &str) -> bool {
        self.name(id).is_some_and(|n| *n == *name)
    }

    /// The first of `candidates` named `name`.
    fn child_named(&self, candidates: impl IntoIterator<Item = u32>, name: &str) -> Option<u32> {
        candidates.into_iter().find(|&c| self.is_named(c, name))
    }
}

/// Where an entity sits, root first: each segment its name and its ordinal among
/// the same-named siblings (0 for the first) in hierarchy order, roots in
/// insertion order. [`World::path_of`] makes one and [`World::find_path`] reads it
/// back, in the same World or in another one built from the same scene (#1518).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HierarchyPath(Vec<(String, usize)>);

/// A path's non-empty `/`-separated segments.
fn segments(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|s| !s.is_empty())
}
