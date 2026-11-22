use std::path::{Path, PathBuf};

use glam::Vec3;

use rusty_render::ibl::reflection_bake::{cube_file, BakedCube};
use rusty_scene::lighting::reflection_probe::clamp_resolution;
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::{ReflectionProbe, ReflectionProbeSet, Scene};

pub(super) struct ReflectionSnapshot {
    probes: Vec<(ReflectionProbe, u32)>,
}

type BakeInputs = (Vec3, u32, f32, f32, bool, bool);

impl ReflectionSnapshot {
    pub(super) fn take(scene: &Scene) -> Self {
        let default = scene.lighting_settings.reflections.resolution;
        Self::of(&scene.reflection_probes, default)
    }

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

    pub(super) fn get(&self, i: usize) -> Option<&(ReflectionProbe, u32)> {
        self.probes.get(i)
    }

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

#[derive(Default)]
pub(super) struct StagedCubemaps {
    files: Vec<(PathBuf, PathBuf)>,
    moved: usize,
    irradiance: Vec<Sh9>,
}

impl StagedCubemaps {
    pub(super) fn len(&self) -> usize {
        self.files.len()
    }

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

    pub(super) fn commit(&mut self, set: &mut ReflectionProbeSet) -> Result<(), String> {
        while let Some((staged, target)) = self.files.get(self.moved) {
            let path = target.to_string_lossy().into_owned();
            rusty_core::fs::written::note_written(&path);
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
        for (staged, _) in &self.files[self.moved..] {
            let _ = std::fs::remove_file(staged);
        }
    }
}
