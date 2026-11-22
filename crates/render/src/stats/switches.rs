#[cfg(feature = "dev")]
use crate::Renderer;
use rusty_core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use rusty_scene::lighting::lightmap_set::LightmapSet;

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum RenderSwitch {
        Shadows = "shadows",
        Lightmaps = "lightmaps",
        ReflectionProbes = "reflection_probes",
        ClusteredLights = "clustered_lights",
        Ssao = "ssao",
        Ssil = "ssil",
    }
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum EarlyFlushes {
        None = "none",
        Occlusion = "occlusion",
        #[default]
        OcclusionScene = "occlusion_scene",
    }
}

impl EarlyFlushes {
    pub(crate) fn count(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderSwitches {
    off: [bool; RenderSwitch::ALL.len()],
    flushes: EarlyFlushes,
}

impl RenderSwitches {
    pub const ON: Self = Self {
        off: [false; RenderSwitch::ALL.len()],
        flushes: EarlyFlushes::OcclusionScene,
    };

    pub fn flushes(self) -> EarlyFlushes {
        self.flushes
    }

    pub fn set_flushes(&mut self, flushes: EarlyFlushes) {
        self.flushes = flushes;
    }

    pub fn on(self, switch: RenderSwitch) -> bool {
        !self.off[switch as usize]
    }

    pub fn set(&mut self, switch: RenderSwitch, on: bool) {
        self.off[switch as usize] = !on;
    }

    pub(crate) fn listed<T>(self, switch: RenderSwitch, items: &[T]) -> &[T] {
        if self.on(switch) {
            items
        } else {
            &[]
        }
    }

    pub(crate) fn lightmap_lists(self, set: &LightmapSet) -> [&[String]; 3] {
        match self.on(RenderSwitch::Lightmaps) {
            true => [&set.pages, &set.directions, &set.masks],
            false => [&[]; 3],
        }
    }

    pub(crate) fn casts_shadows(self, capture_shadows: bool) -> bool {
        capture_shadows && self.on(RenderSwitch::Shadows)
    }

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
