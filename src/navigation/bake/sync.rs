//! src/navigation/bake/sync.rs — rebake only what changed (#456).
//!
//! [`NavigationGraph::sync`] runs every play tick in place of the old full rebake
//! every 60 frames. It diffs the scene against the inputs the last bake read
//! (`inputs`): nothing changed costs one pass over the static colliders and the
//! obstacles. A changed collider or obstacle dirties the cells its old and new
//! footprints cover; overlapping dirty rectangles merge. Each rectangle's
//! pre-erosion columns are rebuilt and spliced in, then erosion re-runs over the
//! rectangles grown by its reach, and those cells of the final graph are spliced.
//!
//! A settings change or a change of the grid's bounds or spacing is a full bake:
//! every cell moves or every span may differ. Connectivity is computed on demand
//! from the spans (`links.rs`), so there are no stored components or links to
//! recompute, locally or globally.

use super::super::NavigationGraph;
use super::erosion;
use super::inputs::{collider_keys, diff, obstacle_keys};
use super::raster;
use super::region::{merge_overlapping, CellRect};
use super::{BakeState, ObstacleVolume};
use crate::physics::collider_world_triangles;
use crate::scene::Scene;

/// What a [`NavigationGraph::sync`] did.
#[derive(Clone, Debug, PartialEq)]
pub enum Rebake {
    /// Nothing the bake reads changed.
    Unchanged,
    /// These cell rectangles were rebaked (erosion reach included).
    Incremental(Vec<CellRect>),
    /// The whole grid was rebaked.
    Full,
}

impl NavigationGraph {
    /// Bring the navmesh up to date with `scene`, rebaking only the cells whose
    /// inputs changed since the last bake. A graph that was never baked, or whose
    /// settings, bounds or spacing changed, is baked in full. The result is the
    /// same graph a full [`Self::bake`] of `scene` gives.
    pub fn sync(&mut self, scene: &Scene) -> Rebake {
        let current = self.bake_state.as_ref().is_some_and(|st| {
            st.inputs.settings == scene.nav_settings && !self.needs_reshape(scene)
        });
        let Some(mut st) = self.bake_state.take().filter(|_| current) else {
            self.bake(scene);
            return Rebake::Full;
        };
        let dirty = st.update_inputs(scene);
        if dirty.is_empty() {
            self.bake_state = Some(st);
            return Rebake::Unchanged;
        }

        let regions = merge_overlapping(dirty);
        for &r in &regions {
            let ids = st.inputs.collider_ids_in(r);
            let volumes = st.inputs.obstacles_in(r);
            st.raw.rebuild_raw(scene, &ids, &volumes, r);
        }
        let radius = scene.nav_settings.agent_radius;
        let reach = erosion::reach(radius, self.grid_spacing);
        let grown = regions.iter().filter_map(|r| r.grown(reach).clamped(self));
        let changed = merge_overlapping(grown.collect());
        for &r in &changed {
            let cols = st.raw.eroded_columns(radius, r);
            self.splice(r, &cols);
        }
        self.bake_generation = self.bake_generation.wrapping_add(1);
        st.log.record(self.bake_generation, Some(changed.clone()));
        self.bake_state = Some(st);
        Rebake::Incremental(changed)
    }
}

impl BakeState {
    /// Diff the recorded inputs against `scene`, record the current ones, and
    /// return the cells whose inputs changed.
    fn update_inputs(&mut self, scene: &Scene) -> Vec<CellRect> {
        let mut dirty = Vec::new();
        let raw = &self.raw;
        let collider_rect = |id: u32, _: &_| {
            let mesh = collider_world_triangles(scene, id)?;
            raster::footprint(raw, &mesh.vertices)
        };
        let colliders = diff(
            &self.inputs.colliders,
            collider_keys(scene),
            collider_rect,
            &mut dirty,
        );
        let obstacle_rect =
            |_: u32, v: &ObstacleVolume| raster::footprint(raw, &v.triangles().concat());
        let obstacles = diff(
            &self.inputs.obstacles,
            obstacle_keys(scene),
            obstacle_rect,
            &mut dirty,
        );
        self.inputs.colliders = colliders;
        self.inputs.obstacles = obstacles;
        dirty
    }
}
