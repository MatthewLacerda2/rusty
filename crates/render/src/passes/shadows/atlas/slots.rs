//! Where each shadowed light's tiles live, frame after frame (#1711). Pure CPU.
//!
//! A light keeps its tiles, keyed by its entity id, for as long as it keeps asking
//! for a shadow, as HDRP's cached shadow atlas keeps a light's placement and
//! Unreal caches a movable light's static depth until the light moves. So a tile's
//! static bake ([`super::cache`]) survives what used to re-plan the whole atlas:
//!
//! - **A light that stops being a candidate keeps its tiles** for [`KEEP_FRAMES`]
//!   rendered frames: one at intensity 0 (an alarm blinking), out of view, no
//!   longer casting, or cut by the camera's light budget (#873). When it comes
//!   back, its tiles and their bakes are still there. A despawned light frees its
//!   tiles at once, and a new scene starts the slots over.
//! - **Hysteresis on size** ([`super::hysteresis`]): a light's settled size changes
//!   only after its wanted size has been out of band for some frames in a row, so
//!   a camera walking across a step re-bakes nothing. The budget may halve a
//!   settled size under pressure; the tiles grow back when the pressure ends.
//!
//! Each frame the candidates are first **budgeted** in importance order by cell
//! count alone: a light that does not fit is retried at half the size, down to
//! [`MIN_TILE`], then **dropped** and counted in
//! `RenderCounters::shadow_lights_dropped`, never silently. Only then are tiles
//! **placed**: a light that has its tiles at its budgeted size keeps them, a new or
//! resized one takes free cells ([`CellGrid`]), evicting idle lights' tiles, the
//! longest idle first, when it has to. When fragmentation still refuses it, the
//! frame's lights are **repacked** once, largest first (powers of two packed in
//! Z-order from the largest down leave no gap, so the budget guarantees they all
//! fit): the one case a shown light's tiles move. Frames are counted, never timed,
//! so a replay plans the same atlas.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};

use super::alloc::{cells, CellGrid};
use super::hysteresis::settle;
use super::plan::{
    candidates, faces, light_views, texel_at_one_metre, AtlasPlan, Candidate, Tile, CELLS, MIN_TILE,
};
use crate::clusters::LocalLight;
use rusty_scene::SceneId;

/// Rendered frames an idle light keeps its tiles: ten seconds at 60 fps.
pub(super) const KEEP_FRAMES: u64 = 600;

/// One frame's lights, as the plan reads them.
pub(crate) struct Frame<'a> {
    /// The frame's local lights, each with whether it asks for a shadow.
    pub lights: &'a [(LocalLight, bool)],
    /// Each light's entity id, in the same order.
    pub ids: &'a [u32],
    pub view_proj: Mat4,
    pub eye: Vec3,
}

/// One light's tiles.
#[derive(Debug)]
struct Slot {
    /// Each face's top-left corner, in texels.
    origins: Vec<[u32; 2]>,
    size: u32,
    /// The size the hysteresis settled on, before the budget halved it.
    settled: u32,
    /// The last frame the light was shown.
    seen: u64,
    /// Frames in a row its wanted size was out of the settled size's band.
    off_band: u32,
}

/// Every light's tiles, and the cells they hold.
#[derive(Debug, Default)]
pub(crate) struct ShadowSlots {
    scene: Option<SceneId>,
    frame: u64,
    grid: CellGrid,
    slots: BTreeMap<u32, Slot>,
}

/// A light the budget kept.
#[derive(Clone, Copy)]
struct Kept {
    /// Its index in the frame's lights.
    light: usize,
    id: u32,
    /// Its tile size this frame.
    size: u32,
    /// The size it settled at, before the budget halved it.
    settled: u32,
}

impl ShadowSlots {
    /// Plan this frame's atlas for `frame`'s lights in `scene`; `alive` says whether
    /// an entity id still exists, so a despawned light's tiles are freed at once.
    pub(crate) fn plan(
        &mut self,
        scene: Option<SceneId>,
        frame: &Frame,
        alive: impl Fn(u32) -> bool,
    ) -> AtlasPlan {
        if scene != self.scene {
            *self = Self {
                scene,
                ..Self::default()
            };
        }
        self.frame += 1;
        let now = self.frame;
        let gone: Vec<u32> = (self.slots.iter())
            .filter(|(&id, s)| !alive(id) || now - s.seen > KEEP_FRAMES)
            .map(|(&id, _)| id)
            .collect();
        gone.into_iter().for_each(|id| self.free(id));

        let ranked = candidates(frame.lights, frame.view_proj, frame.eye);
        let (kept, dropped) = self.budget(frame, &ranked);
        for k in &kept {
            let faces = faces(&frame.lights[k.light].0) as usize;
            match self.slots.get_mut(&k.id) {
                Some(s) if s.size == k.size && s.origins.len() == faces => {
                    if s.settled != k.settled {
                        s.off_band = 0;
                    }
                    (s.seen, s.settled) = (now, k.settled);
                }
                Some(_) => self.free(k.id),
                None => {}
            }
        }
        dropped.iter().for_each(|&id| self.free(id));
        for k in &kept {
            let faces = faces(&frame.lights[k.light].0);
            if !self.slots.contains_key(&k.id) && !self.place_evicting(k, faces, now) {
                self.repack(frame, &kept, now);
                break;
            }
        }
        self.output(frame, &kept, dropped.len() as u32)
    }

