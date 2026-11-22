use crate::lighting::MixedLightingMode;
use crate::Scene;
use rusty_components::{LightComponent, LightMode, LightType};

impl Scene {
    pub fn light_mode(&self, id: u32, light: &LightComponent) -> LightMode {
        self.lightmaps.baked_mode(id, light.effective_mode())
    }

    pub fn awaits_bake(&self, id: u32, light: &LightComponent) -> bool {
        !matches!(light.light_type, LightType::Area | LightType::Ambient)
            && self.light_mode(id, light) == LightMode::Baked
            && !self.lightmaps.bakes_direct(id)
    }

    pub fn drawn_mode(&self, id: u32, light: &LightComponent, preview: bool) -> LightMode {
        match preview && self.awaits_bake(id, light) {
            true => LightMode::Mixed,
            false => self.light_mode(id, light),
        }
    }

    pub fn waits_for_bake(&self, id: u32) -> bool {
        let world = &self.world;
        world.is_active_in_hierarchy(id)
            && (world.light(id)).is_some_and(|l| l.intensity > 0.0 && self.awaits_bake(id, &l))
    }

    pub fn lights_awaiting_bake(&self) -> Vec<u32> {
        (self.world.ids_with_light().into_iter())
            .filter(|&id| self.waits_for_bake(id))
            .collect()
    }

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

pub const PROBES_STALE: &str = "Lighting.BakeLightmaps: the light probes' shadowmask \
     occlusion is stale until Probe.Bake() or Generate Lighting";

pub const NO_CHANNELS: &str = "Probe.Bake: no shadowmask channels are recorded, so the \
     probes hold no occlusion; run Lighting.BakeLightmaps() first, or Generate Lighting";

impl Scene {
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
