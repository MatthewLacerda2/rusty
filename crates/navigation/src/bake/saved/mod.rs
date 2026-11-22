//! A refused file (unreadable, stale, another grid) changes nothing: the caller bakes in full. The authored links are not saved; they are re-snapped onto the restored spans.
//!
//! stored-format: the `<scene>.navmesh` bytes and their fingerprint

mod file;
mod fingerprint;
#[cfg(test)]
mod tests;

use super::super::NavigationGraph;
use super::BakeState;
use file::{Layer, Saved, Shape};
use rusty_scene::Scene;

pub fn baked_fingerprint(bytes: &[u8]) -> Option<u64> {
    file::decode(bytes).ok().map(|saved| saved.fingerprint)
}

impl NavigationGraph {
    pub fn bake_to_bytes(&mut self, scene: &Scene) -> Vec<u8> {
        self.bake(scene);
        let st = self.bake_state.as_ref().expect("a bake keeps its state");
        let (_, fingerprint) = fingerprint::read(scene, &st.raw);
        file::encode(&Saved {
            fingerprint,
            shape: self.shape(),
            baked: Layer {
                cell_start: self.cell_start.clone(),
                spans: self.spans.clone(),
            },
            raw: Layer {
                cell_start: st.raw.cell_start.clone(),
                spans: st.raw.spans.clone(),
            },
            links: self.offmesh.auto.clone(),
        })
    }

    pub fn restore(&mut self, scene: &Scene, bytes: &[u8]) -> Result<(), String> {
        let saved = file::decode(bytes)?;
        let (b, spacing) = self.target_shape(scene);
        let mut g = Self::new(b.min_x, b.max_x, b.min_z, b.max_z, spacing);
        if g.shape() != saved.shape {
            return Err("baked on another grid".into());
        }
        g.adopt_settings(&scene.nav_settings);
        let mut raw = g.empty_like();
        let (inputs, fingerprint) = fingerprint::read(scene, &raw);
        if fingerprint != saved.fingerprint {
            return Err("the scene changed since the bake".into());
        }
        (raw.cell_start, raw.spans) = (saved.raw.cell_start, saved.raw.spans);
        (g.cell_start, g.spans) = (saved.baked.cell_start, saved.baked.spans);
        g.offmesh.auto = saved.links;
        g.offmesh.authored = g.resolve_authored(&inputs.link_keys());
        g.index_links();
        let log = self.bake_state.take().map(|s| s.log).unwrap_or_default();
        g.bake_generation = self.bake_generation.wrapping_add(1);
        g.static_watch = self.static_watch.take();
        g.tracked_scene = self.tracked_scene.clone();
        g.requests = std::mem::take(&mut self.requests);
        let mut state = BakeState { raw, inputs, log };
        state.log.record(g.bake_generation, None);
        g.bake_state = Some(Box::new(state));
        *self = g;
        Ok(())
    }

    fn shape(&self) -> Shape {
        Shape {
            bounds: [self.min_x, self.max_x, self.min_z, self.max_z],
            spacing: self.grid_spacing,
            width: self.width as u32,
            height: self.height as u32,
        }
    }
}
