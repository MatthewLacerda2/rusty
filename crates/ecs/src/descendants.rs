//! A bounded downward walk: each id is visited once, so a cycle or a child listed twice
//! ends it, and a dead child is skipped.

use rusty_core::collections::Set;

use super::bundle::Core;
use super::world::World;

pub struct Descendants<'w, F> {
    world: &'w World,
    stack: Vec<u32>,
    /// Membership only, never iterated, so its order reaches no reader.
    seen: Set<u32>,
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
                let at = self.stack.len();
                self.world.push_children(id, &mut self.stack);
                self.stack[at..].reverse(); // off the stack in declared order
            }
            return Some(id);
        }
    }
}

impl World {
    /// Allocates nothing once `out` is warm; returns `id`'s static flag (`false` when dead).
    pub fn push_children(&self, id: u32, out: &mut Vec<u32>) -> bool {
        let Some(core) = self.component::<Core>(id) else {
            return false;
        };
        out.extend_from_slice(&core.children);
        core.is_static
    }

    pub fn self_and_descendants(&self, id: u32) -> Descendants<'_, fn(u32) -> bool> {
        self.self_and_descendants_while(id, |_| true)
    }

    pub fn self_and_descendants_while<F: FnMut(u32) -> bool>(
        &self,
        id: u32,
        descend: F,
    ) -> Descendants<'_, F> {
        Descendants {
            world: self,
            stack: vec![id],
            seen: Set::default(),
            descend,
        }
    }

    pub fn find_child(&self, id: u32, path: &str) -> Option<u32> {
        let at = Some(id).filter(|&id| self.contains(id));
        segments(path).try_fold(at?, |p, name| self.child_named(self.children(p), name))
    }

    pub fn find_path(&self, path: &HierarchyPath) -> Option<u32> {
        let mut at: Option<u32> = None;
        for (name, ordinal) in &path.0 {
            let siblings = at.map_or_else(|| self.roots(), |p| self.children(p));
            let mut same = siblings.into_iter().filter(|&c| self.is_named(c, name));
            at = Some(same.nth(*ordinal)?);
        }
        at
    }

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

    fn roots(&self) -> Vec<u32> {
        let ids = self.ids().iter().copied();
        ids.filter(|&id| self.ancestors(id).next().is_none())
            .collect()
    }

    fn is_named(&self, id: u32, name: &str) -> bool {
        self.name(id).is_some_and(|n| *n == *name)
    }

    fn child_named(&self, candidates: impl IntoIterator<Item = u32>, name: &str) -> Option<u32> {
        candidates.into_iter().find(|&c| self.is_named(c, name))
    }
}

/// Root first, each segment a name and its ordinal among same-named siblings. A value, so a `/`
/// in a name needs no escaping.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HierarchyPath(Vec<(String, usize)>);

fn segments(path: &str) -> impl Iterator<Item = &str> {
    path.split('/').filter(|s| !s.is_empty())
}
