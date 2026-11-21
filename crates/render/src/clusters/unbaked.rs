//! crates/render/src/clusters/unbaked.rs — `Baked` lights no bake has taken in
//! yet (#2522): what the frame draws them as, and the console's word on them.
//!
//! A light is `Baked` unless it says otherwise, and a `Baked` light reaches
//! surfaces only through the lightmaps and probes, so a new one lights nothing
//! until Generate Lighting. A build with the editor or the dev layer (the editor,
//! the harness, a bench) previews it live until then, as a `Mixed` light draws
//! before its bake (`Scene::drawn_mode`); a shipped `player` draws what the bake
//! holds. Either way the console names the waiting lights once.

use std::collections::BTreeSet;

use rusty_components::{LightComponent, LightMode};
use rusty_scene::Scene;

/// Whether this build previews an unbaked `Baked` light live: one with the
/// editor or the dev layer, never a shipped `player`.
pub(crate) const PREVIEWS_UNBAKED: bool = cfg!(any(feature = "editor", feature = "dev"));

/// The mode the frame draws light `id` as, in this build.
pub(crate) fn drawn_mode(scene: &Scene, id: u32, light: &LightComponent) -> LightMode {
    scene.drawn_mode(id, light, PREVIEWS_UNBAKED)
}

/// How many waiting lights the note names before it says "and N more".
const NAMED: usize = 3;

/// The lights the console has named as waiting for a bake, so each is named once.
#[derive(Default)]
pub(crate) struct UnbakedNote {
    named: BTreeSet<u32>,
}

impl UnbakedNote {
    /// The console line for `scene`'s waiting lights, when one of them has not
    /// been named yet (`preview`: whether they are drawn live meanwhile). A light
    /// a bake took in and that waits again later (made `Baked` again, its id
    /// handed back by an Undo) is named again.
    pub(crate) fn note(&mut self, scene: &Scene, preview: bool) -> Option<String> {
        let waiting = scene.lights_awaiting_bake();
        self.named.retain(|id| waiting.contains(id));
        let new = waiting.iter().any(|id| !self.named.contains(id));
        self.named.extend(&waiting);
        new.then(|| message(scene, &waiting, preview))
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
    let meanwhile = match preview {
        true => "drawn live as a preview until then",
        false => "lighting nothing until then",
    };
    format!(
        "{count} Baked {lights} for a bake ({names}), {meanwhile}: run \
         Generate Lighting (Lighting.Generate()), or set a light that must move, \
         flash or switch at runtime to Realtime"
    )
}

#[cfg(test)]
#[path = "unbaked_tests.rs"]
mod tests;
