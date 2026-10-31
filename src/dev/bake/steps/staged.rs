//! What a stepped probe bake holds until it commits (#1602): the reflection probes as
//! they were when it started, and the cubemaps it has baked from them, written beside
//! their final names and moved into place only on commit.

use std::path::{Path, PathBuf};

use glam::Vec3;

use crate::scene::lighting::reflection_probe::clamp_resolution;
use crate::scene::{ReflectionProbe, Scene};

/// The reflection probes a bake works on: copies taken at its start, each with the
/// face size it bakes at (its own, or the scene's default then).
pub(super) struct ReflectionSnapshot {
    probes: Vec<(ReflectionProbe, u32)>,
}

/// What a reflection probe's cube is baked from. A change to any of it, or to the
/// count, makes the staged cubes stale; a change to anything else (its box, its
/// intensity) does not, and survives the commit.
type BakeInputs = (Vec3, u32, f32, f32, bool);

impl ReflectionSnapshot {
    pub(super) fn take(scene: &Scene) -> Self {
        let default = scene.lighting_settings.reflections.resolution;
        let probes = scene.reflection_probes.probes.iter();
        let probes = probes
            .map(|p| (p.clone(), clamp_resolution(p.resolution.unwrap_or(default))))
            .collect();
        Self { probes }
    }

    pub(super) fn len(&self) -> usize {
        self.probes.len()
    }

    /// Probe `i` as it was, and the face size it bakes at.
    pub(super) fn get(&self, i: usize) -> Option<&(ReflectionProbe, u32)> {
        self.probes.get(i)
    }

    /// Whether `scene`'s reflection probes are no longer the ones this snapshot holds.
    pub(super) fn changed(&self, scene: &Scene) -> bool {
        let now = Self::take(scene);
        now.probes.len() != self.probes.len()
            || now
                .probes
                .iter()
                .zip(&self.probes)
                .any(|(a, b)| inputs(a) != inputs(b))
    }
}

fn inputs((p, resolution): &(ReflectionProbe, u32)) -> BakeInputs {
    (p.position, *resolution, p.near_clip, p.far_clip, p.shadows)
}

/// Cubemaps baked but not applied: each one at `<final>.baking`, moved over
/// `reflection_probe_<i>.ktx2` on [`commit`](Self::commit). Whatever is still staged
/// when the bake is dropped is deleted, so a dropped bake leaves no file behind.
#[derive(Default)]
pub(super) struct StagedCubemaps {
    /// `(staged, final)` per probe baked, in probe order.
    files: Vec<(PathBuf, PathBuf)>,
    /// Whether the files were moved into place: nothing is left to delete.
    committed: bool,
}

impl StagedCubemaps {
    /// How many probes have been baked.
    pub(super) fn len(&self) -> usize {
        self.files.len()
    }

    /// Stage the next probe's cube in `dir`.
    pub(super) fn write(&mut self, dir: &Path, bytes: &[u8]) -> Result<(), String> {
        let name = format!("reflection_probe_{}.ktx2", self.files.len());
        let target = dir.join(&name);
        let staged = dir.join(format!("{name}.baking"));
        std::fs::write(&staged, bytes).map_err(|e| {
            format!(
                "Failed to write reflection cubemap {}: {e}",
                staged.display()
            )
        })?;
        self.files.push((staged, target));
        Ok(())
    }

    /// Move every staged cube into place and point its probe at it. The caller has
    /// checked the probes are the ones baked.
    pub(super) fn commit(&mut self, scene: &mut Scene) -> Result<(), String> {
        if std::mem::replace(&mut self.committed, true) {
            return Ok(());
        }
        for i in 0..self.files.len() {
            let (staged, target) = &self.files[i];
            let path = target.to_string_lossy().into_owned();
            // Noted first, so the editor's follow never reads the engine's own write
            // as an outside edit.
            crate::core::fs::written::note_written(&path);
            // lint: allow(atomic-write) publishes a cube staged whole beside its name at bake time
            std::fs::rename(staged, target)
                .map_err(|e| format!("Failed to write reflection cubemap {path}: {e}"))?;
            scene.reflection_probes.set_cubemap(i, path);
        }
        Ok(())
    }
}

impl Drop for StagedCubemaps {
    fn drop(&mut self) {
        // After a commit the staged names are gone; a failed commit's rest remain.
        for (staged, _) in &self.files {
            // Best effort: a staged file that is already gone needs no removing.
            let _ = std::fs::remove_file(staged);
        }
    }
}
