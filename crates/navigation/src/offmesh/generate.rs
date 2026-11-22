//! A **drop** lands on a floor more than `max_step` and at most `drop_height` lower; a **jump** up to `jump_height` higher, or across a gap of at most `jump_distance`. A level jump needs a real gap (a column with no floor at the ledge's height), so floor erosion merely trimmed is never jumped.
//! Links are thinned to one per `link_spacing` run in grid-fixed buckets, so an incremental rebake regenerates whole buckets and matches a full bake.

use super::super::bake::CellRect;
use super::super::{NavMeshSettings, NavigationGraph, SpanRef};
use super::{OffMeshLink, OffMeshLinkKind};

const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinkParams {
    drop_height: f32,
    jump_distance: f32,
    jump_height: f32,
    edge: i32,
    pub reach: i32,
    stride: i32,
}

impl LinkParams {
    pub fn new(s: &NavMeshSettings, spacing: f32) -> Option<Self> {
        let on = |v: f32| v > 0.0;
        if !(on(s.drop_height) || on(s.jump_distance) || on(s.jump_height)) {
            return None;
        }
        let cells = |v: f32| {
            if on(v) {
                (v / spacing).ceil() as i32
            } else {
                0
            }
        };
        let edge = 2 * cells(s.agent_radius) + 2;
        Some(Self {
            drop_height: s.drop_height.max(0.0),
            jump_distance: s.jump_distance.max(0.0),
            jump_height: s.jump_height.max(0.0),
            edge,
            reach: edge + cells(s.jump_distance),
            stride: ((s.link_spacing / spacing).round() as i32).max(1),
        })
    }

    pub fn region(&self, changed: CellRect) -> CellRect {
        let r = changed.grown(self.reach + 1);
        let down = |v: i32| v.div_euclid(self.stride) * self.stride;
        let up = |v: i32| down(v) + self.stride - 1;
        CellRect {
            x0: down(r.x0),
            z0: down(r.z0),
            x1: up(r.x1),
            z1: up(r.z1),
        }
    }
}

impl NavigationGraph {
    pub(in super::super) fn generate_links(
        &self,
        raw: &NavigationGraph,
        p: &LinkParams,
        region: CellRect,
    ) -> Vec<OffMeshLink> {
        let mut kept = std::collections::BTreeSet::new();
        let mut out = Vec::new();
        for gz in region.z0..=region.z1 {
            for gx in region.x0..=region.x1 {
                for index in self.span_range(gx, gz) {
                    let s = SpanRef {
                        gx,
                        gz,
                        index: index as u32,
                    };
                    let layer = self.link_end(s).layer;
                    for (d, &(dx, dz)) in DIRS.iter().enumerate() {
                        let Some(link) = self.ledge_link(raw, p, s, dx, dz) else {
                            continue;
                        };
                        let (line, along) = if dx != 0 { (gx, gz) } else { (gz, gx) };
                        if kept.insert((d, line, along.div_euclid(p.stride), layer)) {
                            out.push(link);
                        }
                    }
                }
            }
        }
        sort_generated(&mut out);
        out
    }

    fn ledge_link(
        &self,
        raw: &NavigationGraph,
        p: &LinkParams,
        s: SpanRef,
        dx: i32,
        dz: i32,
    ) -> Option<OffMeshLink> {
        self.link_to(s, s.gx + dx, s.gz + dz)
            .is_none()
            .then_some(())?;
        let a = self.spans[s.index as usize];
        let top = a.y + self.max_step.max(p.jump_height);
        let mut gap = false;
        for k in 1..=p.reach {
            let (cx, cz) = (s.gx + k * dx, s.gz + k * dz);
            if !self.in_bounds(cx, cz) {
                return None;
            }
            for i in self.span_range(cx, cz).rev() {
                let b = self.spans[i];
                let Some(kind) = p.classify(k, b.y - a.y, gap, self.max_step) else {
                    continue;
                };
                let high = a.y.max(b.y);
                let room = high + self.agent_height;
                let clear = |j: i32| raw.open_at(s.gx + j * dx, s.gz + j * dz, high, self);
                if a.ceiling < room || b.ceiling < room || !(1..k).all(clear) {
                    continue;
                }
                let t = SpanRef {
                    gx: cx,
                    gz: cz,
                    index: i as u32,
                };
                return Some(OffMeshLink {
                    kind,
                    start: self.span_world(s),
                    end: self.span_world(t),
                    from: self.link_end(s),
                    to: self.link_end(t),
                    bidirectional: false,
                    cost_override: -1.0,
                    area: super::super::WALKABLE_AREA,
                    owner: None,
                });
            }
            let level = |y: f32| (y - a.y).abs() <= self.max_step;
            if !gap && self.spans_at(cx, cz).iter().any(|b| level(b.y)) {
                return None; // floor carries on past an eroded strip
            }
            if !raw.open_at(cx, cz, top, self) {
                return None; // nothing past this column can be reached
            }
            gap |= !raw.spans_at(cx, cz).iter().any(|u| level(u.y));
        }
        None
    }

    fn open_at(&self, gx: i32, gz: i32, y: f32, g: &NavigationGraph) -> bool {
        let spans = self.spans_at(gx, gz);
        let (lo, hi) = (y + g.max_step, y + g.agent_height);
        spans.is_empty() || spans.iter().any(|u| u.y <= lo && u.ceiling >= hi)
    }
}

pub fn sort_generated(links: &mut [OffMeshLink]) {
    links.sort_by_key(|l| (l.from.gz, l.from.gx, l.from.layer, l.to.gz, l.to.gx));
}

impl LinkParams {
    fn classify(&self, k: i32, dy: f32, gap: bool, max_step: f32) -> Option<OffMeshLinkKind> {
        let near = k <= self.edge;
        if dy < -max_step {
            if -dy > self.drop_height {
                None
            } else if near {
                Some(OffMeshLinkKind::Drop)
            } else {
                Some(OffMeshLinkKind::Jump)
            }
        } else if dy > max_step {
            (dy <= self.jump_height).then_some(OffMeshLinkKind::Jump)
        } else {
            (gap && self.jump_distance > 0.0).then_some(OffMeshLinkKind::Jump)
        }
    }
}
