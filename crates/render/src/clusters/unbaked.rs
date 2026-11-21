//! crates/render/src/clusters/unbaked.rs — `Baked` lights no bake has taken in
//! yet (#2522): what the frame draws them as, and the console's word on them.
//!
//! A light is `Baked` unless it says otherwise, and a `Baked` light reaches
//! surfaces only through the lightmaps and probes, so a new one lights nothing
//! until Generate Lighting. A build with the editor or the dev layer (the editor,
//! the harness, a bench) previews it live until then, as a `Mixed` light draws
//! before its bake (`Scene::drawn_mode`); a shipped `player` draws what the bake
//! holds. Either way the console names each waiting light once per scene.
//!
//! [`drawn_mode`] is the renderer's one reading of a light's mode: no other
//! renderer line calls `Scene::light_mode` (`tools/lint`'s `drawn-mode` rule).

use std::collections::BTreeSet;

use rusty_components::{LightComponent, LightMode};
use rusty_ecs::{Activity, Changes, Cursor};
use rusty_scene::revision::Revision;
use rusty_scene::{Scene, SceneId};

/// Whether this build previews an unbaked `Baked` light live: one with the
/// editor or the dev layer, never a shipped `player`.
pub(crate) const PREVIEWS_UNBAKED: bool = cfg!(any(feature = "editor", feature = "dev"));

/// The mode the frame draws light `id` as, in this build.
pub(crate) fn drawn_mode(scene: &Scene, id: u32, light: &LightComponent) -> LightMode {
    scene.drawn_mode(id, light, PREVIEWS_UNBAKED)
}

/// How many waiting lights the note names before it says "and N more".
const NAMED: usize = 3;

/// What the note was last brought up to date with: the scene, its bake's
/// record, and the change cursors of its lights and their activity.
struct Seen {
    scene: SceneId,
    lightmaps: Revision,
    light: Cursor<LightComponent>,
    activity: Cursor<Activity>,
}

/// The lights the console has named as waiting for a bake, so each is named once
/// per scene, and what they were read from. Kept, not walked: the lights are read
/// again only when a light or an activity changed, or the bake's record did.
#[derive(Default)]
pub(crate) struct UnbakedNote {
    seen: Option<Seen>,
    named: BTreeSet<u32>,
}

impl UnbakedNote {
    /// The console line for `scene`'s waiting lights, when one of them has not
    /// been named yet (`preview`: whether they are drawn live meanwhile). A light
    /// is named once per scene, whatever its activity or intensity does after; a
    /// bake (a new record) or another scene names afresh what still waits.
    pub(crate) fn note(&mut self, scene: &Scene, preview: bool) -> Option<String> {
        if !self.changed(scene) {
            return None;
        }
        let waiting = scene.lights_awaiting_bake();
        let new = waiting.iter().any(|id| !self.named.contains(id));
        self.named.extend(&waiting);
        new.then(|| message(scene, &waiting, preview))
    }

    /// Whether a light may have started waiting since the last read: another
    /// scene or bake record (which forgets the names), a light's change, or an
    /// activity change (a parent's reaches its children).
    fn changed(&mut self, scene: &Scene) -> bool {
        let world = &scene.world;
        let (id, lightmaps) = (scene.id(), scene.lightmaps.revision());
        let Some(seen) = (self.seen.as_mut()).filter(|s| s.scene == id && s.lightmaps == lightmaps)
        else {
            self.seen = Some(Seen {
                scene: id,
                lightmaps,
                light: world.change_cursor(),
                activity: world.change_cursor(),
            });
            self.named.clear();
            return true;
        };
        let named = &self.named;
        let light = match world.changes(&mut seen.light) {
            Changes::Rebuild => true,
            // A set lookup per changed id: a live light flickering every frame
            // is no waiting one, and walks nothing.
            Changes::Ids(ids) => ids.iter().any(|&id| {
                !named.contains(&id) && world.light(id).is_some_and(|l| scene.awaits_bake(id, &l))
            }),
        };
        let active = !world.changes(&mut seen.activity).is_empty();
        light | active
    }
}

impl crate::Renderer {
    /// Name `scene`'s `Baked` lights no bake has taken in yet in the console, once
    /// each; a preview or a bake capture (an aside render) says nothing.
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
    let lights = if count == 1 {
        "light waits"
    } else {
        "lights wait"
    };
    match preview {
        true => format!(
            "{count} Baked {lights} for a bake ({names}), drawn live as a preview \
             until then: run Generate Lighting (Lighting.Generate()), or set a light \
             that must move, flash or switch at runtime to Realtime"
        ),
        // The player has no bake verb: it can only say what the build shipped.
        false => format!(
            "{count} Baked {lights} for a bake ({names}): the scene shipped without \
             a bake that covers {them}, so {they} light nothing",
            them = if count == 1 { "it" } else { "them" },
            they = if count == 1 { "it" } else { "they" },
        ),
    }
}

#[cfg(test)]
#[path = "unbaked_tests.rs"]
mod tests;
