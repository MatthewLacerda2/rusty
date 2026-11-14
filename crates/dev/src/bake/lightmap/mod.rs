//! crates/dev/src/bake/lightmap/ — the lightmap bake as an authoring action (#438).
//!
//! The one path behind `Lighting.BakeLightmaps()` and the lightmap step of the
//! editor's Generate Lighting button (#832), so the two never drift. It gathers the
//! static scene, runs the CPU bake (`scene::lighting::lightmap`, pure and seeded),
//! writes its atlas pages beside the scene, and points the scene's
//! `lightmaps` at them. Saving the scene (or Generate Lighting, which writes them)
//! then persists those references. Headless: no GPU is involved.
//!
//! The lightmaps are packed into atlas pages (`lightmap::atlas`) and written by
//! `write`: each page as `<scene file>.lightmaps/lightmap_<page>_<tag>.ktx2` (BC6H
//! with its mips, #1694, what the scene references and the renderer loads) and the
//! RGBM PNG of its base level beside it under the same name (the fallback for a GPU
//! without BC, and what an agent can open). One folder per scene, emptied of old
//! pages once the new ones are written (`write`), so a rebake never leaves stale
//! ones behind and a failed write leaves the old ones. `<tag>` hashes the page,
//! so a rebake that changes a page changes its path and the renderer, which caches
//! pages by path, loads the new one. A directional bake (#810) writes each page's
//! direction page beside it as `lightmap_dir_<page>_<tag>.ktx2` (RGBA8 with the
//! bake's mips, never block-compressed, #1996), and a `Shadowmask` bake (#1695)
//! its shadowmask page as `lightmap_mask_<page>_<tag>.png` (RGBA8, one channel
//! per masked light; the loader makes its mips).
//!
//! The bake splits in three so the editor can run the middle on a worker thread
//! ([`job`], #808): [`gather_bake_input`] borrows the scene, `bake` needs only
//! the owned [`BakeScene`], and [`apply_lightmaps`] writes the result back. A script's
//! bake runs the middle on a worker too, so its caller can log progress (`logged`,
//! #1667). Either way the bake writes its report beside the scene
//! (`report::lightmaps`, #1980).
//!
//! Allowed deps: scene (the bake and its data), asset (the KTX2 page), image (PNG
//! I/O).

use std::path::PathBuf;
use std::sync::Arc;

use glam::Vec3;

use crate::bake::denoise::{self, Oidn};
use crate::bake::progress::{BakeLog, MeshCost, MeshTime, Step};
use crate::bake::report::{self, lightmaps::Measured, Stages};
use rusty_asset::MapRef;
use rusty_lighting::lightmap::{BakeProgress, BakeScene};
use rusty_scene::lighting::settings::BakeSettings;
use rusty_scene::Scene;

mod encoder;
pub mod job;
pub(in crate::bake) mod logged;
mod measure;
mod pages;
mod preview;
mod write;
pub use job::{BakeOutcome, LightmapBakeJob};
pub use pages::{encode_pages, EncodedPages};
#[cfg(test)]
use write::content_tag;
pub use write::{apply_encoded, apply_lightmaps};

/// The lightmap folder for a scene: `foo.scene` → `foo.scene.lightmaps/`. What
/// lies in it is a baked page, which the texture importer leaves alone
/// (`asset::lightmap_page::is_baked`, #1694).
pub fn lightmap_dir(scene_path: &str) -> PathBuf {
    PathBuf::from(format!(
        "{scene_path}{}",
        rusty_asset::lightmap_page::DIR_SUFFIX
    ))
}

/// Bake every static mesh with a lightmap UV and point the scene at the new pages.
/// Returns how many lightmaps were written (0 when nothing is lightmappable, which
/// also clears the scene's old references). Errors when the scene was never saved
/// (there is nowhere to write) or a file cannot be written.
pub fn bake_scene_lightmaps(
    scene: &mut Scene,
    scene_path: Option<&str>,
    settings: &BakeSettings,
) -> Result<usize, String> {
    bake_scene_lightmaps_logged(scene, scene_path, settings, &mut BakeLog::off())
}

