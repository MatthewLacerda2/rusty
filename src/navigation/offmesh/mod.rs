//! src/navigation/offmesh/ — off-mesh links (#462): connections between walkable
//! spans that walking can't join (a drop off a ledge, a jump onto a box, a ladder).
//!
//! Links are extra A\* edges stored beside the spans. A link names its two ends as
//! `(cell, layer)` — the layer is the span's position within its cell's column —
//! not as span indices, because an incremental rebake splices the compact span
//! array and shifts the indices of every cell after the change. Cells and layers
//! outside the changed rectangles don't move.
//!
//! Two sources, both one list each:
//! * `generate` — drop and jump links found at bake time along ledges, enabled by
//!   the scene's `NavMeshSettings` (Unity's auto-generated OffMeshLinks).
//! * `authored` — `OffMeshLink` components, their ends snapped onto the navmesh.
//!
//! `exits` indexes every end an agent may leave from (both ends of a two-way link)
//! by `(cell, layer)`, so A\* finds a span's links with a binary search.

mod authored;
mod generate;
#[cfg(test)]
mod tests;

use glam::Vec3;

pub use crate::components::{OffMeshLinkData, OffMeshLinkKind};
pub(super) use authored::{authored_keys, AuthoredKey};
pub(super) use generate::{sort_generated, LinkParams};

use super::{NavigationGraph, SpanRef};

/// One end of a link: a cell and the span's layer in that cell's column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LinkEnd {
    pub gx: i32,
    pub gz: i32,
    pub layer: u32,
}

/// A link in the navmesh: auto-generated (drop, jump) or authored (manual).
#[derive(Clone, Debug, PartialEq)]
pub struct OffMeshLink {
    pub kind: OffMeshLinkKind,
    /// World points the link runs between, start to end.
    pub start: Vec3,
    pub end: Vec3,
    pub from: LinkEnd,
    pub to: LinkEnd,
    /// Whether it can be crossed end to start too. Generated links are one-way.
    pub bidirectional: bool,
    /// The cost of crossing, in world units; negative uses the link's length times
    /// its area's cost.
    pub cost_override: f32,
    /// The link's navigation area (#460): an agent whose mask excludes it never
    /// crosses it. Generated links are `Walkable`.
    pub area: u8,
    /// The authoring entity, `None` for a generated link.
    pub owner: Option<u32>,
}

impl OffMeshLink {
    /// The link as crossed forward (start to end) or backward.
    pub fn data(&self, forward: bool) -> OffMeshLinkData {
        let (start, end) = if forward {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        };
        OffMeshLinkData {
            kind: self.kind,
            start,
            end,
            owner: self.owner,
        }
    }
}

/// The graph's links and their exit index.
#[derive(Default)]
pub(super) struct OffMeshLinks {
    pub auto: Vec<OffMeshLink>,
    pub authored: Vec<OffMeshLink>,
    /// `(cell index, layer, link id, forward)`, sorted: every end a link leaves from.
    exits: Vec<(u32, u32, u32, bool)>,
}

impl OffMeshLinks {
    fn get(&self, id: u32) -> &OffMeshLink {
        let id = id as usize;
        match id.checked_sub(self.auto.len()) {
            Some(i) => &self.authored[i],
            None => &self.auto[id],
        }
    }
}

/// One link move out of a span: where it lands, its A\* cost, its area, and the link
/// crossed.
pub(super) struct LinkMove {
    pub to: SpanRef,
    pub cost: f32,
    pub area: u8,
    pub data: OffMeshLinkData,
}

impl NavigationGraph {
    /// Every off-mesh link in the navmesh: the generated ones (ordered by their
    /// source cell), then the authored ones (by entity id).
    pub fn offmesh_links(&self) -> impl Iterator<Item = &OffMeshLink> {
        self.offmesh.auto.iter().chain(&self.offmesh.authored)
    }

    /// The span a link end names, or `None` when its column no longer has that layer.
    pub(super) fn link_span(&self, e: LinkEnd) -> Option<SpanRef> {
        let range = self.span_range(e.gx, e.gz);
        let index = range.start + e.layer as usize;
        (index < range.end).then_some(SpanRef {
            gx: e.gx,
            gz: e.gz,
            index: index as u32,
        })
    }

    /// The link end at span `s`.
    pub(super) fn link_end(&self, s: SpanRef) -> LinkEnd {
        let start = self.span_range(s.gx, s.gz).start as u32;
        LinkEnd {
            gx: s.gx,
            gz: s.gz,
            layer: s.index - start,
        }
    }

    /// Rebuild the exit index after either link list changed.
    pub(super) fn index_links(&mut self) {
        let links = &self.offmesh;
        let mut exits = Vec::new();
        for (id, l) in links.auto.iter().chain(&links.authored).enumerate() {
            let cell = |e: LinkEnd| self.index(e.gx, e.gz) as u32;
            exits.push((cell(l.from), l.from.layer, id as u32, true));
            if l.bidirectional {
                exits.push((cell(l.to), l.to.layer, id as u32, false));
            }
        }
        exits.sort_unstable();
        self.offmesh.exits = exits;
    }

    /// The link moves out of span `here`, in a fixed order. A move's cost is the
    /// link's (its `cost_override`, else its length times its area's cost, #460) in
    /// A\*'s units, never below the octile distance it covers, so the search
    /// heuristic stays admissible.
    pub(super) fn link_moves(&self, here: SpanRef) -> impl Iterator<Item = LinkMove> + '_ {
        let key = (
            self.index(here.gx, here.gz) as u32,
            self.link_end(here).layer,
        );
        let exits = &self.offmesh.exits;
        let first = exits.partition_point(|e| (e.0, e.1) < key);
        exits[first..]
            .iter()
            .take_while(move |e| (e.0, e.1) == key)
            .filter_map(move |&(_, _, id, forward)| {
                let link = self.offmesh.get(id);
                let far = if forward { link.to } else { link.from };
                let to = self.link_span(far)?;
                let cost = self.link_cost(link, here, to);
                Some(LinkMove {
                    to,
                    cost,
                    area: link.area,
                    data: link.data(forward),
                })
            })
    }

    /// The cheapest link from `a` landing on `b`, as crossed: what a path step that
    /// is not a walk went through.
    pub(super) fn link_between(&self, a: SpanRef, b: SpanRef) -> Option<OffMeshLinkData> {
        let mut best: Option<LinkMove> = None;
        for m in self.link_moves(a).filter(|m| m.to == b) {
            if best.as_ref().is_none_or(|bm| m.cost < bm.cost) {
                best = Some(m);
            }
        }
        best.map(|m| m.data)
    }

    fn link_cost(&self, link: &OffMeshLink, from: SpanRef, to: SpanRef) -> f32 {
        let s = self.grid_spacing;
        let (a, b) = (self.span_world(from), self.span_world(to));
        let cost = if link.cost_override >= 0.0 {
            link.cost_override / s
        } else {
            let flat = Vec3::new(b.x - a.x, 0.0, b.z - a.z).length();
            (flat / s + (b.y - a.y).abs()) * self.area_cost(link.area)
        };
        cost.max(super::astar::heuristic(from, to))
    }
}
