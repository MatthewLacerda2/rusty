//! The mode the frame draws a light as (#1695): its own, but `Baked` for a `Mixed`
//! light the last `Shadowmask` bake found no channel for (`LightmapSet::fell_back`),
//! and the shadowmask channel it masks a light by.

use crate::components::{LightComponent, LightMode};
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
    /// (`LightmapSet::mask_channel`) while the scene is on `Shadowmask`, else
    /// none, so a scene turned back to `BakedIndirect` draws as one before its
    /// next bake.
    pub fn mask_channel(&self, id: u32) -> Option<u8> {
        let on = self.lighting_settings.mixed_lighting_mode == MixedLightingMode::Shadowmask;
        self.lightmaps.mask_channel(id).filter(|_| on)
    }
}
