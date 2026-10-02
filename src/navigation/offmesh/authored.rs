//! Authored links (#462): `OffMeshLink` components resolved onto the navmesh.
//!
//! A link is read as an [`AuthoredKey`] — its world ends and options — so the
//! rebake's per-tick diff sees a moved, edited or toggled link like any other input.
//! Each end snaps to the walkable point nearest it within [`LINK_SNAP_DISTANCE`]
//! (Unity connects a link end to the navmesh near it); a link with an end that
//! finds none connects nothing until the navmesh or the link moves.

use glam::Vec3;

use super::super::{NavigationGraph, SpanRef};
use super::{OffMeshLink, OffMeshLinkKind};
use crate::components::LINK_SNAP_DISTANCE;
use crate::scene::Scene;

/// An authored link as the bake reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AuthoredKey {
    pub start: Vec3,
    pub end: Vec3,
    pub bidirectional: bool,
    pub cost_override: f32,
}

/// Every active link on an active entity, with its key, by ascending id.
pub fn authored_keys(scene: &Scene) -> Vec<(u32, AuthoredKey)> {
    let mut out: Vec<_> = scene
        .world
        .ids_with_offmesh_link()
        .into_iter()
        .filter(|&id| scene.world.is_active(id))
        .filter_map(|id| {
            let link = scene.world.offmesh_link(id)?.clone();
            link.active.then_some(())?;
            let (start, end) = link.world_ends(scene.compute_world_matrix(id));
            let key = AuthoredKey {
                start,
                end,
                bidirectional: link.bidirectional,
                cost_override: link.cost_override,
            };
            Some((id, key))
        })
        .collect();
    out.sort_by_key(|&(id, _)| id);
    out
}

impl NavigationGraph {
    /// The links `keys` make on this navmesh: those whose both ends snap onto it.
    pub(in super::super) fn resolve_authored(
        &self,
        keys: &[(u32, AuthoredKey)],
    ) -> Vec<OffMeshLink> {
        keys.iter()
            .filter_map(|&(id, k)| {
                let (start, from) = self.snap_link_end(k.start)?;
                let (end, to) = self.snap_link_end(k.end)?;
                Some(OffMeshLink {
                    kind: OffMeshLinkKind::Manual,
                    start,
                    end,
                    from: self.link_end(from),
                    to: self.link_end(to),
                    bidirectional: k.bidirectional,
                    cost_override: k.cost_override,
                    owner: Some(id),
                })
            })
            .collect()
    }

    /// Whether entity `id`'s link connects two walkable points right now.
    pub fn link_connected(&self, id: u32) -> bool {
        self.offmesh.authored.iter().any(|l| l.owner == Some(id))
    }

    /// The walkable point a link end at `p` lands on, and its span.
    fn snap_link_end(&self, p: Vec3) -> Option<(Vec3, SpanRef)> {
        let at = self.sample_position(p, LINK_SNAP_DISTANCE)?;
        Some((at, self.span_under(at)?))
    }
}
