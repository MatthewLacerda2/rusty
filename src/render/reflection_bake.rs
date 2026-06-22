//! src/render/reflection_bake.rs — Reflection-probe bake driver (#245).
//!
//! The specular sibling of the light-probe bake (`probe_bake.rs`). For each reflection
//! probe it captures the STATIC scene to a 6-face cubemap from the probe position
//! (`capture_static_cubemap`, #243), GGX-prefilters it into a roughness mip chain
//! (`reflection_prefilter`), encodes the result to KTX2 (`ktx2_encode`), writes that file
//! next to the scene, and stores its path on the probe (the `cubemap_path` field from
//! #244). The runtime then loads it (`load_cubemap`) and the forward pass reflects it.
//!
//! Pure render layer: a function of (static scene, probe positions, resolution) plus the
//! output directory — no wall-clock / RNG, so it is clean under the determinism guard
//! like the capture and prefilter it stands on. File I/O is plain baked-asset output.

use std::path::Path;

use super::ktx2_encode::encode_cubemap;
use super::reflection_prefilter::prefilter;
use super::Renderer;
use crate::scene::Scene;

/// Default per-face capture resolution for a reflection bake. Higher than the light-probe
/// bake's 32 (which only feeds a 9-coefficient SH): a reflection cubemap is sampled
/// directly, so it wants enough base resolution that a mirror surface reads crisp room
/// detail before the rougher mips blur it.
pub const DEFAULT_REFLECTION_RESOLUTION: u32 = 128;

impl Renderer {
    /// Bake one reflection probe: capture the static scene from `position`, prefilter it
    /// into a roughness mip chain, and return the encoded KTX2 bytes. `resolution` is the
    /// per-face base edge length (clamped to the renderer's offscreen size by the capture).
    pub fn bake_reflection(
        &mut self,
        scene: &Scene,
        position: glam::Vec3,
        resolution: u32,
    ) -> Vec<u8> {
        let capture = self.capture_static_cubemap(scene, position, resolution);
        let prefiltered = prefilter(&capture);
        // The mip a fully-rough surface samples (CPU mirror of the shader's selection),
        // logged so a bake's roughness range is visible when debugging a probe.
        log::debug!(
            "[ReflectionBake] {} mips, roughest at mip {:.1}",
            prefiltered.level_count(),
            super::reflection_prefilter::roughness_to_mip(1.0, prefiltered.level_count() as u32)
        );
        encode_cubemap(&prefiltered)
    }

    /// Bake every reflection probe in `scene`, writing each one's KTX2 cubemap into
    /// `out_dir` (named `reflection_probe_<i>.ktx2`) and pointing the probe's
    /// `cubemap_path` at it. Returns the number of probes baked, or an error if a file
    /// could not be written. With no probes it is a successful no-op (0 baked).
    pub fn bake_reflections(
        &mut self,
        scene: &mut Scene,
        out_dir: &Path,
        resolution: u32,
    ) -> Result<usize, String> {
        let positions: Vec<glam::Vec3> = scene
            .reflection_probes
            .probes
            .iter()
            .map(|p| p.position)
            .collect();
        for (i, position) in positions.into_iter().enumerate() {
            let bytes = self.bake_reflection(scene, position, resolution);
            let path = out_dir.join(format!("reflection_probe_{i}.ktx2"));
            std::fs::write(&path, &bytes).map_err(|e| {
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
