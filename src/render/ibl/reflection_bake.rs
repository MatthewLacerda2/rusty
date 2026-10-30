//! src/render/reflection_bake.rs — Reflection-probe bake driver (#245).
//!
//! The specular sibling of the light-probe bake (`probe/bake.rs`). For each reflection
//! probe it captures the STATIC scene to a 6-face cubemap from the probe position
//! (`capture_static_cubemap`, #243), GGX-prefilters it into a roughness mip chain
//! (`reflection_prefilter`), encodes the result to KTX2 (`ktx2_encode`), writes that file
//! next to the scene, and stores its path on the probe (the `cubemap_path` field from
//! #244). Each frame loads it through the probe array's cubemap cache
//! (`Renderer::cached_cubemap`), and the forward pass reflects it.
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
use crate::render::ibl::reflection_prefilter::prefilter;
use crate::render::Renderer;
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
        // A scene file's values reach here unclamped: bake from a clamped copy.
        let mut probe = probe.clone();
        probe.clamp_settings();
        let resolution = clamp_resolution(resolution);
        let lens = CaptureLens {
            near: probe.near_clip,
            far: probe.far_clip,
            shadows: probe.shadows,
        };
        let capture = self.capture_reflection_cubemap(scene, probe.position, resolution, lens);
        let prefiltered = prefilter(&capture);
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

    /// Bake every reflection probe in `scene` at its resolution, writing each one's KTX2 cubemap into
    /// `out_dir` (named `reflection_probe_<i>.ktx2`) and pointing the probe's
    /// `cubemap_path` at it. Returns the number of probes baked, or an error if a file
    /// could not be written. With no probes it is a successful no-op (0 baked).
    pub fn bake_reflections(&mut self, scene: &mut Scene, out_dir: &Path) -> Result<usize, String> {
        let default = scene.lighting_settings.reflections.resolution;
        let probes = scene.reflection_probes.probes.clone();
        for (i, probe) in probes.iter().enumerate() {
            let resolution = clamp_resolution(probe.resolution.unwrap_or(default));
            let bytes = self.bake_reflection(scene, probe, resolution);
            let path = out_dir.join(format!("reflection_probe_{i}.ktx2"));
            crate::core::fs::written::write(&path, &bytes).map_err(|e| {
                format!("Failed to write reflection cubemap {}: {e}", path.display())
            })?;
            let path_str = path.to_string_lossy().into_owned();
            scene.reflection_probes.set_cubemap(i, path_str);
        }
        Ok(scene.reflection_probes.len())
    }
}

#[cfg(test)]
#[path = "reflection_bake_tests.rs"]
mod reflection_bake_tests;
