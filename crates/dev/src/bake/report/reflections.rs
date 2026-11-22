use std::path::Path;
use std::time::Duration;

use serde_json::{json, Value};

use super::{bytes_under, entity_name, point, secs, Kind, Report, Stages};
use rusty_render::ibl::reflection_bake::cube_file;
use rusty_scene::lighting::reflection_probe::clamp_resolution;
use rusty_scene::{ProbeShape, ReflectionProbe, ReflectionProbeSet, Scene};

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
    let probe = |(i, p): (usize, &ReflectionProbe)| {
        let overlaps = (set.probes.iter().enumerate())
            .filter(|&(j, q)| j != i && p.overlaps(q))
            .map(|(j, _)| j);
        let mut entry = json!({
            "index": i,
            "position": point(p.position),
            "shape": format!("{:?}", p.shape),
            "resolution": clamp_resolution(p.resolution.unwrap_or(default)),
            "importance": p.importance,
            "manual": p.manual,
            "bytes": bytes_under(&cube_file(dir, i)),
            "seconds": times.get(i).map(|d| secs(*d)),
            "overlaps": overlaps.collect::<Vec<_>>(),
        });
        if let Some(o) = entry.as_object_mut() {
            o.extend(volume(p));
        }
        entry
    };
    let probes: Vec<Value> = set.probes.iter().enumerate().map(probe).collect();
    Report::new(Kind::Reflections, stages, settings, fingerprint, output).with(json!({
        "count": set.len(),
        "probes": probes,
        "picks": picks(scene, set),
    }))
}

fn volume(p: &ReflectionProbe) -> Vec<(String, Value)> {
    let rows = match p.shape {
        ProbeShape::Box => vec![
            ("boxMin", point(p.box_min)),
            ("boxMax", point(p.box_max)),
            ("rotation", json!(p.rotation.to_array())),
        ],
        ProbeShape::Sphere => vec![("radius", json!(p.radius))],
    };
    rows.into_iter().map(|(k, v)| (k.to_owned(), v)).collect()
}

fn probe_keys(set: &ReflectionProbeSet) -> String {
    let key = |p: &ReflectionProbe| {
        let clip = (p.near_clip, p.far_clip, p.shadows);
        let volume = (p.box_min, p.box_max, p.rotation, p.shape, p.radius);
        format!("{:?}{volume:?}{:?}{clip:?}", p.position, p.resolution)
    };
    set.probes.iter().map(key).collect()
}

pub(super) fn picks(scene: &Scene, set: &ReflectionProbeSet) -> Value {
    let world = &scene.world;
    let mut ids = world.ids_with_mesh();
    ids.sort_unstable();
    let prepared = set.prepared();
    let pick = |id: u32| {
        let centre = super::drawn_centre(scene, id)?;
        let blend = prepared.blend(centre);
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
