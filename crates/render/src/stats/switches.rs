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
//! | `lightmaps` | the colour, direction and shadowmask pages (group 0, bindings 14, 16, 19) | the 1x1 stand-in pages, each object's `lightmap_page` 0 |
//! | `reflection_probes` | the resident probes' cube array (group 0, binding 4) | the 1-cube stand-in, each instance's `reflection_probe` 0 (the sky's cube) |
//! | `clustered_lights` | the cluster light lists (group 0, bindings 8 and 9) | every cluster's light range empty; the lights stay uploaded at binding 7 |

#[cfg(feature = "dev")]
use crate::Renderer;
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
    }
}

/// Which [`RenderSwitch`]es are on: all of them unless a measurement turned one off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderSwitches {
    off: [bool; RenderSwitch::ALL.len()],
}

impl RenderSwitches {
    /// Every switch on: what every renderer draws with.
    pub const ON: Self = Self {
        off: [false; RenderSwitch::ALL.len()],
    };

    /// Whether `switch` is on.
    pub fn on(self, switch: RenderSwitch) -> bool {
        !self.off[switch as usize]
    }

    /// Turn `switch` on or off.
    pub fn set(&mut self, switch: RenderSwitch, on: bool) {
        self.off[switch as usize] = !on;
    }

    /// The colour, direction and shadowmask page lists the renderer binds for
    /// `set`: none with the `lightmaps` switch off, as for an unbaked scene.
    pub(crate) fn lightmap_lists(self, set: &LightmapSet) -> [&[String]; 3] {
        match self.on(RenderSwitch::Lightmaps) {
            true => [&set.pages, &set.directions, &set.masks],
            false => [&[]; 3],
        }
    }

    /// Whether every switch is on.
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
#[path = "switches_gpu_tests.rs"]
mod gpu_tests;
