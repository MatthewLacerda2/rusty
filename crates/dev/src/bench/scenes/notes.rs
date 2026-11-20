//! What a scene's report warns of beside its rows: the console errors the bench
//! logged (with the console kept beside the report), the bake the scene lacks, the
//! baked data on disk it ran without, the textures it drew uncompressed for want
//! of an import, and the models its loads cooked for want of one.

use std::path::Path;

use rusty_navigation::Start;
use rusty_render::StaleOcclusion;
use rusty_scene::{Scene, Sh9};
use rusty_script::scripting::LogLevel;

use super::super::super::harness::Harness;

/// Keep the bench's console as `<scene>.console.log` in `out_dir`, and the
/// warning for the errors it logged: a script failing every tick, or a sidecar
/// that failed to load, makes the scene look cheaper than the game is.
pub(super) fn keep_console(harness: &Harness, out_dir: &Path, scene: &str) -> Option<String> {
    rusty_logging::forward(&harness.console);
    let path = out_dir.join(format!("{scene}.console.log"));
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // lint: allow(asset-write) a bench report, not read by a path-keyed cache
    // lint: allow(stored-format) the console log, plain text
    let kept = std::fs::write(&path, harness.console_text()).is_ok();
    let console = harness.console.borrow();
    let errors: Vec<&str> = console
        .warnings()
        .iter()
        .filter(|(_, level)| *level == LogLevel::Error)
        .map(|(msg, _)| msg.as_str())
        .collect();
    let first = errors.first()?;
    let count: u64 = errors.iter().map(|m| repeats(m)).sum();
    let s = if count == 1 { "" } else { "s" };
    let at = match kept {
        true => format!("; the console is in {}", path.display()),
        false => String::new(),
    };
    Some(format!(
        "{count} console error{s} during the bench (first: {first}): the scene may not \
         run as the game does{at}"
    ))
}

/// How many times a kept console line stands for: `msg (×3)` is three.
fn repeats(msg: &str) -> u64 {
    msg.strip_suffix(')')
        .and_then(|m| m.rsplit_once(" (×"))
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or(1)
}

/// The warning for a scene benched without its baked lighting, naming what is
/// missing from what it places: no lightmaps, or lightmap pages the scene names
/// that are not on disk (colour, direction and shadowmask pages, counted); light
/// probes placed whose SH never loaded (every one zero: no sidecar, or one that
/// failed); reflection probes placed with no cube on disk. What a scene does not
/// place is not missing.
pub fn unbaked(scene: &Scene) -> Option<String> {
    let mut missing = Vec::new();
    let lightmaps = &scene.lightmaps;
    if !lightmaps.holds_bake() {
        missing.push("lightmaps".to_string());
    }
    let pages = [
        ("pages", &lightmaps.pages),
        ("direction pages", &lightmaps.directions),
        ("shadowmask pages", &lightmaps.masks),
    ];
    let absent: Vec<String> = pages
        .iter()
        .filter_map(|(what, named)| {
            let gone = named.iter().filter(|p| !Path::new(p).is_file()).count();
            (gone > 0).then(|| format!("{gone} of {} {what}", named.len()))
        })
        .collect();
    if !absent.is_empty() {
        missing.push(format!("lightmaps ({} not on disk)", absent.join(", ")));
    }
    let probes = scene.probes.probes();
    if !probes.is_empty() && probes.iter().all(|p| p.sh == Sh9::zero()) {
        missing.push("light probes (placed, no baked SH loaded)".into());
    }
    let reflections = &scene.reflection_probes.probes;
    let no_cube = reflections
        .iter()
        .filter(|p| p.cubemap_path.is_empty() || !Path::new(&p.cubemap_path).is_file())
        .count();
    if no_cube > 0 {
        missing.push(format!(
            "reflection probes ({no_cube} of {} with no cube on disk)",
            reflections.len()
        ));
    }
    (!missing.is_empty()).then(|| {
        format!(
            "not baked: {} (its lighting cost is not the shipped one)",
            missing.join(", ")
        )
    })
}

/// The warning for a scene whose textures `found` (`Renderer::uncached_textures`)
/// were uploaded uncompressed only because the project has no current import
/// cache for them: a headless run compresses none (`asset::import::import_all`,
/// #2319), so the scene's memory and load are not what the shipped game's are.
pub fn uncompressed(found: rusty_render::Uploads) -> Option<String> {
    let s = if found.count == 1 { "" } else { "s" };
    (found.count > 0).then(|| {
        format!(
            "uploaded uncompressed: {} texture{s}, {:.1} MB (their memory and load cost \
             is not the shipped one; `make import PROJECT=<dir>` makes their caches)",
            found.count,
            found.bytes as f64 / (1024.0 * 1024.0),
        )
    })
}

/// The warning for a scene whose timed loads cooked models (`models_cooked` > 0,
/// the most of any load): the project has no cooked file the read takes for them
/// (`asset::cook`: none made yet, or one made from another parse), so each load ran
/// the unwrap, the tangents and the clusters a cooked project reads from disk, and
/// `load.import` is not the shipped one.
pub fn cooked_at_load(loads: &[super::load::Load]) -> Option<String> {
    let cooked = |l: &super::load::Load| {
        let count = l.counts.iter().find(|(k, _)| k == "models_cooked");
        count.map_or(0.0, |(_, n)| *n)
    };
    let most = loads.iter().map(cooked).fold(0.0, f64::max);
    (most > 0.0).then(|| {
        format!(
            "its models were cooked at load ({most} in a load): `load.import` holds the \
             cooking a cooked project skips; `make import PROJECT=<dir>` writes their \
             cooked files"
        )
    })
}

/// The warnings for baked data a scene has on disk that its run did not use, one a
/// kind: a sidecar its load refused (`BakeFiles::refusals`, why included), a PVS
/// its statics moved away from (`stale`, `Renderer::stale_occlusion`), a navmesh
/// Play's start refused (`nav`, `NavigationGraph::last_start`). The scene was
/// measured as another one: nothing PVS-culled, or a navmesh baked at Play. A
/// scene that never had the artefact gets no line.
pub fn bakes_unused(
    scene: &Scene,
    stale: Option<StaleOcclusion>,
    nav: Option<Start>,
) -> Vec<String> {
    let mut out: Vec<String> = scene
        .bake_files()
        .refusals()
        .into_iter()
        .map(|(path, why)| {
            let kind = path
                .extension()
                .map_or("data".into(), |e| e.to_string_lossy());
            format!("baked {kind} not used: refused ({why})")
        })
        .collect();
    match stale {
        Some(s) if s.occluders > 0 => out.push(format!(
            "baked occlusion not used: stale ({} of the {} baked meshes changed since the \
             bake are occluders, so nothing is occlusion-culled; re-bake it)",
            s.occluders, s.changed
        )),
        Some(s) => out.push(format!(
            "baked occlusion partly used: stale ({} baked meshes changed since the bake are \
             always drawn; re-bake it)",
            s.changed
        )),
        None => {}
    }
    if nav == Some(Start::Refused) {
        out.push(
            "baked navmesh not used: refused at Play's start (stale or malformed; the console \
             says why), so Play baked it in full"
                .into(),
        );
    }
    out
}

#[cfg(test)]
#[path = "notes_tests.rs"]
mod tests;