/// [`bake_scene_lightmaps`], reporting its progress into `log` as it goes (#1667).
pub fn bake_scene_lightmaps_logged(
    scene: &mut Scene,
    scene_path: Option<&str>,
    settings: &BakeSettings,
    log: &mut BakeLog,
) -> Result<usize, String> {
    let scene_path = scene_path.ok_or(UNSAVED)?;
    let mut stages = Stages::start();
    let input = gather_bake_input(scene);
    stages.lap("gather");
    let run = logged::bake_logged(&input, settings, log)?;
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

/// Write the report of a bake of `scene` (#1980): it read `input` with its
/// settings, ran `stages`, and made `pages`, now written beside `scene_path`.
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

/// `progress` lending the bake OIDN when `settings` pick it and it opens (#1884,
/// [`denoise::lent`], which warns once when it is missing), and the [`Oidn`]
/// lent, whose time the log reads after.
pub(super) fn lend_denoiser(
    progress: BakeProgress,
    settings: &BakeSettings,
) -> (BakeProgress, Option<Arc<Oidn>>) {
    match denoise::lent(settings) {
        Some(oidn) => (progress.with_denoiser(oidn.clone()), Some(oidn)),
        None => (progress, None),
    }
}

/// A timed mesh as the log names it: its entity's name, or `#<id>` without one.
fn mesh_cost(scene: &Scene, time: &MeshTime) -> MeshCost {
    let name = scene.world.name(time.entity).map(|n| n.to_string());
    MeshCost {
        name: name.unwrap_or_else(|| format!("#{}", time.entity)),
        texels: time.texels,
        busy: time.busy,
    }
}

/// The static meshes asking for a lightmap (`BakeMesh::receives_lightmap`): not a
/// `ShadowsOnly` one nor one at Scale In Lightmap 0 (#1704), which get none by
/// design. What [`missing_uv_report`] counts against.
pub fn lightmap_receivers(input: &BakeScene) -> usize {
    input
        .meshes
        .iter()
        .filter(|m| m.receives_lightmap())
        .count()
}

/// The bake summary's one-time nudge (#831): how many static meshes got no lightmap
/// for want of a lightmap UV, or `None` when every one got one. `static_meshes`
/// counts those asking for one ([`lightmap_receivers`]); each with a usable
/// lightmap UV bakes exactly one lightmap, so the difference is the rest.
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

/// Log [`missing_uv_report`], once per bake.
fn warn_missing_uvs(static_meshes: usize, lightmaps: usize) {
    if let Some(report) = missing_uv_report(static_meshes, lightmaps) {
        log::warn!("[LightmapBake] {report}");
    }
}

/// Why a bake cannot start: there is nowhere beside the scene to write.
pub const UNSAVED: &str = "save the scene before baking lightmaps";

/// The static scene flattened for the bake: owned data, free of the ECS.
pub fn gather_bake_input(scene: &Scene) -> BakeScene {
    BakeScene::gather(scene, &texture_average)
}

/// A texture's average linear colour (its texels are sRGB), for the bake's albedo
/// and emission. `None` when it cannot be read; the material colour then stands alone.
fn texture_average(map: &MapRef) -> Option<Vec3> {
    let file = map.file()?;
    let rgba = image::open(file).ok()?.to_rgba8();
    let count = (rgba.width() * rgba.height()).max(1) as f32;
    let to_linear = rusty_core::color::srgb_to_linear;
    let sum = rgba.pixels().fold(Vec3::ZERO, |acc, p| {
        acc + Vec3::new(to_linear(p[0]), to_linear(p[1]), to_linear(p[2]))
    });
    Some(sum / count)
}

#[cfg(test)]
mod job_tests;
#[cfg(test)]
mod limits_tests;
#[cfg(test)]
mod logged_tests;
#[cfg(test)]
mod preview_edit_tests;
#[cfg(test)]
mod preview_tests;
#[cfg(test)]
mod report_tests;
#[cfg(test)]
mod shadowmask_tests;
#[cfg(test)]
mod survivor_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod write_tests;
