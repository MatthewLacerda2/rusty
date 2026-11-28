use glam::Vec3;

use super::inputs::{collider_keys, obstacle_keys, Source};
use super::modifiers::modifier_keys;
use super::still::StaticWatch;
use crate::offmesh::authored_keys;
use crate::NavigationGraph;
use rusty_scene::Scene;

impl NavigationGraph {
    pub(crate) fn shift_baked(&mut self, scene: &mut Scene, by: Vec3) {
        if let Some(st) = self.bake_state.as_mut() {
            st.raw.translate(by);
            let inputs = &mut st.inputs;
            rekey(&mut inputs.colliders, collider_keys(scene));
            rekey(&mut inputs.obstacles, obstacle_keys(scene));
            rekey(&mut inputs.modifiers, modifier_keys(scene));
            rekey(&mut inputs.links, authored_keys(scene));
        }
        // It moved every static: the watch starts after it.
        self.static_watch = Some(StaticWatch::open(&mut scene.world));
    }
}

/// Whole tiles: each input covers the same cells. A set that differs is left for the next
/// sync to rebake.
fn rekey<K>(old: &mut [Source<K>], fresh: Vec<(u32, K)>) {
    let same_ids = old.len() == fresh.len() && old.iter().zip(&fresh).all(|(s, f)| s.id == f.0);
    if same_ids {
        for (s, (_, key)) in old.iter_mut().zip(fresh) {
            s.key = key;
        }
    }
}
