use std::path::Path;

use rusty_components::{LightMode, LightType};
use rusty_navigation::Start;
use rusty_render::StaleOcclusion;
use rusty_scene::{Scene, Sh9};
use rusty_script::scripting::LogLevel;

use super::super::super::harness::Harness;

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

fn repeats(msg: &str) -> u64 {
    msg.strip_suffix(')')
        .and_then(|m| m.rsplit_once(" (×"))
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or(1)
}

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lights {
    pub total: u32,
    pub live: u32,
    pub mixed: u32,
}

pub fn lights(scene: &Scene) -> Lights {
    let mut out = Lights::default();
    for id in scene.world.ids_with_light() {
        let Some(light) = scene.world.light(id) else {
            continue;
        };
        if light.light_type == LightType::Ambient || !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let mode = scene.light_mode(id, &light);
        out.total += 1;
        out.live += u32::from(mode.renders_live(false));
        out.mixed += u32::from(mode == LightMode::Mixed);
    }
    out
}

pub(super) fn push_lights(report: &mut super::SceneReport, lights: Lights) {
    report.push_value("level_lights", f64::from(lights.total));
    report.push_value("level_lights_live", f64::from(lights.live));
    report.warnings.extend(mixed_lights(lights));
}

pub fn mixed_lights(lights: Lights) -> Option<String> {
    let Lights { total, mixed, .. } = lights;
    let s = if total == 1 { "" } else { "s" };
    let are = if mixed == 1 { "is" } else { "are" };
    (mixed > 0).then(|| {
        format!(
            "{mixed} of {total} light{s} {are} Mixed: their direct light is shaded live every \
             frame"
        )
    })
}

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

pub fn scene_bakes(
    harness: &Harness,
    own: &Path,
    loaded: rusty_scene::SceneId,
    stale: Option<StaleOcclusion>,
) -> Vec<String> {
    let world = harness.world.borrow();
    let live = world.scene().borrow();
    let mut out: Vec<String> = unbaked(&live).into_iter().collect();
    let nav = world.nav().borrow();
    let nav = nav.tracks(&live).then(|| nav.last_start()).flatten();
    let path = world.script_manager().scene_path_cell().borrow().clone();
    let path = path.unwrap_or_else(|| own.to_string_lossy().into_owned());
    let stale = stale.filter(|_| live.id() == loaded);
    out.extend(bakes_unused(&live, &path, stale, nav));
    out
}

pub fn bakes_unused(
    scene: &Scene,
    path: &str,
    stale: Option<StaleOcclusion>,
    nav: Option<Start>,
) -> Vec<String> {
    let name = |p: &Path| {
        p.file_name().map_or(p.display().to_string(), |n| {
            n.to_string_lossy().into_owned()
        })
    };
    let occlusion = rusty_scene::lighting::occlusion::occlusion_path(path);
    let navmesh = rusty_scene::nav_settings::navmesh_path(path);
    let mut out = Vec::new();
    for (kind, file) in [("occlusion", &occlusion), ("navmesh", &navmesh)] {
        if let Some(why) = scene.bake_files().not_used(file) {
            out.push(format!("baked {kind} not used: {} {why}", name(file)));
        }
    }
    let pvs = name(&occlusion);
    match stale {
        Some(s) if s.occluders > 0 => out.push(format!(
            "baked occlusion not used: {pvs} stale ({} of the {} baked meshes changed \
             since the bake are occluders, so nothing is occlusion-culled; re-bake it)",
            s.occluders, s.changed
        )),
        Some(s) => out.push(format!(
            "baked occlusion partly used: {pvs} stale ({} baked meshes changed since the \
             bake are always drawn; re-bake it)",
            s.changed
        )),
        None => {}
    }
    if let Some(Start::Refused(why)) = nav {
        out.push(format!(
            "baked navmesh not used: {} refused at Play's start ({why}), so Play baked it \
             in full",
            name(&navmesh)
        ));
    }
    out
}

#[cfg(test)]
#[path = "baked_tests.rs"]
mod baked_tests;
#[cfg(test)]
#[path = "lights_tests.rs"]
mod lights_tests;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
