//! src/render/scene/occlusion.rs — the baked PVS as a camera reads it (#1714):
//! which retained objects are still what was baked, and whether the bake may be
//! used at all.
//!
//! Bound once per change, not per frame: re-checked against every baked object's
//! fingerprint (`scene::lighting::occlusion::Fingerprints`) when the scene's PVS was
//! replaced or a static object was re-derived or moved. Per camera it is one cell
//! lookup, and per object a map lookup and a bit test, with no allocation.
//!
//! The rules that keep it from ever hiding what a camera sees:
//! - only an object whose fingerprint is what was baked is culled: one moved,
//!   re-materialled, made dynamic or spawned since is drawn;
//! - the bake is used only while every baked occluder still matches: one moved,
//!   removed or deactivated may have uncovered anything, so nothing is culled until
//!   it is re-baked (said once in the log);
//! - a camera whose culling mask leaves out an occluder's layer sees past it, so it
//!   culls nothing by occlusion.

use std::collections::BTreeMap;
use std::sync::Arc;

use glam::Vec3;

use crate::scene::lighting::occlusion::{Fingerprints, OcclusionData};
use crate::scene::revision::Revision;
use crate::scene::Scene;

/// One scene's PVS, bound to its objects.
#[derive(Default)]
pub(crate) struct OcclusionBinding {
    data: Option<Arc<OcclusionData>>,
    /// The scene PVS's revision it was bound at; `None` until bound.
    seen: Option<Revision>,
    /// A static object changed since: re-bind on the next extract.
    stale: bool,
    /// Entity → its index in the PVS, for the objects unchanged since the bake.
    unchanged: BTreeMap<u32, u32>,
    /// Every baked occluder is unchanged.
    usable: bool,
    /// The layers the occluders sit on.
    layers: u32,
    /// The revision a stale bake was last reported at, so it is said once.
    reported: Option<Revision>,
}

impl OcclusionBinding {
    /// A static object came, went, changed or moved.
    pub(super) fn forget(&mut self) {
        self.stale = true;
    }

    /// Re-bind to `scene` if its PVS was replaced or a static object changed.
    pub(super) fn refresh(&mut self, scene: &Scene) {
        let revision = scene.occlusion.revision();
        if !self.stale && self.seen == Some(revision) {
            return;
        }
        (self.seen, self.stale) = (Some(revision), false);
        self.data = scene.occlusion.as_ref().map(Arc::clone);
        self.unchanged.clear();
        (self.usable, self.layers) = (false, 0);
        let Some(data) = self.data.as_deref() else {
            return;
        };
        let prints = Fingerprints::of_scene(scene);
        let (mut moved, mut occluders_moved) = (0, 0);
        for (i, baked) in data.objects.iter().enumerate() {
            match prints.of(scene, baked.entity) {
                Some((print, _)) if print == baked.fingerprint => {
                    self.unchanged.insert(baked.entity, i as u32);
                    if baked.occluder {
                        self.layers |= 1u32 << scene.world.layer(baked.entity).min(31);
                    }
                }
                _ => {
                    moved += 1;
                    occluders_moved += usize::from(baked.occluder);
                }
            }
        }
        self.usable = occluders_moved == 0;
        if moved > 0 && self.reported != Some(revision) {
            self.reported = Some(revision);
            log::warn!(
                "[Occlusion] the bake is older than the scene's statics: {moved} baked mesh(es) \
                 changed since; {}. Generate Lighting re-bakes it.",
                match self.usable {
                    true => "they are always drawn".to_string(),
                    false =>
                        format!("{occluders_moved} were occluders, so nothing is occlusion-culled"),
                }
            );
        }
    }

    /// The PVS row a camera at `eye` drawing the layers of `mask` reads, `None`
    /// when it culls nothing by occlusion: no usable bake, an occluder's layer left
    /// out of its mask, or an eye outside every cell.
    pub(crate) fn row(&self, eye: Vec3, mask: u32) -> Option<u32> {
        let data = self.data.as_deref().filter(|_| self.usable)?;
        (mask & self.layers == self.layers)
            .then(|| data.row_at(eye))
            .flatten()
    }

    /// The box of the cell holding `eye`, when the bake is usable there: what the
    /// editor's occlusion view outlines.
    pub(crate) fn cell_at(&self, eye: Vec3) -> Option<(Vec3, Vec3)> {
        let data = self.data.as_deref().filter(|_| self.usable)?;
        data.grid.index_of(eye).map(|cell| data.grid.bounds(cell))
    }

    /// Whether entity `id` is hidden from a cell whose row is `row`.
    pub(crate) fn hides(&self, row: u32, id: u32) -> bool {
        let Some(data) = self.data.as_deref() else {
            return false;
        };
        self.unchanged.get(&id).is_some_and(|&i| !data.sees(row, i))
    }
}
