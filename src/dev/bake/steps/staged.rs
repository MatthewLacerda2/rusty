//! What a stepped probe bake holds until it commits (#1602): the reflection probes as
//! they were when it started, and the cubemaps it has baked from them, written beside
//! their final names and moved into place only on commit.

use std::path::{Path, PathBuf};

use glam::Vec3;

use crate::render::ibl::reflection_bake::{cube_file, BakedCube};
use crate::scene::lighting::reflection_probe::clamp_resolution;
use crate::scene::lighting::sh::Sh9;
use crate::scene::{ReflectionProbe, ReflectionProbeSet, Scene};

/// The reflection probes a bake works on: copies taken at its start, each with the
/// face size it bakes at (its own, or the scene's default then).
pub(super) struct ReflectionSnapshot {
    probes: Vec<(ReflectionProbe, u32)>,
}

/// What a reflection probe's cube is baked from. A change to any of it, or to the
/// count, makes the staged cubes stale; a change to anything else (its box, its
/// intensity) does not, and survives the commit.
/// Whether it is manual counts too: the bake re-places the placed ones (#1737).
type BakeInputs = (Vec3, u32, f32, f32, bool, bool);

impl ReflectionSnapshot {
    pub(super) fn take(scene: &Scene) -> Self {
        let default = scene.lighting_settings.reflections.resolution;
        Self::of(&scene.reflection_probes, default)
    }

    /// Copies of `set`'s probes, each baked at its own face size or `default`.
    pub(super) fn of(set: &ReflectionProbeSet, default: u32) -> Self {
        let probes = set.probes.iter();
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
    (
        p.position,
        *resolution,
        p.near_clip,
        p.far_clip,
        p.shadows,
        p.manual,
    )
}

/// Cubemaps baked but not applied: each one at `<final>.baking`, moved over
/// `<scene>.reflections/reflection_probe_<i>.ktx2` on [`commit`](Self::commit). Whatever is still staged
/// when the bake is dropped is deleted, so a dropped bake leaves no file behind.
#[derive(Default)]
pub(super) struct StagedCubemaps {
    /// `(staged, final)` per probe baked, in probe order.
    files: Vec<(PathBuf, PathBuf)>,
    /// How many of `files` are moved into place: a failed commit resumes there.
    moved: usize,
    /// Each staged cube's own irradiance SH (#1697), in probe order.
    irradiance: Vec<Sh9>,
}

impl StagedCubemaps {
    /// How many probes have been baked.
    pub(super) fn len(&self) -> usize {
        self.files.len()
    }

    /// Stage the next probe's cube in `dir`, and the irradiance it was baked with.
    pub(super) fn write(&mut self, dir: &Path, baked: &BakedCube) -> Result<(), String> {
        let bytes = &baked.bytes;
        let target = cube_file(dir, self.files.len());
        let mut staged = target.clone().into_os_string();
        staged.push(".baking");
        let staged = PathBuf::from(staged);
        // lint: allow(asset-write) a `.baking` staging file no cache reads; `commit` notes the cube it moves into place
        std::fs::write(&staged, bytes).map_err(|e| {
            format!(
                "Failed to write reflection cubemap {}: {e}",
                staged.display()
            )
        })?;
        self.files.push((staged, target));
        self.irradiance.push(baked.irradiance);
        Ok(())
    }

    /// Move every staged cube not yet moved into place, then point each probe of
    /// `set` (the planned copy, not the scene's) at its cube and store the irradiance
    /// it was baked with (#1697). The caller has checked the probes are the ones
    /// baked. On a failed move `set` is untouched and a retry resumes at the cube that
    /// failed.
    pub(super) fn commit(&mut self, set: &mut ReflectionProbeSet) -> Result<(), String> {
        while let Some((staged, target)) = self.files.get(self.moved) {
            let path = target.to_string_lossy().into_owned();
            // Noted first, so the editor's follow never reads the engine's own write
            // as an outside edit.
            crate::core::fs::written::note_written(&path);
            // lint: allow(atomic-write) publishes a cube staged whole beside its name at bake time
            std::fs::rename(staged, target)
                .map_err(|e| format!("Failed to write reflection cubemap {path}: {e}"))?;
            self.moved += 1;
        }
        for (i, ((_, target), sh)) in self.files.iter().zip(&self.irradiance).enumerate() {
            set.set_cubemap(i, target.to_string_lossy().into_owned());
            set.set_irradiance(i, Some(*sh));
        }
        Ok(())
    }
}

impl Drop for StagedCubemaps {
    fn drop(&mut self) {
        // The moved ones' staged names are gone; a failed commit's rest remain.
        for (staged, _) in &self.files[self.moved..] {
            // Best effort: a staged file that is already gone needs no removing.
            let _ = std::fs::remove_file(staged);
        }
    }
}
