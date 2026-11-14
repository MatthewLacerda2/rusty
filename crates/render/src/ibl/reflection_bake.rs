//! crates/render/src/ibl/reflection_bake.rs — Reflection-probe bake driver (#245).
//!
//! The specular sibling of the light-probe bake (`probe/bake.rs`). For each reflection
//! probe it captures the STATIC scene to a 6-face cubemap from the probe position
//! (`capture_reflection_cubemap`, #243, #1584), GGX-prefilters it into a roughness mip
//! chain (`reflection_prefilter`, on the GPU since #1602), encodes the result to KTX2
//! (`ktx2_encode`), writes that file into the folder it is given ([`cube_file`]; the
//! dev bake passes the scene's own `<scene>.reflections/`, #1692), and stores its path on the
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

/// One baked reflection probe: its KTX2 cube, and the irradiance it stores beside it
/// (#1697, [`stored_irradiance`]).
pub struct BakedCube {
    pub bytes: Vec<u8>,
    pub irradiance: Sh9,
}

/// The name every baked cube starts with: probe `i`'s is `reflection_probe_<i>.ktx2`.
pub const CUBE_PREFIX: &str = "reflection_probe_";

/// Where probe `i`'s cube goes in `dir`: the one name both bake paths (this run-to-end
/// one and the editor's stepped one) write.
pub fn cube_file(dir: &Path, i: usize) -> PathBuf {
    dir.join(format!("{CUBE_PREFIX}{i}.ktx2"))
}

impl Renderer {
    /// Bake one reflection probe: capture the static scene from its position at
    /// `resolution` per face through its clip planes and shadow setting, prefilter it
    /// into a roughness mip chain, and return the encoded KTX2 bytes with the
    /// capture's irradiance SH (#1697).
    pub fn bake_reflection(
        &mut self,
        scene: &Scene,
        probe: &ReflectionProbe,
        resolution: u32,
    ) -> BakedCube {
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
        let (capture, open) =
            self.capture_reflection_cubemap(scene, probe.position, resolution, lens);
        let prefiltered = self.prefilter_gpu(&capture);
        let irradiance = stored_irradiance(capture, open, scene.drawn_ambient().radiance());
        // The mip a fully-rough surface samples (CPU mirror of the shader's selection),
        // logged so a bake's roughness range is visible when debugging a probe.
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
        self.bake_reflections_observed(scene, out_dir, &mut |_, _| {})
    }

    /// [`bake_reflections`](Self::bake_reflections), telling `observe` `(done, total)`
    /// before each probe. The one run-to-end loop: `Reflection.Bake()` logs through it
    /// (#1667).
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

    /// Bake reflection probe `i` of `scene` at its resolution into
    /// `out_dir/reflection_probe_<i>.ktx2` and point the probe at it: one probe of
    /// [`Renderer::bake_reflections`]. The editor's stepped bake does not come here: it
    /// steps through `LightingBake::step`, which stages each cube and applies them all
    /// at the end (#1602). The probe stores the cube's irradiance (#1697). A probe
    /// index past the set bakes nothing.
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

/// The irradiance a reflection probe stores beside its cube (#1697): what the
/// lightmap and a light probe at its position measure, so the forward pass's ratio of
/// the two compares like with like. A reflection capture draws the sky where nothing
/// covers a texel (`open`), while the lightmap's escaping rays (`Tracer::sky`) and a
/// light probe's capture take the scene's ambient there; so those texels are cleared
/// and the projection adds `ambient` along them, as [`project_light_probe`] does for
/// a light probe. Glass drawn over the sky is cleared with it: a light probe would
/// keep it, but it holds a little of a texel the ambient fills anyway. The direct
/// light of `Baked` lamps is left out: the capture sees what they light, never the
/// lamps, so the stored value is the bounce light the cube shows, and does not
/// depend on how near a lamp the probe was placed.
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
