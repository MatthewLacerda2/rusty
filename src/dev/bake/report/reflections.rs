//! The reflection-probe bake's report (#1980): per probe its resolution, box,
//! cube file bytes, bake time and the probes its box overlaps; the resolution is
//! the one the bake rendered at (`clamp_resolution`); then **which probe
//! each drawn object picks**, at its bounds' centre as the renderer asks
//! (`ReflectionProbeSet::blend`). That one list shows an L-shaped floor reflecting
//! a single probe everywhere. The picks rank the scene's whole set: a probe the
//! memory budget leaves out at runtime is not skipped here.

use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use super::{bytes_under, entity_name, point, secs, Kind, Report, Stages};
use crate::render::ibl::reflection_bake::cube_file;
use crate::scene::lighting::reflection_probe::clamp_resolution;
use crate::scene::{ReflectionProbeSet, Scene};

/// The report of a bake of `scene`'s reflection probes into `dir` that ran
/// `stages`, each probe taking `times[i]` (empty when they were not timed one by
/// one), and read the scene `scene_print` (`probes::scene_fingerprint`).
pub fn report(
    scene: &Scene,
    (stages, times): (&Stages, &[Duration]),
    dir: &Path,
    scene_print: u64,
) -> Report {
    let set = &scene.reflection_probes;
    let settings = scene.lighting_settings.reflections;
    let fingerprint = super::probes::keyed(scene_print, &probe_keys(set));
    let output = bytes_under(dir);
    let default = settings.resolution;
    let probe = |(i, p): (
        usize,
        &crate::scene::lighting::reflection_probe::ReflectionProbe,
    )| {
        let overlaps = (set.probes.iter().enumerate())
            .filter(|&(j, q)| j != i && boxes_overlap(p.box_min, p.box_max, q.box_min, q.box_max))
            .map(|(j, _)| j);
        json!({
            "index": i,
            "position": point(p.position),
            "boxMin": point(p.box_min),
            "boxMax": point(p.box_max),
            "resolution": clamp_resolution(p.resolution.unwrap_or(default)),
            "importance": p.importance,
            "manual": p.manual,
            "bytes": bytes_under(&cube_file(dir, i)),
            "seconds": times.get(i).map(|d| secs(*d)),
            "overlaps": overlaps.collect::<Vec<_>>(),
        })
    };
    let probes: Vec<Value> = set.probes.iter().enumerate().map(probe).collect();
    Report::new(Kind::Reflections, stages, settings, fingerprint, output).with(json!({
        "count": set.len(),
        "probes": probes,
        "picks": picks(scene, set),
    }))
}

/// What a probe bake reads of each probe, for the input fingerprint.
fn probe_keys(set: &ReflectionProbeSet) -> String {
    let key = |p: &crate::scene::lighting::reflection_probe::ReflectionProbe| {
        let clip = (p.near_clip, p.far_clip, p.shadows);
        format!(
            "{:?}{:?}{:?}{:?}{clip:?}",
            p.position, p.box_min, p.box_max, p.resolution
        )
    };
    set.probes.iter().map(key).collect()
}

/// Whether two boxes share more than a face.
fn boxes_overlap(a0: glam::Vec3, a1: glam::Vec3, b0: glam::Vec3, b1: glam::Vec3) -> bool {
    a0.cmplt(b1).all() && b0.cmplt(a1).all()
}

/// Per active drawn object, by id: its centre as the renderer takes it
/// ([`super::drawn_centre`]) and the probes it
/// reflects with their weights (`probe: null` is the sky).
pub(super) fn picks(scene: &Scene, set: &ReflectionProbeSet) -> Value {
    let world = &scene.world;
    let mut ids = world.ids_with_mesh();
    ids.sort_unstable();
    let pick = |id: u32| {
        let centre = super::drawn_centre(scene, id)?;
        let blend = set.blend(centre);
        Some(json!({
            "entity": id,
            "name": entity_name(scene, id),
            "centre": point(centre),
            "probe": blend.map(|b| b.first),
            "weight": blend.map_or(0.0, |b| b.first_weight),
            "second": blend.and_then(|b| b.second),
            "secondWeight": blend.map_or(0.0, |b| b.second_weight),
        }))
    };
    let active = ids
        .into_iter()
        .filter(|&id| world.is_active_in_hierarchy(id));
    Value::Array(active.filter_map(pick).collect())
}
