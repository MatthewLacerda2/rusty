use std::collections::BTreeMap;

use glam::{Mat4, Vec3};

use super::alloc::{cells, CellGrid};
use super::hysteresis::settle;
use super::plan::{
    candidates, faces, light_views, texel_at_one_metre, AtlasPlan, Candidate, Tile, CELLS, MIN_TILE,
};
use crate::clusters::LocalLight;
use rusty_scene::SceneId;

pub(super) const KEEP_FRAMES: u64 = 600;

pub(crate) struct Frame<'a> {
    pub lights: &'a [(LocalLight, bool)],
    pub ids: &'a [u32],
    pub view_proj: Mat4,
    pub eye: Vec3,
}

#[derive(Debug)]
struct Slot {
    origins: Vec<[u32; 2]>,
    size: u32,
    settled: u32,
    seen: u64,
    off_band: u32,
}

#[derive(Debug, Default)]
pub(crate) struct ShadowSlots {
    scene: Option<SceneId>,
    frame: u64,
    grid: CellGrid,
    slots: BTreeMap<u32, Slot>,
}

#[derive(Clone, Copy)]
struct Kept {
    light: usize,
    id: u32,
    size: u32,
    settled: u32,
}

impl ShadowSlots {
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

    fn free(&mut self, id: u32) {
        if let Some(slot) = self.slots.remove(&id) {
            for origin in slot.origins {
                self.grid.free(origin, slot.size);
            }
        }
    }

    #[cfg(test)]
    pub(super) fn held_cells(&self) -> (u32, u32) {
        let by_slots = (self.slots.values())
            .map(|s| s.origins.len() as u32 * cells(s.size))
            .sum();
        (CELLS * CELLS - self.grid.free_cells(), by_slots)
    }
}

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
