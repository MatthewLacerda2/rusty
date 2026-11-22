//! [`World::touch`] notes a column write for the journal and stamps it in one call, so a raw
//! write cannot do one and forget the other. A subtree's stamping walk runs only while a cursor
//! reads its channel; with none, an `any` keeps `static_revision` moving.

use super::{Activity, Channel, Children, Layer, WorldPose, STATIC};
use crate::World;

/// Kept between walks; a walk started inside another's visit finds it taken and starts cold.
#[derive(Debug, Default)]
pub(super) struct Scratch {
    stack: Vec<u32>,
    seen: Vec<u32>,
    walk: u32,
}

impl Scratch {
    fn begin(&mut self) {
        self.walk = self.walk.wrapping_add(1);
        if self.walk == 0 {
            self.seen.fill(0);
            self.walk = 1;
        }
    }

    fn first_visit(&mut self, id: u32) -> bool {
        let at = id as usize;
        if at >= self.seen.len() {
            self.seen.resize(at + 1, 0);
        }
        std::mem::replace(&mut self.seen[at], self.walk) != self.walk
    }
}

impl World {
    pub(crate) fn touch<T: Channel>(&mut self, id: u32) {
        const { assert!(T::COLUMN, "a derived channel is stamped at its source") };
        self.note(id);
        if T::STATIC_GEOMETRY && self.is_static(id) {
            self.static_geometry_moved();
        }
        if self.changes.get_mut().logs.len() == 1 {
            return; // only `StaticSet` exists: no column was ever opened
        }
        if let Some(at) = self.changes.get_mut().find::<T>() {
            self.stamp_at(at, id);
        }
    }

    /// Called before the row is written, so a removal stamps the columns it had; a placement then
    /// stamps the new row's with [`World::stamp_carried`].
    pub(crate) fn touch_entity(&mut self, id: u32, is_static: bool) {
        self.note(id);
        if is_static {
            self.stamp_static(id);
        }
        if self.changes.get_mut().logs.len() == 1 {
            return;
        }
        self.stamp_carried(id);
        for at in [
            self.changes.get_mut().find::<Activity>(),
            self.changes.get_mut().find::<Layer>(),
            self.changes.get_mut().find::<Children>(),
        ] {
            at.into_iter().for_each(|at| self.stamp_at(at, id));
        }
    }

    pub(crate) fn stamp_carried(&mut self, id: u32) {
        if self.changes.get_mut().logs.len() == 1 {
            return;
        }
        let Some(handle) = self.handles.get(id) else {
            return;
        };
        let Ok(row) = self.inner.entity(handle) else {
            return;
        };
        let live = self.order.len();
        for log in self.changes.get_mut().logs.iter_mut() {
            if log.carried.is_some_and(|carried| carried(&row)) {
                log.stamp(id, live);
            }
        }
    }

    fn stamp_at(&mut self, at: usize, id: u32) {
        let live = self.order.len();
        self.changes.get_mut().logs[at].stamp(id, live);
    }

    pub(crate) fn stamp_static(&mut self, id: u32) {
        self.stamp_at(STATIC, id);
    }

    pub(crate) fn stamp_layer(&mut self, id: u32) {
        if let Some(at) = self.changes.get_mut().find::<Layer>() {
            self.stamp_at(at, id);
        }
    }

    pub(crate) fn stamp_children(&mut self, id: u32) {
        if let Some(at) = self.changes.get_mut().find::<Children>() {
            self.stamp_at(at, id);
        }
    }

    pub(crate) fn stamp_hierarchy_below(&mut self, id: u32) {
        let activity = self.changes.get_mut().find::<Activity>();
        if activity.is_none() && !self.changes.get_mut().logs[STATIC].recording {
            if self.self_and_descendants(id).any(|e| self.is_static(e)) {
                self.stamp_static(id);
            }
            return;
        }
        self.each_below(id, |w, e, is_static| {
            if let Some(at) = activity {
                w.stamp_at(at, e);
            }
            if is_static {
                w.stamp_static(e);
            }
        });
    }

    pub(crate) fn stamp_pose_below(&mut self, id: u32) {
        let Some(at) = self.changes.get_mut().find::<WorldPose>() else {
            return;
        };
        // Still unread: its subtree was walked with that stamp.
        if self.changes.get_mut().logs[at].pending(id) {
            return;
        }
        self.each_below(id, |w, e, is_static| {
            w.stamp_at(at, e);
            if is_static {
                w.static_geometry_moved();
            }
        });
    }

    fn static_geometry_moved(&mut self) {
        self.changes.get_mut().static_geometry += 1;
    }

    fn each_below(&mut self, id: u32, mut visit: impl FnMut(&mut World, u32, bool)) {
        let mut s = std::mem::take(&mut self.changes.get_mut().scratch);
        s.begin();
        s.stack.push(id);
        while let Some(e) = s.stack.pop() {
            if !self.contains(e) || !s.first_visit(e) {
                continue;
            }
            let at = s.stack.len();
            let is_static = self.push_children(e, &mut s.stack);
            s.stack[at..].reverse();
            visit(self, e, is_static);
        }
        self.changes.get_mut().scratch = s;
    }
}
