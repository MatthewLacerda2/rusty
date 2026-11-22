use std::collections::{BTreeMap, BTreeSet};

use rusty_components::{LightComponent, LightMode};
use rusty_ecs::{Activity, Changes, Cursor};
use rusty_scene::revision::Revision;
use rusty_scene::{Scene, SceneId};

/// Whether this build previews an unbaked `Baked` light live: one with the
/// editor or the dev layer, never a shipped `player`.
pub(crate) const PREVIEWS_UNBAKED: bool = cfg!(any(feature = "editor", feature = "dev"));

pub(crate) fn drawn_mode(scene: &Scene, id: u32, light: &LightComponent) -> LightMode {
    scene.drawn_mode(id, light, PREVIEWS_UNBAKED)
}

const NAMED: usize = 3;

type Record = (BTreeMap<u32, LightMode>, BTreeSet<u32>);

fn record(scene: &Scene) -> Record {
    (
        scene.lightmaps.baked_as.clone(),
        scene.lightmaps.fell_back.clone(),
    )
}

fn same_record(scene: &Scene, kept: &Record) -> bool {
    scene.lightmaps.baked_as == kept.0 && scene.lightmaps.fell_back == kept.1
}

struct Seen {
    scene: SceneId,
    lightmaps: Revision,
    record: Record,
    light: Cursor<LightComponent>,
    activity: Cursor<Activity>,
}

#[derive(Default)]
pub(crate) struct UnbakedNote {
    seen: Option<Seen>,
    named: BTreeSet<u32>,
}

impl UnbakedNote {
    pub(crate) fn note(&mut self, scene: &Scene, preview: bool) -> Option<String> {
        if !self.changed(scene) {
            return None;
        }
        let waiting = scene.lights_awaiting_bake();
        let new = waiting.iter().any(|id| !self.named.contains(id));
        self.named.extend(&waiting);
        new.then(|| message(scene, &waiting, preview))
    }

    fn changed(&mut self, scene: &Scene) -> bool {
        let world = &scene.world;
        let (id, revision) = (scene.id(), scene.lightmaps.revision());
        let kept = (self.seen.as_mut())
            .filter(|s| s.scene == id)
            .filter(|s| s.lightmaps == revision || same_record(scene, &s.record));
        let Some(seen) = kept else {
            self.seen = Some(Seen {
                scene: id,
                lightmaps: revision,
                record: record(scene),
                light: world.change_cursor(),
                activity: world.change_cursor(),
            });
            self.named.clear();
            return true;
        };
        seen.lightmaps = revision;
        let named = &self.named;
        let fresh = |changes: Changes| match changes {
            Changes::Rebuild => true,
            Changes::Ids(ids) => {
                (ids.iter()).any(|&id| !named.contains(&id) && scene.waits_for_bake(id))
            }
        };
        let light = fresh(world.changes(&mut seen.light));
        let active = fresh(world.changes(&mut seen.activity));
        light | active
    }
}

impl crate::Renderer {
    pub(crate) fn note_unbaked(&mut self, scene: &Scene) {
        if self.is_aside() {
            return;
        }
        if let Some(line) = self.unbaked.note(scene, PREVIEWS_UNBAKED) {
            log::warn!("[Lighting] {line}");
        }
    }
}

fn message(scene: &Scene, waiting: &[u32], preview: bool) -> String {
    let mut names: Vec<String> = (waiting.iter().take(NAMED))
        .map(|&id| match scene.world.name(id) {
            Some(name) => format!("`{}`", &*name),
            None => format!("#{id}"),
        })
        .collect();
    if waiting.len() > NAMED {
        names.push(format!("{} more", waiting.len() - NAMED));
    }
    let (count, names) = (waiting.len(), names.join(", "));
    let one = count == 1;
    let lights = if one { "light waits" } else { "lights wait" };
    let (it, they) = if one { ("it", "it") } else { ("them", "they") };
    let head = format!("{count} Baked {lights} for a bake ({names})");
    if !preview {
        // The player has no bake verb: it can only say what the build shipped.
        return format!(
            "{head}: the scene shipped without a bake that covers {it}, so {they} \
             light nothing; a light that only exists at runtime (a prefab spawned \
             during play) says Realtime"
        );
    }
    let unrecorded = waiting
        .iter()
        .any(|id| !scene.lightmaps.baked_as.contains_key(id));
    let probes = match !scene.probes.is_empty() && unrecorded {
        true => {
            " over its probe bake (probe-lit objects can read brighter, the probes \
                  may hold its light already)"
        }
        false => "",
    };
    let advice = if scene.is_playing() {
        format!(
            "a light spawned during Play is in no bake: make it Realtime (a \
             prefab's light says so in its file), or bake the scene with {it} in it"
        )
    } else if scene.lightmaps.holds_bake() {
        format!(
            "run Generate Lighting (Lighting.Generate()) with {it} active and above \
             intensity 0, or set a light that must move, flash or switch at runtime \
             to Realtime"
        )
    } else {
        "run Generate Lighting (Lighting.Generate()), or set a light that must move, \
         flash or switch at runtime to Realtime"
            .to_string()
    };
    format!("{head}, drawn live as a preview{probes} until then: {advice}")
}

#[cfg(test)]
#[path = "unbaked_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "unbaked_kept_tests.rs"]
mod kept_tests;
