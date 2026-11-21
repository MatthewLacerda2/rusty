use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{entity_name, point, Kind, Report, Stages, TOP};
use crate::bake::steps::Stage;
use rusty_render::DEFAULT_BAKE_RESOLUTION;
use rusty_scene::lighting::probe::{ProbeVolume, BLEND_PROBES};
use rusty_scene::Scene;

pub const FAR: f32 = 8.0;

#[derive(Debug, Default)]
pub struct Timeline {
    last: Option<(Stage, Instant)>,
    pub bounces: Vec<Duration>,
    pub reflections: Vec<Duration>,
}

impl Timeline {
    pub fn note(&mut self, stage: Stage) {
        self.note_at(stage, Instant::now());
    }

    pub fn note_at(&mut self, stage: Stage, now: Instant) {
        let Some((was, at)) = self.last.replace((stage, now)) else {
            return;
        };
        let (list, i) = match was {
            Stage::Light { bounce, .. } => (&mut self.bounces, bounce.max(1) as usize - 1),
            Stage::Reflection { done, .. } => (&mut self.reflections, done),
            Stage::Done => return,
        };
        if list.len() <= i {
            list.resize(i + 1, Duration::ZERO);
        }
        list[i] += now.saturating_duration_since(at);
    }

    pub fn stages(&self) -> (Stages, Stages) {
        let (mut light, mut reflection) = (Stages::start(), Stages::start());
        for (b, took) in self.bounces.iter().enumerate() {
            light.push(&format!("bounce {}", b + 1), *took);
        }
        reflection.push("capture", self.reflections.iter().sum());
        (light, reflection)
    }
}

pub fn keep_both(scene: &Scene, path: &str, timeline: &Timeline) {
    let (light, reflection) = timeline.stages();
    let print = scene_fingerprint(scene);
    super::keep(path, report(scene, &light, print));
    let dir = crate::bake::reflection::reflection_dir(path);
    let times = &timeline.reflections;
    super::keep(
        path,
        super::reflections::report(scene, (&reflection, times), &dir, print),
    );
}

pub fn report(scene: &Scene, stages: &Stages, scene_print: u64) -> Report {
    let volume = &scene.probes;
    let probes = volume.probes();
    let keys: String = probes.iter().map(|p| format!("{:?}", p.position)).collect();
    let fingerprint = keyed(scene_print, &keys);
    let per_probe = std::mem::size_of::<rusty_scene::lighting::sh::Sh9>() + 16;
    let output = (probes.len() * per_probe) as u64;
    let settings = json!({ "resolution": DEFAULT_BAKE_RESOLUTION, "blendProbes": BLEND_PROBES });
    let (min, max) = bounds(volume);
    Report::new(Kind::Probes, stages, settings, fingerprint, output).with(json!({
        "count": probes.len(),
        "manual": probes.iter().filter(|p| p.manual).count(),
        "bounds": { "min": min.map(point), "max": max.map(point) },
        "bounces": stages.to_json(),
        "sampling": if volume.grid().is_some() { "grid" } else { "nearest" },
        "farObjects": far_objects(scene, volume),
    }))
}

fn bounds(volume: &ProbeVolume) -> (Option<glam::Vec3>, Option<glam::Vec3>) {
    let at = volume.probes().iter().map(|p| p.position);
    (
        at.clone().reduce(glam::Vec3::min),
        at.reduce(glam::Vec3::max),
    )
}

pub(super) fn far_objects(scene: &Scene, volume: &ProbeVolume) -> Value {
    let world = &scene.world;
    let probes = volume.probes();
    let mut ids = world.ids_with_mesh();
    ids.sort_unstable();
    let dynamic =
        (ids.into_iter()).filter(|&id| !world.is_static(id) && world.is_active_in_hierarchy(id));
    let distance = |id: u32| {
        let centre = super::drawn_centre(scene, id)?;
        let nearest = probes
            .iter()
            .map(|p| p.position.distance(centre))
            .reduce(f32::min);
        Some((id, centre, nearest.unwrap_or(f32::INFINITY)))
    };
    let mut far: Vec<_> = dynamic.filter_map(distance).filter(|f| f.2 > FAR).collect();
    far.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
    let listed = far.iter().take(TOP).map(|&(id, centre, d)| {
        let nearest = d.is_finite().then_some(d);
        json!({ "entity": id, "name": entity_name(scene, id), "centre": point(centre), "nearest": nearest })
    });
    json!({ "count": far.len(), "list": listed.collect::<Vec<_>>() })
}

pub fn scene_fingerprint(scene: &Scene) -> u64 {
    let mut hash = rusty_core::hash::Content::default();
    let world = &scene.world;
    let prints = rusty_lighting::occlusion::Fingerprints::of_scene(scene);
    let mut ids = world.ids_with_mesh();
    ids.sort_unstable();
    let drawn = |&id: &u32| world.is_static(id) && world.is_active_in_hierarchy(id);
    for id in ids.into_iter().filter(drawn) {
        hash.write(&id.to_le_bytes());
        match prints.of(scene, id) {
            Some((print, _)) => hash.write(&print.to_le_bytes()),
            None => raw_geometry(scene, id, &mut hash),
        }
        let slots = world.material(id).map_or(1, |m| m.materials.len().max(1));
        for slot in 0..slots {
            let material = scene.material_entry_at(id, slot).map(|(_, m)| m);
            hash.write(format!("{material:?}").as_bytes());
        }
    }
    let mut lights = world.ids_with_light();
    lights.sort_unstable();
    for id in lights {
        let light = world
            .light(id)
            .map(|l| format!("{:?}", &*l))
            .unwrap_or_default();
        hash.write(format!("{id}{light}{:?}", world.local_to_world(id)).as_bytes());
    }
    let environment = (
        (
            &scene.skybox_path,
            scene.ambient_color,
            scene.ambient_intensity,
        ),
        (&scene.ambient, &scene.sky, &scene.fog),
        (&*scene.lightmaps, &scene.lighting_settings),
    );
    let probes = scene.probes.probes().iter().map(|p| (p.position, p.manual));
    hash.write(format!("{:?}", probes.collect::<Vec<_>>()).as_bytes());
    hash.write(format!("{environment:?}").as_bytes());
    hash.finish()
}

fn raw_geometry(scene: &Scene, id: u32, hash: &mut rusty_core::hash::Content) {
    let matrix = scene.world.local_to_world(id).to_cols_array();
    hash.write(bytemuck::cast_slice(&matrix));
    if let Some(mesh) = scene.world.mesh(id) {
        mesh.vertices
            .iter()
            .for_each(|v| hash.write(bytemuck::cast_slice(&v.position)));
        hash.write(bytemuck::cast_slice(&mesh.indices));
    }
}

pub fn keyed(scene_print: u64, keys: &str) -> u64 {
    let mut hash = rusty_core::hash::Content::default();
    hash.write(&scene_print.to_le_bytes());
    hash.write(keys.as_bytes());
    hash.finish()
}
