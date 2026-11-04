//! src/render/ibl/reflection_bake.rs — Reflection-probe bake driver (#245).
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

use crate::render::ibl::cubemap_capture::CaptureLens;
use crate::render::ibl::ktx2_encode::encode_cubemap;
use crate::render::ibl::probe::bake::project_cubemap;
use crate::render::ibl::reflection_prefilter::gpu::ProbePrefilter;
use crate::render::ibl::reflection_prefilter::PrefilteredCubemap;
use crate::render::{CubemapCapture, Renderer};
use crate::scene::lighting::lightmap::{baked_direct_sh, BakeScene};
use crate::scene::lighting::reflection_probe::clamp_resolution;
use crate::scene::lighting::sh::Sh9;
use crate::scene::{ReflectionProbe, ReflectionProbeSet, Scene};

/// One baked reflection probe: its KTX2 cube, and the irradiance that cube gives at
/// the probe, the cube's own L2 SH ([`project_cubemap`]) before the direct light of
/// `Baked` lights is added ([`with_baked_direct`]).
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
        let capture = self.capture_reflection_cubemap(scene, probe.position, resolution, lens);
        let irradiance = project_cubemap(&capture);
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
        let mut cubes = Vec::with_capacity(total);
        for i in 0..total {
            observe(i, total);
            cubes.push(self.bake_reflection_into(scene, i, out_dir)?);
        }
        let irradiance = with_baked_direct(scene, &scene.reflection_probes, &cubes);
        for (i, sh) in irradiance.into_iter().enumerate() {
            scene.reflection_probes.set_irradiance(i, sh);
        }
        Ok(total)
    }

    /// Bake reflection probe `i` of `scene` at its resolution into
    /// `out_dir/reflection_probe_<i>.ktx2` and point the probe at it: one probe of
    /// [`Renderer::bake_reflections`]. The editor's stepped bake does not come here: it
    /// steps through `LightingBake::step`, which stages each cube and applies them all
    /// at the end (#1602). Returns the cube's own irradiance SH, which the caller
    /// finishes with [`with_baked_direct`] and stores; the probe's stored one is left
    /// as it was. A probe index past the set bakes nothing (`None`).
    fn bake_reflection_into(
        &mut self,
        scene: &mut Scene,
        i: usize,
        out_dir: &Path,
    ) -> Result<Option<Sh9>, String> {
        let Some(probe) = scene.reflection_probes.probes.get(i).cloned() else {
            return Ok(None);
        };
        let default = scene.lighting_settings.reflections.resolution;
        let resolution = clamp_resolution(probe.resolution.unwrap_or(default));
        let baked = self.bake_reflection(scene, &probe, resolution);
        let path = cube_file(out_dir, i);
        crate::core::fs::written::write(&path, &baked.bytes)
            .map_err(|e| format!("Failed to write reflection cubemap {}: {e}", path.display()))?;
        scene
            .reflection_probes
            .set_cubemap(i, path.to_string_lossy().into_owned());
        Ok(Some(baked.irradiance))
    }
}

/// The irradiance each probe of `set` stores (#1697): `cubes[i]`, probe `i`'s cube
/// SH (`None` for one not baked), plus the direct light of `scene`'s `Baked` lights
/// at its position, shadowed by the static meshes and traced once for every probe
/// (`baked_direct_sh`). A capture sees lit surfaces, never a light, so without it a
/// probe beside a `Baked` lamp would store only the lamp's bounce, while the lightmap
/// and the light probes it is compared with hold its direct light too: the light
/// probe bake adds it the same way (`probe::bounce`).
pub fn with_baked_direct(
    scene: &Scene,
    set: &ReflectionProbeSet,
    cubes: &[Option<Sh9>],
) -> Vec<Option<Sh9>> {
    let positions: Vec<Vec3> = set.probes.iter().map(|p| p.position).collect();
    let statics = BakeScene::gather(scene, &|_| None);
    let direct = baked_direct_sh(&statics, &positions);
    (cubes.iter().zip(direct))
        .map(|(cube, direct)| {
            cube.map(|mut sh| {
                sh.accumulate(&direct);
                sh
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "reflection_bake_tests.rs"]
mod reflection_bake_tests;

#[cfg(test)]
#[path = "reflection_irradiance_tests.rs"]
mod reflection_irradiance_tests;
