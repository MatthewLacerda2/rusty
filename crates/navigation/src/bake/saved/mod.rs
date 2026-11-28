//! A refused file changes nothing; else tiles whose fingerprint matches are installed and the rest
//! rebaked. Authored links are re-snapped, not saved.
//!
//! stored-format: the `<scene>.navmesh` bytes and their fingerprint

mod bytes;
mod file;
mod fingerprint;
#[cfg(test)]
mod tests;

use super::super::{LinkEnd, NavigationGraph, OffMeshLink, TileKey};
use super::region::CellRect;
use super::BakeState;
use crate::offmesh::sort_generated;
use file::{Layer, Saved, SavedTile, Shape};
use rusty_scene::Scene;

pub fn baked_fingerprint(bytes: &[u8]) -> Option<u64> {
    file::decode(bytes).ok().map(|saved| saved.fingerprint)
}

/// A test hook: the offset of a saved navmesh's first baked span.
#[doc(hidden)]
pub fn first_baked_span(bytes: &[u8]) -> Option<usize> {
    file::first_baked_span(bytes)
}

impl NavigationGraph {
    pub fn bake_to_bytes(&mut self, scene: &Scene) -> Vec<u8> {
        self.bake(scene);
        let st = self.bake_state.as_ref().expect("a bake keeps its state");
        let (_, prints) = fingerprint::read(scene, &st.raw);
        let side = self.tiles.side;
        let origin = self.tiles.origin;
        let tiles = prints.tiles.iter().map(|(&key, &fingerprint)| {
            let home = |e: LinkEnd| (e.gx + origin.0, e.gz + origin.1);
            let leaving = |l: &&OffMeshLink| {
                let (x, z) = home(l.from);
                (x.div_euclid(side), z.div_euclid(side)) == key
            };
            let links = self.offmesh.auto.iter().filter(leaving);
            SavedTile {
                key,
                fingerprint,
                baked: self.layer(key),
                raw: st.raw.layer(key),
                links: links.map(|l| shifted(l, origin)).collect(),
            }
        });
        file::encode(&Saved {
            fingerprint: prints.whole,
            shape: self.shape(),
            tiles: tiles.collect(),
        })
    }

    /// How many tiles were rebaked.
    pub fn restore(&mut self, scene: &Scene, bytes: &[u8]) -> Result<usize, String> {
        let saved = file::decode(bytes)?;
        let (b, spacing, side) = self.target_shape(scene);
        let mut g = Self::empty(b.min_x, b.max_x, b.min_z, b.max_z, spacing, side);
        if (saved.shape.spacing, saved.shape.side) != (spacing, side as u32) {
            return Err("baked on another grid".into());
        }
        g.adopt_settings(&scene.nav_settings);
        let mut raw = g.empty_like();
        let (inputs, prints) = fingerprint::read(scene, &raw);
        let (stale, mut links) = g.install_current(&mut raw, &saved, &prints.tiles)?;
        if stale.len() == prints.tiles.len() && !stale.is_empty() {
            return Err("the scene changed since the bake".into());
        }
        sort_generated(&mut links);
        g.offmesh.auto = links;
        let mut state = BakeState {
            raw,
            inputs,
            log: Default::default(),
        };
        let rects: Vec<CellRect> = stale.iter().filter_map(|&k| g.tiles.rect(k)).collect();
        g.rebake_regions(scene, &mut state, rects);
        let named = |e: LinkEnd| e.layer < g.spans_at(e.gx, e.gz).len() as u32;
        if !g.offmesh.auto.iter().all(|l| named(l.from) && named(l.to)) {
            return Err("a link out of range".into());
        }
        g.offmesh.authored = g.resolve_authored(&state.inputs.link_keys());
        g.index_links();
        state.log = self.bake_state.take().map(|s| s.log).unwrap_or_default();
        g.bake_generation = self.bake_generation.wrapping_add(1);
        g.static_watch = self.static_watch.take();
        g.tracked_scene = self.tracked_scene.clone();
        g.requests = std::mem::take(&mut self.requests);
        state.log.record(g.bake_generation, None);
        g.bake_state = Some(Box::new(state));
        *self = g;
        Ok(stale.len())
    }

    /// The saved tiles whose fingerprint matches, installed; the keys of the rest, and the
    /// installed tiles' links.
    fn install_current(
        &mut self,
        raw: &mut Self,
        saved: &Saved,
        prints: &std::collections::BTreeMap<TileKey, u64>,
    ) -> Result<(Vec<TileKey>, Vec<OffMeshLink>), String> {
        let mut stale = Vec::new();
        let mut kept = saved.tiles.iter().peekable();
        let mut links = Vec::new();
        for (&key, &print) in prints {
            while kept.next_if(|t| t.key < key).is_some() {}
            match kept.next_if(|t| t.key == key && t.fingerprint == print) {
                Some(t) => {
                    self.install(key, &t.baked)?;
                    raw.install(key, &t.raw)?;
                    let back = (-self.tiles.origin.0, -self.tiles.origin.1);
                    links.extend(t.links.iter().map(|l| shifted(l, back)));
                }
                None => stale.push(key),
            }
        }
        Ok((stale, links))
    }

    fn layer(&self, key: TileKey) -> Layer {
        let Some(t) = self.tiles.tile(key) else {
            return Layer::default();
        };
        let ids = t.base as usize..(t.base + t.len()) as usize;
        Layer {
            cell_start: t.cell_start.clone(),
            spans: self.spans[ids].to_vec(),
        }
    }

    fn install(&mut self, key: TileKey, layer: &Layer) -> Result<(), String> {
        if layer.spans.is_empty() {
            return Ok(());
        }
        let rect = self.tiles.rect(key).ok_or("a tile outside the bounds")?;
        let side = self.tiles.side;
        let (x0, z0) = (
            key.0 * side - self.tiles.origin.0,
            key.1 * side - self.tiles.origin.1,
        );
        let starts = &layer.cell_start;
        let outside = (0..(side * side) as usize).any(|l| {
            let (gx, gz) = (x0 + l as i32 % side, z0 + l as i32 / side);
            !rect.contains(gx, gz) && starts[l] != starts[l + 1]
        });
        if outside {
            return Err("a tile holds spans outside the bounds".into());
        }
        self.tiles
            .write(key, starts.clone(), &layer.spans, &mut self.spans);
        Ok(())
    }

    fn shape(&self) -> Shape {
        Shape {
            bounds: [self.min_x, self.max_x, self.min_z, self.max_z],
            spacing: self.grid_spacing,
            side: self.tiles.side as u32,
        }
    }
}

fn shifted(l: &OffMeshLink, by: (i32, i32)) -> OffMeshLink {
    let mut l = l.clone();
    for e in [&mut l.from, &mut l.to] {
        (e.gx, e.gz) = (e.gx + by.0, e.gz + by.1);
    }
    l
}
