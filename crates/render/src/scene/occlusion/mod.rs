//! crates/render/src/scene/occlusion/ — the baked PVS as a camera reads it (#1714):
//! which retained objects are still what was baked, and whether the bake may be
//! used at all.
//!
//! Bound once per change, not per frame. A whole bind checks every baked object's
//! fingerprint (`scene::lighting::occlusion::Fingerprints`): when the scene's PVS was
//! replaced, the objects were rebuilt, or a `LODGroup` changed which renderers it
//! holds. Otherwise only the static objects re-derived or moved since are re-checked
//! ([`OcclusionBinding::touch`]), so a script tweening one static door costs one
//! fingerprint a frame, not one per baked object. Geometry hashes are kept by mesh
//! revision across binds. Per camera it is one cell lookup, and per object a map
//! lookup and a bit test, with no allocation.
//!
//! The rules that keep it from ever hiding what a camera sees:
//! - only an object whose fingerprint is what was baked is culled: one moved,
//!   re-shaped, given a material that occludes differently, regrouped, made
//!   dynamic or spawned since is drawn;
//! - the bake is used only while every baked occluder still matches: one moved,
//!   removed or deactivated may have uncovered anything, so nothing is culled until
//!   it is re-baked (said once in the log);
//! - a camera whose culling mask leaves out an occluder's layer sees past it, so it
//!   culls nothing by occlusion.
//!
//! A point or spot light's shadow views read it too ([`OcclusionBinding::shadow_row`],
//! part 4): a baked caster its cell's row does not see is not drawn into its maps.
//! A wall hides what is behind it from a light only if the wall itself is drawn
//! into that light's maps, so a light also culls nothing while any matching
//! occluder casts no shadow (cast mode Off).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use glam::Vec3;

use rusty_lighting::occlusion::Fingerprints;
use rusty_scene::lighting::occlusion::OcclusionData;
use rusty_scene::revision::Revision;
use rusty_scene::Scene;

/// One scene's PVS, bound to its objects.
#[derive(Default)]
pub(crate) struct OcclusionBinding {
    data: Option<Arc<OcclusionData>>,
    /// The scene PVS's revision it was bound at; `None` until bound.
    seen: Option<Revision>,
    /// Bind whole on the next extract.
    stale: bool,
    /// Static objects changed since: re-check these alone.
    touched: BTreeSet<u32>,
    /// The `LODGroup` membership and kept geometry hashes fingerprints read.
    prints: Fingerprints,
    /// Entity → its index in the PVS, for the objects unchanged since the bake.
    unchanged: BTreeMap<u32, u32>,
    /// Baked objects that no longer match, and of them the occluders.
    changed: BTreeSet<u32>,
    broken: BTreeSet<u32>,
    /// Matching occluders that cast no shadow: while any does, no light culls.
    silent: BTreeSet<u32>,
    /// The layers the matching occluders sit on.
    layers: u32,
    /// The revision a stale bake was last reported at, and whether that report
    /// said culling stopped: said once, and again when an occluder breaks after.
    reported: Option<(Revision, bool)>,
    /// Fingerprints the last refresh computed: what a tween costs.
    pub(crate) rechecked: usize,
}

#[cfg(test)]
/// What a binding decides, for comparing an incrementally kept one with a fresh one.
pub(crate) type BindingState = (bool, u32, Vec<(u32, u32)>, Vec<u32>);

impl OcclusionBinding {
    /// Bind whole on the next refresh: the objects were rebuilt.
    pub(super) fn forget(&mut self) {
        self.stale = true;
    }

    /// Static object `id` came, went, changed or moved: re-check it.
    pub(super) fn touch(&mut self, id: u32) {
        self.touched.insert(id);
    }

    /// The `LODGroup`s changed: if which renderers they hold moved, the regrouped
    /// objects' fingerprints did too, so the bind is redone whole.
    pub(super) fn regroup(&mut self, scene: &Scene) {
        if self.prints.regroup(scene) {
            self.stale = true;
        }
    }

    /// Re-bind to `scene` if its PVS was replaced or a static object changed.
    pub(super) fn refresh(&mut self, scene: &Scene) {
        self.rechecked = 0;
        let revision = scene.occlusion.revision();
        if self.stale || self.seen != Some(revision) {
            self.bind(scene, revision);
        } else if !self.touched.is_empty() {
            let touched = std::mem::take(&mut self.touched);
            touched.into_iter().for_each(|id| self.check(scene, id));
        } else {
            return;
        }
        self.warn_changed(revision);
    }

    /// Check every baked object against `scene`.
    fn bind(&mut self, scene: &Scene, revision: Revision) {
        (self.seen, self.stale) = (Some(revision), false);
        self.touched.clear();
        self.data = scene.occlusion.as_ref().map(Arc::clone);
        self.unchanged.clear();
        self.changed.clear();
        self.broken.clear();
        self.silent.clear();
        self.layers = 0;
        let Some(data) = self.data.clone() else {
            return;
        };
        self.prints.regroup(scene);
        self.prints.sweep();
        for baked in &data.objects {
            self.check(scene, baked.entity);
        }
    }

