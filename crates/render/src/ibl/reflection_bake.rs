use std::path::{Path, PathBuf};

use glam::Vec3;

use crate::ibl::cubemap_capture::CaptureLens;
use crate::ibl::ktx2_encode::encode_cubemap;
use crate::ibl::probe::bake::project_light_probe;
use crate::ibl::probe::capture::ProbeCapture;
use crate::ibl::reflection_prefilter::gpu::ProbePrefilter;
use crate::ibl::reflection_prefilter::PrefilteredCubemap;
use crate::{CubemapCapture, Renderer};
use rusty_scene::lighting::reflection_probe::clamp_resolution;
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::{AmbientRadiance, ReflectionProbe, Scene};

pub struct BakedCube {
    pub bytes: Vec<u8>,
    pub irradiance: Sh9,
}

pub const CUBE_PREFIX: &str = "reflection_probe_";

/// The one name both the run-to-end bake and the editor's stepped bake write.
pub fn cube_file(dir: &Path, i: usize) -> PathBuf {
    dir.join(format!("{CUBE_PREFIX}{i}.ktx2"))
}

impl Renderer {
    pub fn bake_reflection(
        &mut self,
        scene: &Scene,
        probe: &ReflectionProbe,
        resolution: u32,
    ) -> BakedCube {
        let mut probe = probe.clone();
        probe.clamp_settings();
        let resolution = clamp_resolution(resolution);
        let lens = CaptureLens {
            near: probe.near_clip,
            far: probe.far_clip,
            shadows: probe.shadows,
        };
        let (capture, open) =
            self.capture_reflection_cubemap(scene, probe.position, resolution, lens);
        let prefiltered = self.prefilter_gpu(&capture);
        let irradiance = stored_irradiance(capture, open, scene.drawn_ambient().radiance());
        log::debug!(
            "[ReflectionBake] {} mips, roughest at mip {:.1}",
            prefiltered.level_count(),
            crate::ibl::reflection_prefilter::roughness_to_mip(
                1.0,
                prefiltered.level_count() as u32
            )
        );
        BakedCube {
            bytes: encode_cubemap(&prefiltered),
            irradiance,
        }
    }

    pub(crate) fn prefilter_gpu(&self, capture: &CubemapCapture) -> PrefilteredCubemap {
        let filter = self
            .probe_prefilter
            .get_or_init(|| ProbePrefilter::new(&self.device));
        filter.run((&self.device, (&self.queue, &self.staging)), capture)
    }

    /// A test hook: the bake tests call it; the dev bake calls `bake_reflections_observed`.
    pub fn bake_reflections(&mut self, scene: &mut Scene, out_dir: &Path) -> Result<usize, String> {
        self.bake_reflections_observed(scene, out_dir, &mut |_, _| {})
    }

    pub fn bake_reflections_observed(
        &mut self,
        scene: &mut Scene,
        out_dir: &Path,
        observe: &mut dyn FnMut(usize, usize),
    ) -> Result<usize, String> {
        let total = scene.reflection_probes.len();
        for i in 0..total {
            observe(i, total);
            self.bake_reflection_into(scene, i, out_dir)?;
        }
        Ok(total)
    }

    fn bake_reflection_into(
        &mut self,
        scene: &mut Scene,
        i: usize,
        out_dir: &Path,
    ) -> Result<(), String> {
        let Some(probe) = scene.reflection_probes.probes.get(i).cloned() else {
            return Ok(());
        };
        let default = scene.lighting_settings.reflections.resolution;
        let resolution = clamp_resolution(probe.resolution.unwrap_or(default));
        let baked = self.bake_reflection(scene, &probe, resolution);
        let path = cube_file(out_dir, i);
        rusty_core::fs::written::write(&path, &baked.bytes)
            .map_err(|e| format!("Failed to write reflection cubemap {}: {e}", path.display()))?;
        let set = &mut scene.reflection_probes;
        set.set_cubemap(i, path.to_string_lossy().into_owned());
        set.set_irradiance(i, Some(baked.irradiance));
        Ok(())
    }
}

/// The capture's irradiance SH, with open (sky) texels taking the ambient as the lightmap and a
/// light probe see them, so the forward pass's ratio of the cube to the lightmap is like for like.
pub fn stored_irradiance(
    mut cube: CubemapCapture,
    open: [Vec<bool>; 6],
    ambient: AmbientRadiance,
) -> Sh9 {
    for (face, open) in cube.faces.iter_mut().zip(&open) {
        for (texel, _) in face.iter_mut().zip(open).filter(|(_, &o)| o) {
            *texel = Vec3::ZERO;
        }
    }
    project_light_probe(&ProbeCapture { cube, open }, ambient)
}

#[cfg(test)]
#[path = "reflection_bake_tests.rs"]
mod reflection_bake_tests;

#[cfg(test)]
#[path = "stored_irradiance_tests.rs"]
mod stored_irradiance_tests;