    /// The candidates the atlas has room for, by cell count, the most important
    /// first, each at its settled size or smaller; and the ids of those dropped.
    fn budget(&mut self, frame: &Frame, ranked: &[Candidate]) -> (Vec<Kept>, Vec<u32>) {
        let (mut kept, mut dropped) = (Vec::new(), Vec::new());
        let mut free = CELLS * CELLS;
        for c in ranked {
            let id = frame.ids[c.light];
            let faces = faces(&frame.lights[c.light].0);
            let settled = self.settle(id, faces, c);
            let mut size = settled;
            while size > MIN_TILE && faces * cells(size) > free {
                size /= 2;
            }
            if faces * cells(size) > free {
                dropped.push(id);
                continue;
            }
            free -= faces * cells(size);
            kept.push(Kept {
                light: c.light,
                id,
                size,
                settled,
            });
        }
        (kept, dropped)
    }

    /// The size light `id` (with `faces` tiles) asks of the budget this frame: its
    /// settled size, or what its coverage asks for once the hysteresis lets it go
    /// ([`settle`]). A light not shown last frame starts counting over.
    fn settle(&mut self, id: u32, faces: u32, c: &Candidate) -> u32 {
        let now = self.frame;
        let Some(slot) = self.slots.get_mut(&id) else {
            return c.size;
        };
        if slot.origins.len() as u32 != faces {
            return c.size;
        }
        if slot.seen + 1 != now {
            slot.off_band = 0;
        }
        settle(slot.settled, &mut slot.off_band, c.want, c.size)
    }

    /// Give light `k` `faces` free tiles of its size, evicting idle lights' tiles
    /// (the longest idle first) until they fit. False when even that fails.
    fn place_evicting(&mut self, k: &Kept, faces: u32, now: u64) -> bool {
        loop {
            if let Some(origins) = self.try_place(k.size, faces) {
                self.slots.insert(k.id, slot(origins, k, now));
                return true;
            }
            let idle = (self.slots.iter())
                .filter(|(_, s)| s.seen < now)
                .min_by_key(|(&id, s)| (s.seen, id))
                .map(|(&id, _)| id);
            match idle {
                Some(victim) => self.free(victim),
                None => return false,
            }
        }
    }

    /// `faces` tiles of `size` from the free cells, or none of them.
    fn try_place(&mut self, size: u32, faces: u32) -> Option<Vec<[u32; 2]>> {
        let mut origins = Vec::with_capacity(faces as usize);
        for _ in 0..faces {
            match self.grid.place(size) {
                Some(origin) => origins.push(origin),
                None => {
                    origins.iter().for_each(|&o| self.grid.free(o, size));
                    return None;
                }
            }
        }
        Some(origins)
    }

    /// Place every `kept` light again from an empty grid, largest tiles first. Called
    /// once idle lights are all evicted, so `kept` are the only slots.
    fn repack(&mut self, frame: &Frame, kept: &[Kept], now: u64) {
        let mut order: Vec<&Kept> = kept.iter().collect();
        order.sort_by_key(|k| std::cmp::Reverse(k.size));
        let old = std::mem::take(&mut self.slots);
        self.grid = CellGrid::default();
        for k in order {
            let faces = faces(&frame.lights[k.light].0);
            let origins = (self.try_place(k.size, faces))
                .expect("the budget leaves room for every kept light, packed largest first");
            let off_band = (old.get(&k.id))
                .filter(|s| s.settled == k.settled)
                .map_or(0, |s| s.off_band);
            let s = Slot {
                off_band,
                ..slot(origins, k, now)
            };
            self.slots.insert(k.id, s);
        }
    }

    /// The frame's tiles, in `kept`'s order, each light's faces in turn.
    fn output(&self, frame: &Frame, kept: &[Kept], dropped: u32) -> AtlasPlan {
        let mut out = AtlasPlan {
            dropped,
            resizing: kept.iter().any(|k| self.slots[&k.id].off_band > 0),
            ..AtlasPlan::default()
        };
        for k in kept {
            let (light, id, size) = (k.light, k.id, k.size);
            let slot = &self.slots[&id];
            let l = &frame.lights[light].0;
            out.shadows.push((light, out.tiles.len() as u32));
            let texel = texel_at_one_metre(l, size);
            for (view_proj, &origin) in light_views(l).into_iter().zip(&slot.origins) {
                out.tiles.push(Tile {
                    view_proj,
                    origin,
                    size,
                    texel,
                    light: Vec3::from(l.position),
                    statics: true,
                });
            }
        }
        out
    }

    /// Free light `id`'s tiles, if it has any.
    fn free(&mut self, id: u32) {
        if let Some(slot) = self.slots.remove(&id) {
            for origin in slot.origins {
                self.grid.free(origin, slot.size);
            }
        }
    }

    /// Cells the grid holds, and cells the slots say they hold: equal unless a tile
    /// leaked.
    #[cfg(test)]
    pub(super) fn held_cells(&self) -> (u32, u32) {
        let by_slots = (self.slots.values())
            .map(|s| s.origins.len() as u32 * cells(s.size))
            .sum();
        (CELLS * CELLS - self.grid.free_cells(), by_slots)
    }
}

/// A fresh slot over `origins` for light `k`, shown at frame `now`.
fn slot(origins: Vec<[u32; 2]>, k: &Kept, now: u64) -> Slot {
    Slot {
        origins,
        size: k.size,
        settled: k.settled,
        seen: now,
        off_band: 0,
    }
}

#[cfg(test)]
#[path = "slots_tests.rs"]
mod tests;
