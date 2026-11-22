use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use glam::Vec3;

use rusty_scene::lighting::occlusion::gather::Fingerprints;
use rusty_scene::lighting::occlusion::OcclusionData;
use rusty_scene::revision::Revision;
use rusty_scene::Scene;

#[derive(Default)]
pub(crate) struct OcclusionBinding {
    data: Option<Arc<OcclusionData>>,
    seen: Option<Revision>,
    stale: bool,
    touched: BTreeSet<u32>,
    prints: Fingerprints,
    unchanged: BTreeMap<u32, u32>,
    changed: BTreeSet<u32>,
    broken: BTreeSet<u32>,
    silent: BTreeSet<u32>,
    layers: u32,
    reported: Option<(Revision, bool)>,
    /// Fingerprints the last refresh computed: a test hook (`camera_gpu_tests`).
    pub(crate) rechecked: usize,
}

#[cfg(test)]
pub(crate) type BindingState = (bool, u32, Vec<(u32, u32)>, Vec<u32>);

impl OcclusionBinding {
    pub(super) fn forget(&mut self) {
        self.stale = true;
    }

    pub(super) fn touch(&mut self, id: u32) {
        self.touched.insert(id);
    }

    pub(super) fn regroup(&mut self, scene: &Scene) {
        if self.prints.regroup(scene) {
            self.stale = true;
        }
    }

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

    fn usable(&self) -> bool {
        self.broken.is_empty()
    }

    pub(crate) fn row(&self, eye: Vec3, mask: u32) -> Option<u32> {
        let data = self.data.as_deref().filter(|_| self.usable())?;
        (mask & self.layers == self.layers)
            .then(|| data.row_at(eye))
            .flatten()
    }

    pub(crate) fn shadow_row(&self, light: Vec3) -> Option<u32> {
        let data = self.data.as_deref().filter(|_| self.usable())?;
        self.silent.is_empty().then(|| data.row_at(light)).flatten()
    }

    pub(crate) fn occludes(&self, id: u32) -> bool {
        let Some(data) = self.data.as_deref() else {
            return false;
        };
        let at = self.unchanged.get(&id);
        at.is_some_and(|&i| data.objects[i as usize].occluder)
    }

    pub(crate) fn cell_at(&self, eye: Vec3) -> Option<(Vec3, Vec3)> {
        let data = self.data.as_deref().filter(|_| self.usable())?;
        data.grid.index_of(eye).map(|cell| data.grid.bounds(cell))
    }

    pub(crate) fn hides(&self, row: u32, id: u32) -> bool {
        let Some(data) = self.data.as_deref() else {
            return false;
        };
        self.unchanged.get(&id).is_some_and(|&i| !data.sees(row, i))
    }

    #[cfg(test)]
    pub(crate) fn bound(scene: &Scene) -> Self {
        let mut binding = Self::default();
        binding.refresh(scene);
        binding
    }

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

mod stale;

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

fn casts(scene: &Scene, id: u32) -> bool {
    let mesh = scene.world.mesh(id);
    mesh.is_some_and(|m| m.cast_shadows.casts())
}

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
