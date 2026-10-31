//! src/render/ibl/reflection_bake.rs — Reflection-probe bake driver (#245).
//!
//! The specular sibling of the light-probe bake (`probe/bake.rs`). For each reflection
//! probe it captures the STATIC scene to a 6-face cubemap from the probe position
//! (`capture_reflection_cubemap`, #243, #1584), GGX-prefilters it into a roughness mip
//! chain (`reflection_prefilter`, on the GPU since #1602), encodes the result to KTX2
//! (`ktx2_encode`), writes that file next to the scene, and stores its path on the
//! probe (the `cubemap_path` field from #244). The probe array copies it in while the
//! probe is near the camera (`Renderer::update_reflection_probes`), and the forward
//! pass reflects it.
//!
//! Each probe bakes at its own resolution, or the scene's default
//! (`LightingSettings::reflections`), through its own lens: its clip planes and
//! shadows (#1584). The probe array the frame copies cubes into takes the face of
//! the largest resident cube, so every power-of-two size fits.
//!
//! Pure render layer: a function of (static scene, probe positions) plus the output
//! directory — no wall-clock / RNG, so it is clean under the determinism guard
//! like the capture and prefilter it stands on. File I/O is plain baked-asset output.

use std::path::Path;

use crate::render::ibl::cubemap_capture::CaptureLens;
use crate::render::ibl::ktx2_encode::encode_cubemap;
use crate::render::ibl::reflection_prefilter::gpu::ProbePrefilter;
use crate::render::ibl::reflection_prefilter::PrefilteredCubemap;
use crate::render::{CubemapCapture, Renderer};
use crate::scene::lighting::reflection_probe::clamp_resolution;
use crate::scene::{ReflectionProbe, Scene};

impl Renderer {
    /// Bake one reflection probe: capture the static scene from its position at
    /// `resolution` per face through its clip planes and shadow setting, prefilter it
    /// into a roughness mip chain, and return the encoded KTX2 bytes.
    pub fn bake_reflection(
        &mut self,
        scene: &Scene,
        probe: &ReflectionProbe,
        resolution: u32,
    ) -> Vec<u8> {
        // A scene's probes are held to range on load and on edit, but any probe may be
        // passed here (a test builds its own): bake from a held copy.
        let mut probe = probe.clone();
        probe.clamp_settings();
        let resolution = clamp_resolution(resolution);
        let lens = CaptureLens {
            near: probe.near_clip,
            far: probe.far_clip,
            shadows: probe.shadows,
        };
        let capture = self.capture_reflection_cubemap(scene, probe.position, resolution, lens);
        let prefiltered = self.prefilter_gpu(&capture);
        // The mip a fully-rough surface samples (CPU mirror of the shader's selection),
        // logged so a bake's roughness range is visible when debugging a probe.
        log::debug!(
            "[ReflectionBake] {} mips, roughest at mip {:.1}",
            prefiltered.level_count(),
            crate::render::ibl::reflection_prefilter::roughness_to_mip(
                1.0,
                prefiltered.level_count() as u32
            )
        );
        encode_cubemap(&prefiltered)
    }

    /// GGX-prefilter `capture` on the GPU (#1602), building the pipeline on the
    /// renderer's first bake.
    pub(crate) fn prefilter_gpu(&self, capture: &CubemapCapture) -> PrefilteredCubemap {
        let filter = self
            .probe_prefilter
            .get_or_init(|| ProbePrefilter::new(&self.device));
        filter.run((&self.device, &self.queue), capture)
    }

    /// Bake every reflection probe in `scene` at its resolution, writing each one's KTX2 cubemap into
    /// `out_dir` (named `reflection_probe_<i>.ktx2`) and pointing the probe's
    /// `cubemap_path` at it. Returns the number of probes baked, or an error if a file
    /// could not be written. With no probes it is a successful no-op (0 baked).
    pub fn bake_reflections(&mut self, scene: &mut Scene, out_dir: &Path) -> Result<usize, String> {
        for i in 0..scene.reflection_probes.len() {
            self.bake_reflection_into(scene, i, out_dir)?;
        }
        Ok(scene.reflection_probes.len())
    }

    /// Bake reflection probe `i` of `scene` at its resolution into
    /// `out_dir/reflection_probe_<i>.ktx2` and point the probe at it: one probe of
    /// [`Renderer::bake_reflections`]. The editor's stepped bake does not come here: it
    /// steps through `LightingBake::step`, which stages each cube and applies them all
    /// at the end (#1602). A probe index past the set bakes nothing.
    pub fn bake_reflection_into(
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
        let bytes = self.bake_reflection(scene, &probe, resolution);
        let path = out_dir.join(format!("reflection_probe_{i}.ktx2"));
        crate::core::fs::written::write(&path, &bytes)
            .map_err(|e| format!("Failed to write reflection cubemap {}: {e}", path.display()))?;
        scene
            .reflection_probes
            .set_cubemap(i, path.to_string_lossy().into_owned());
        Ok(())
    }
}

#[cfg(test)]
#[path = "reflection_bake_tests.rs"]
mod reflection_bake_tests;
