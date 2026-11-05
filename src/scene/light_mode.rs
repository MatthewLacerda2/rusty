//! The mode the frame draws a light as (#1695): its own, but `Baked` for a `Mixed`
//! light the last `Shadowmask` bake found no channel for (`LightmapSet::fell_back`),
//! and the shadowmask channel it masks a light by.

use crate::components::{LightComponent, LightMode, LightType};
use crate::scene::lighting::MixedLightingMode;
use crate::scene::Scene;

impl Scene {
    /// Light `id`'s mode as the renderer acts on it: `light.effective_mode()`, held
    /// to what the last bake made of it (`LightmapSet::baked_mode`). A bake decides
    /// afresh from `effective_mode`, never from this.
    pub fn light_mode(&self, id: u32, light: &LightComponent) -> LightMode {
        self.lightmaps.baked_mode(id, light.effective_mode())
    }

    /// The shadowmask channel the frame masks light `id` by (#1695): its bake's
    /// (`LightmapSet::mask_channel`) while the scene is on `Shadowmask` and the
    /// light is still one a bake masks (a `Mixed` light casting shadows; a sun
    /// always casts), else none. So a scene turned back to `BakedIndirect`, or a
    /// lamp set to `Realtime` or its shadows turned off, draws as one before the
    /// next bake.
    pub fn mask_channel(&self, id: u32) -> Option<u8> {
        if self.lighting_settings.mixed_lighting_mode != MixedLightingMode::Shadowmask {
            return None;
        }
        let channel = self.lightmaps.mask_channel(id)?;
        let light = self.world.light(id)?;
        let casts = light.cast_shadows || light.light_type == LightType::Directional;
        (light.effective_mode() == LightMode::Mixed && casts).then_some(channel)
    }
}

/// The note `Lighting.BakeLightmaps` leaves on a `Shadowmask` scene with probes
/// (#1695): the lightmap bake writes the channels, only a probe bake writes their
/// occlusion, so until one runs a dynamic object reads the old (or no) occlusion.
pub const PROBES_STALE: &str = "Lighting.BakeLightmaps: the light probes' shadowmask \
     occlusion is stale until Probe.Bake() or Generate Lighting";

/// The note `Probe.Bake` leaves on a `Shadowmask` scene whose lightmap bake recorded
/// no channels (#1695): the probes then hold no light's occlusion.
pub const NO_CHANNELS: &str = "Probe.Bake: no shadowmask channels are recorded, so the \
     probes hold no occlusion; run Lighting.BakeLightmaps() first, or Generate Lighting";

impl Scene {
    /// What a single bake verb should say about the probes' shadowmask occlusion
    /// (#1695): `lightmaps` for `Lighting.BakeLightmaps`, else `Probe.Bake`. `None`
    /// off `Shadowmask`, with no probe, or when the other bake's half agrees.
    pub fn shadowmask_note(&self, lightmaps: bool) -> Option<&'static str> {
        let on = self.lighting_settings.mixed_lighting_mode == MixedLightingMode::Shadowmask;
        if !on || self.probes.is_empty() {
            return None;
        }
        let channels = !self.lightmaps.channels.is_empty();
        match lightmaps {
            true => channels.then_some(PROBES_STALE),
            false => (!channels).then_some(NO_CHANNELS),
        }
    }
}

#[cfg(test)]
#[path = "light_mode_tests.rs"]
mod tests;
