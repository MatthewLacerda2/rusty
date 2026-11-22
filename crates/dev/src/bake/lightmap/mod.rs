use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec3;

use crate::bake::denoise::{self, Oidn};
use crate::bake::progress::{BakeLog, MeshCost, MeshTime, Step};
use crate::bake::report::{self, lightmaps::Measured, Stages};
use rusty_asset::MapRef;
use rusty_lighting::lightmap::{BakeProgress, BakeScene, MapEncoding};
use rusty_scene::lighting::settings::BakeSettings;
use rusty_scene::Scene;

mod encoder;
pub mod fits;
pub mod job;
pub(in crate::bake) mod logged;
mod measure;
mod pages;
mod preview;
mod spill;
mod write;
pub use job::{BakeOutcome, LightmapBakeJob};
pub use pages::{encode_pages, EncodedPages};
#[cfg(test)]
use write::content_tag;
pub use write::{apply_encoded, apply_lightmaps};

pub fn lightmap_dir(scene_path: &str) -> PathBuf {
    PathBuf::from(format!(
        "{scene_path}{}",
        rusty_asset::lightmap_page::DIR_SUFFIX
    ))
}

pub fn bake_scene_lightmaps(
    scene: &mut Scene,
    scene_path: Option<&str>,
    settings: &BakeSettings,
) -> Result<usize, String> {
    bake_scene_lightmaps_logged(scene, scene_path, settings, &mut BakeLog::off())
}

pub fn bake_scene_lightmaps_logged(
    scene: &mut Scene,
    scene_path: Option<&str>,
    settings: &BakeSettings,
    log: &mut BakeLog,
) -> Result<usize, String> {
    let scene_path = scene_path.ok_or(UNSAVED)?;
    let mut stages = Stages::start();
    spill::sweep(scene_path);
    let input = gather_bake_input(scene);
    fits::fits(&input, settings, false)?;
    crate::memory::return_large_frees();
    stages.lap("gather");
    let run = logged::bake_logged(&input, settings, log, scene_path)?;
    stages.push("trace", run.trace);
    stages.push("encode", run.encode);
    log.enter(Step::WritingLightmaps);
    let times = &run.measured.times;
    log.mesh_costs(times.iter().map(|t| mesh_cost(scene, t)).collect());
    let mut encoded = run.pages;
    encoded.lights = rusty_lighting::lightmap::taken_lights(scene);
    encoded.shadowmask = input.shadowmask.clone();
    warn_missing_uvs(lightmap_receivers(&input), encoded.maps);
    let written = apply_encoded(scene, scene_path, &encoded)?;
    stages.lap("write");
    let baked = (&encoded, &run.measured);
    keep_report(scene, scene_path, (&input, settings), baked, &stages);
    Ok(written)
}

fn keep_report(
    scene: &Scene,
    scene_path: &str,
    read: (&BakeScene, &BakeSettings),
    baked: (&EncodedPages, &Measured),
    stages: &Stages,
) {
    let written = report::bytes_under(&lightmap_dir(scene_path));
    let report = report::lightmaps::report(scene, read, baked, stages, written);
    report::keep(scene_path, report);
}

pub(super) fn lend_denoiser(
    progress: BakeProgress,
    settings: &BakeSettings,
) -> (BakeProgress, Option<Arc<Oidn>>) {
    match denoise::lent(settings) {
        Some(oidn) => (progress.with_denoiser(oidn.clone()), Some(oidn)),
        None => (progress, None),
    }
}

fn mesh_cost(scene: &Scene, time: &MeshTime) -> MeshCost {
    let name = scene.world.name(time.entity).map(|n| n.to_string());
    MeshCost {
        name: name.unwrap_or_else(|| format!("#{}", time.entity)),
        texels: time.texels,
        busy: time.busy,
    }
}

pub fn lightmap_receivers(input: &BakeScene) -> usize {
    input
        .meshes
        .iter()
        .filter(|m| m.receives_lightmap())
        .count()
}

/// Each mesh with a usable lightmap UV bakes exactly one lightmap, so the difference is the
/// meshes without one.
pub fn missing_uv_report(static_meshes: usize, lightmaps: usize) -> Option<String> {
    let n = static_meshes.saturating_sub(lightmaps);
    let (noun, verb) = if n == 1 {
        ("mesh", "has")
    } else {
        ("meshes", "have")
    };
    (n > 0)
        .then(|| format!("{n} static {noun} {verb} no lightmap UV — enable Generate Lightmap UVs"))
}

fn warn_missing_uvs(static_meshes: usize, lightmaps: usize) {
    if let Some(report) = missing_uv_report(static_meshes, lightmaps) {
        log::warn!("[LightmapBake] {report}");
    }
}

pub const UNSAVED: &str = "save the scene before baking lightmaps";

pub fn gather_bake_input(scene: &Scene) -> BakeScene {
    BakeScene::gather(scene, &texture_average)
}

fn texture_average(map: &MapRef, encoding: MapEncoding) -> Option<Vec3> {
    let file = map.file()?;
    let rgba = image::open(file).ok()?.to_rgba8();
    Some(average_texels(
        rgba.pixels().map(|p| [p[0], p[1], p[2]]),
        encoding,
    ))
}

fn average_texels(texels: impl Iterator<Item = [u8; 3]>, encoding: MapEncoding) -> Vec3 {
    let read = |b: u8| match encoding {
        MapEncoding::Colour => rusty_core::color::srgb_to_linear(b),
        MapEncoding::Data => f32::from(b) / 255.0,
    };
    let (sum, count) = texels.fold((Vec3::ZERO, 0u32), |(acc, n), [r, g, b]| {
        (acc + Vec3::new(read(r), read(g), read(b)), n + 1)
    });
    sum / count.max(1) as f32
}

#[cfg(test)]
mod covered_tests;
#[cfg(test)]
mod fits_tests;
#[cfg(test)]
mod job_tests;
#[cfg(test)]
mod limits_tests;
#[cfg(test)]
mod logged_tests;
#[cfg(test)]
pub(crate) mod memory_tests;
#[cfg(test)]
mod pages_memory_tests;
#[cfg(test)]
mod preview_edit_tests;
#[cfg(test)]
mod preview_tests;
#[cfg(test)]
mod report_tests;
#[cfg(test)]
mod shadowmask_tests;
#[cfg(test)]
mod spill_lock_tests;
#[cfg(test)]
mod spill_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod texture_and_page_tests;
#[cfg(test)]
mod write_tests;
