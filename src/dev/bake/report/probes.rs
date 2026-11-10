//! The light-probe bake's report (#1980): the count and bounds, the wall time of
//! each bounce, how the field is read (a grid trilinear, else the
//! `BLEND_PROBES` nearest blended: rusty has no tetrahedral mesh, so there are
//! no tetrahedra to count), and the dynamic objects sampling far from any probe,
//! which light from a probe across the room. [`Timeline`] times the stepped probe
//! job (`steps::LightingBake`) for both reports a probe bake writes: this one and
//! the reflection probes' (`report::reflections`).

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{entity_name, point, Kind, Report, Stages, TOP};
use crate::dev::bake::steps::Stage;
use crate::render::DEFAULT_BAKE_RESOLUTION;
use crate::scene::lighting::probe::{ProbeVolume, BLEND_PROBES};
use crate::scene::Scene;

/// A dynamic object whose nearest probe is farther than this, metres, samples
/// light from far away: twice the automatic placement's default spacing.
pub const FAR: f32 = 8.0;

/// The wall time of each stage of a probe bake: per bounce of the light probes,
/// per reflection probe. Stepped in the editor, a stage's time includes the
/// frames between its steps.
#[derive(Debug, Default)]
pub struct Timeline {
    last: Option<(Stage, Instant)>,
    pub bounces: Vec<Duration>,
    pub reflections: Vec<Duration>,
}

impl Timeline {
    /// The bake is at `stage` now: the time since the last note was the last stage's.
    pub fn note(&mut self, stage: Stage) {
        let now = Instant::now();
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
        list[i] += now - at;
    }

    /// The bounces' and the reflection probes' stages, in order.
    pub fn stages(&self) -> (Stages, Stages) {
        let (mut light, mut reflection) = (Stages::start(), Stages::start());
        for (b, took) in self.bounces.iter().enumerate() {
            light.push(&format!("bounce {}", b + 1), *took);
        }
        reflection.push("capture", self.reflections.iter().sum());
        (light, reflection)
    }
}

/// Write both reports of a finished probe bake of `scene` (saved at `path`), as
/// `timeline` timed it (#1980).
pub fn keep_both(scene: &Scene, path: &str, timeline: &Timeline) {
    let (light, reflection) = timeline.stages();
    super::keep(path, report(scene, &light));
    let dir = crate::dev::bake::reflection::reflection_dir(path);
    let times = &timeline.reflections;
    super::keep(
        path,
        super::reflections::report(scene, &reflection, times, &dir),
    );
}

/// The report of a light-probe bake of `scene` that ran `stages` (one a bounce).
pub fn report(scene: &Scene, stages: &Stages) -> Report {
    let volume = &scene.probes;
    let probes = volume.probes();
    let keys: String = probes.iter().map(|p| format!("{:?}", p.position)).collect();
    let fingerprint = scene_fingerprint(scene, &keys);
    // The baked SH and shadowmask occlusion each probe carries.
    let per_probe = std::mem::size_of::<crate::scene::lighting::sh::Sh9>() + 16;
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

/// The probes' bounds, `None` with none.
fn bounds(volume: &ProbeVolume) -> (Option<glam::Vec3>, Option<glam::Vec3>) {
    let at = volume.probes().iter().map(|p| p.position);
    (
        at.clone().reduce(glam::Vec3::min),
        at.reduce(glam::Vec3::max),
    )
}

/// The active dynamic drawn objects whose world-bounds centre is over [`FAR`] from
/// the nearest probe, farthest first (at most [`TOP`] listed), and their count.
pub(super) fn far_objects(scene: &Scene, volume: &ProbeVolume) -> Value {
    let world = &scene.world;
    let probes = volume.probes();
    let mut ids = world.ids_with_mesh();
    ids.sort_unstable();
    let dynamic =
        (ids.into_iter()).filter(|&id| !world.is_static(id) && world.is_active_in_hierarchy(id));
    let distance = |id: u32| {
        let (min, max) = world.mesh(id)?.world_aabb(world.local_to_world(id))?;
        let centre = (min + max) * 0.5;
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

/// A fingerprint of what a probe bake reads: `keys` (the probes as it reads
/// them), every active static mesh as the occlusion bake fingerprints it (its
/// triangles, world matrix and materials' occlusion), and every light.
pub fn scene_fingerprint(scene: &Scene, keys: &str) -> u64 {
    let mut hash = crate::core::hash::Content::default();
    hash.write(keys.as_bytes());
    let world = &scene.world;
    let mut ids = world.ids_with_mesh();
    ids.sort_unstable();
    for id in ids.into_iter().filter(|&id| world.is_static(id)) {
        let print = crate::scene::lighting::occlusion::fingerprint(scene, id).unwrap_or(0);
        hash.write(&id.to_le_bytes());
        hash.write(&print.to_le_bytes());
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
    hash.finish()
}
