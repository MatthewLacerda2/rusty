//! crates/render/src/stats/switches.rs — the renderer's measurement switches:
//! each scene input the forward pass reads that has no player option, turned off
//! for one run (`bench-scenes --set render.<switch>=false`), so a GPU-cost
//! difference between two builds can be bisected by feature.
//!
//! A switch never changes a pipeline or a bind group layout: what the shader
//! reads is bound to the stand-in it reads when the scene has none, so a build
//! that renumbers its groups measures the same way. Set only through
//! [`Renderer::set_switches`] (the `dev` feature); a game never sets one, and
//! nothing is stored.
//!
//! | switch | what goes | what the forward pass reads instead |
//! |---|---|---|
//! | `shadows` | the cascades and the point/spot atlas tiles drawn (group 3, bindings 0, 1, 4, 5) | a cascade count of 0 and no light's tile, as a probe captured without shadows |
//! | `lightmaps` | the colour, direction and shadowmask pages bound (group 0, bindings 14, 16, 19) | the 1x1 stand-in pages, each object's `lightmap_page` 0 |
//! | `reflection_probes` | the resident probes' cube array (group 0, binding 4) | the 1-cube stand-in, each instance's `reflection_probe` 0 (the sky's cube) |
//! | `clustered_lights` | the cluster light lists (group 0, bindings 8 and 9) | every cluster's light range empty; the lights stay uploaded at binding 7 |
//! | `ssao` | the SSAO pass, in every view (the Scene view's too) | the 1x1 white AO, as with `GraphicsOption::Ssao` off |
//! | `ssil` | the SSIL pass, in every view | no indirect term, as with `GraphicsOption::Ssil` off |
//!
//! `lightmaps` takes only the bound pages: the bake's light modes stay (a `Baked`
//! light still draws no realtime light, so what only its pages lit goes dark; a
//! `Mixed` one keeps the mode the bake recorded), and so do the shadowmask
//! channels it assigned (with no mask bound, a masked light's static casters are
//! drawn into its shadow maps, as under Distance Shadowmask) and the light
//! probes' SH and occlusion. `clustered_lights` leaves the lamps' shadow tiles
//! drawn into the atlas; `shadows` takes those.
//!
//! `ssao` and `ssil` reach the editor's Scene view, which ignores the player's
//! graphics options (`option.ssao` reaches only the game views).
//!
//! One more key changes no input but when the GPU starts: `flushes`
//! ([`EarlyFlushes`], `--set render.flushes=none|occlusion|occlusion_scene`)
//! picks how many of `draw::camera::EARLY_FLUSHES`' points a camera's commands go
//! out early at. Every shape draws the same pixels; only the submits move.

#[cfg(feature = "dev")]
use crate::Renderer;
use rusty_core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use rusty_scene::lighting::lightmap_set::LightmapSet;

rusty_core::named_enum! {
    /// One measurement switch, named as `--set render.<name>` spells it.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum RenderSwitch {
        /// The sun's cascades and the point/spot shadow atlas.
        Shadows = "shadows",
        /// The scene's baked lightmap pages.
        Lightmaps = "lightmaps",
        /// The scene's baked reflection probes.
        ReflectionProbes = "reflection_probes",
        /// The point, spot and area lights in the cluster lists.
        ClusteredLights = "clustered_lights",
        /// Screen-space ambient occlusion, in every view.
        Ssao = "ssao",
        /// Screen-space indirect lighting, in every view.
        Ssil = "ssil",
    }
}

rusty_core::named_enum! {
    /// How many of a camera's early flush points (`draw::camera::EARLY_FLUSHES`,
    /// in its order) send what the camera recorded so far, as `--set
    /// render.flushes=<name>` spells it: a measurement of the GPU's start.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum EarlyFlushes {
        /// None: one submit at the camera's end.
        None = "none",
        /// After the depth work (the shadow sweep, the fog, the occlusion stage).
        Occlusion = "occlusion",
        /// After the depth work and after the opaque scene pass: what every
        /// renderer draws with.
        #[default]
        OcclusionScene = "occlusion_scene",
    }
}

impl EarlyFlushes {
    /// How many of the points, from the first, it flushes at.
    pub(crate) fn count(self) -> usize {
        self as usize
    }
}

/// Which [`RenderSwitch`]es are on: all of them unless a measurement turned one
/// off; and the [`EarlyFlushes`] a camera sends at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderSwitches {
    off: [bool; RenderSwitch::ALL.len()],
    flushes: EarlyFlushes,
}

impl RenderSwitches {
    /// Every switch on and the default flushes: what every renderer draws with.
    pub const ON: Self = Self {
        off: [false; RenderSwitch::ALL.len()],
        flushes: EarlyFlushes::OcclusionScene,
    };

    /// Where a camera's commands go out early.
    pub fn flushes(self) -> EarlyFlushes {
        self.flushes
    }

    /// Send a camera's commands early at `flushes`' points.
    pub fn set_flushes(&mut self, flushes: EarlyFlushes) {
        self.flushes = flushes;
    }

    /// Whether `switch` is on.
    pub fn on(self, switch: RenderSwitch) -> bool {
        !self.off[switch as usize]
    }

    /// Turn `switch` on or off.
    pub fn set(&mut self, switch: RenderSwitch, on: bool) {
        self.off[switch as usize] = !on;
    }

    /// `items` while `switch` is on, else none: the list the renderer places or
    /// bins (the reflection probes, the clustered lights).
    pub(crate) fn listed<T>(self, switch: RenderSwitch, items: &[T]) -> &[T] {
        if self.on(switch) {
            items
        } else {
            &[]
        }
    }

    /// The colour, direction and shadowmask page lists the renderer binds for
    /// `set`: none with the `lightmaps` switch off, as for an unbaked scene.
    pub(crate) fn lightmap_lists(self, set: &LightmapSet) -> [&[String]; 3] {
        match self.on(RenderSwitch::Lightmaps) {
            true => [&set.pages, &set.directions, &set.masks],
            false => [&[]; 3],
        }
    }

    /// Whether the frame's lights cast shadows: `capture_shadows` (false in a
    /// reflection probe captured without them) and the `shadows` switch on.
    pub(crate) fn casts_shadows(self, capture_shadows: bool) -> bool {
        capture_shadows && self.on(RenderSwitch::Shadows)
    }

    /// `options` with the effects whose switch is off (`ssao`, `ssil`) turned off
    /// too: what every view, the Scene view's included, runs its post-FX with.
    pub(crate) fn gate(self, mut options: GraphicsOptions) -> GraphicsOptions {
        for (switch, option) in [
            (RenderSwitch::Ssao, GraphicsOption::Ssao),
            (RenderSwitch::Ssil, GraphicsOption::Ssil),
        ] {
            if !self.on(switch) {
                options.set(option, false);
            }
        }
        options
    }

    /// Whether every switch is on, at the default flushes.
    pub fn all_on(self) -> bool {
        self == Self::ON
    }
}

impl Default for RenderSwitches {
    fn default() -> Self {
        Self::ON
    }
}

#[cfg(feature = "dev")]
impl Renderer {
    /// Draw from the next frame with `switches` (the module's table).
    pub fn set_switches(&mut self, switches: RenderSwitches) {
        self.switches = switches;
    }
}

#[cfg(test)]
#[path = "switches_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "switches_gpu_tests.rs"]
mod gpu_tests;