    /// Re-check entity `id`, if it was baked.
    fn check(&mut self, scene: &Scene, id: u32) {
        let Some(data) = self.data.as_deref() else {
            return;
        };
        let Some(i) = data.object_of(id) else {
            return;
        };
        let baked = data.objects[i as usize];
        self.rechecked += 1;
        let now = self.prints.of(scene, id).map(|(print, _)| print);
        if now == Some(baked.fingerprint) {
            self.unchanged.insert(id, i);
            self.changed.remove(&id);
            self.broken.remove(&id);
            if baked.occluder {
                self.layers |= 1u32 << scene.world.layer(id).min(31);
                match casts(scene, id) {
                    true => self.silent.remove(&id),
                    false => self.silent.insert(id),
                };
            }
        } else {
            self.unchanged.remove(&id);
            self.changed.insert(id);
            self.silent.remove(&id);
            if baked.occluder {
                self.broken.insert(id);
            }
        }
    }

    /// Say once per PVS that it is older than the scene's statics, and again when
    /// a baked occluder changes after a report that only meshes did, since culling
    /// then stops.
    fn warn_changed(&mut self, revision: Revision) {
        let (moved, broken) = (self.changed.len(), self.broken.len());
        if !warning_due(self.reported, revision, (moved, broken)) {
            return;
        }
        self.reported = Some((revision, broken > 0));
        log::warn!(
            "[Occlusion] the bake is older than the scene's statics: {moved} baked mesh(es) \
             changed since; {}. Generate Lighting or Bake Occlusion re-bakes it.",
            match broken {
                0 => "they are always drawn".to_string(),
                n => format!("{n} were occluders, so nothing is occlusion-culled"),
            }
        );
    }

    /// Every baked occluder still matches.
    fn usable(&self) -> bool {
        self.broken.is_empty()
    }

    /// The PVS row a camera at `eye` drawing the layers of `mask` reads, `None`
    /// when it culls nothing by occlusion: no usable bake, an occluder's layer left
    /// out of its mask, or an eye outside every cell.
    pub(crate) fn row(&self, eye: Vec3, mask: u32) -> Option<u32> {
        let data = self.data.as_deref().filter(|_| self.usable())?;
        (mask & self.layers == self.layers)
            .then(|| data.row_at(eye))
            .flatten()
    }

    /// The PVS row a point or spot light at `light` culls its shadow casters by,
    /// `None` when it culls nothing: no usable bake, a matching occluder that casts
    /// no shadow (the wall that hides a caster must be in the light's map), or a
    /// light outside every cell. A light has no culling mask, so every layer counts.
    /// The row carries the camera's caveat: a caster the light sees only through a
    /// gap the bake's sampled points miss (under about a metre of it) is hidden,
    /// and a still light's cached tile keeps that lost shadow until it re-bakes.
    pub(crate) fn shadow_row(&self, light: Vec3) -> Option<u32> {
        let data = self.data.as_deref().filter(|_| self.usable())?;
        self.silent.is_empty().then(|| data.row_at(light)).flatten()
    }

    /// Whether entity `id` is a baked occluder unchanged since the bake: one a
    /// light's mask needs drawn.
    pub(crate) fn occludes(&self, id: u32) -> bool {
        let Some(data) = self.data.as_deref() else {
            return false;
        };
        let at = self.unchanged.get(&id);
        at.is_some_and(|&i| data.objects[i as usize].occluder)
    }

    /// The box of the cell holding `eye`, when the bake is usable there: what the
    /// editor's occlusion view outlines.
    pub(crate) fn cell_at(&self, eye: Vec3) -> Option<(Vec3, Vec3)> {
        let data = self.data.as_deref().filter(|_| self.usable())?;
        data.grid.index_of(eye).map(|cell| data.grid.bounds(cell))
    }

    /// Whether entity `id` is hidden from a cell whose row is `row`.
    pub(crate) fn hides(&self, row: u32, id: u32) -> bool {
        let Some(data) = self.data.as_deref() else {
            return false;
        };
        self.unchanged.get(&id).is_some_and(|&i| !data.sees(row, i))
    }

    /// A binding of `scene` from scratch, for a test outside `render::scene`.
    #[cfg(test)]
    pub(crate) fn bound(scene: &Scene) -> Self {
        let mut binding = Self::default();
        binding.refresh(scene);
        binding
    }

    /// Whether the bake is usable, the occluders' layers, the unchanged objects and
    /// the silent occluders.
    #[cfg(test)]
    pub(crate) fn state(&self) -> BindingState {
        let unchanged = self.unchanged.iter().map(|(&e, &i)| (e, i)).collect();
        let usable = self.data.is_some() && self.usable();
        // A broken bake's layers are never read.
        let silent = self.silent.iter().copied().collect();
        (
            usable,
            if usable { self.layers } else { 0 },
            unchanged,
            silent,
        )
    }
}

#[cfg(test)]
mod camera_gpu_tests;
#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
pub(super) mod ops;
#[cfg(test)]
mod shadow_tests;

/// Whether entity `id`'s mesh casts a shadow (any cast mode but Off).
fn casts(scene: &Scene, id: u32) -> bool {
    let mesh = scene.world.mesh(id);
    mesh.is_some_and(|m| m.cast_shadows.casts())
}

/// Whether a stale-bake warning is due, given the last one (`reported`: its
/// revision and whether it said culling stopped), the PVS `revision` and how many
/// baked meshes, and of them occluders, changed: once per revision, and again when
/// occluders broke after a warning that said only meshes did.
fn warning_due(
    reported: Option<(Revision, bool)>,
    revision: Revision,
    (moved, broken): (usize, usize),
) -> bool {
    let said = match reported {
        Some((r, said_broken)) if r == revision => broken == 0 || said_broken,
        _ => false,
    };
    moved > 0 && !said
}

#[cfg(test)]
#[path = "warn_tests.rs"]
mod warn_tests;
