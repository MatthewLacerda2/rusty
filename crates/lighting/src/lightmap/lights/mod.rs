use glam::Vec3;

pub(in crate::lightmap) mod area;
mod channels;
#[cfg(test)]
mod channels_tests;
#[cfg(test)]
mod recorded_tests;
#[cfg(test)]
mod tests;

pub use channels::{assign_channels, MaskChannels, MASK_CHANNELS};

use super::input::BakeScene;
use area::AreaShape;
use rusty_components::{LightMode, LightType};
use rusty_scene::Scene;

#[derive(Clone, Copy, Debug)]
pub enum LightShape {
    Directional {
        direction: Vec3,
    },
    Point {
        position: Vec3,
        range: f32,
    },
    Spot {
        position: Vec3,
        direction: Vec3,
        range: f32,
        cos_inner: f32,
        cos_outer: f32,
    },
    Area(AreaShape),
}

/// `Mixed` (bounce only) or `Baked` (direct and bounce); `Realtime` lights never get here.
#[derive(Clone, Copy, Debug)]
pub struct BakeLight {
    pub shape: LightShape,
    pub radiance: Vec3,
    /// `Baked`, or a `Mixed` light that fell back for want of a mask channel.
    pub bakes_direct: bool,
    pub mask_channel: Option<u8>,
}

pub type TakenLights = std::collections::BTreeMap<u32, LightMode>;

/// Except intensity 0: a flash lamp placed dark and raised by a script still bounces through SSIL.
pub fn taken_lights(scene: &Scene) -> TakenLights {
    let takes = |id: &u32| {
        let light = scene.world.light(*id);
        scene.world.is_active_in_hierarchy(*id)
            && light.is_some_and(|l| {
                l.light_type != LightType::Ambient
                    && l.intensity > 0.0
                    && l.effective_mode().bakes_indirect()
            })
    };
    scene
        .world
        .ids_with_light()
        .into_iter()
        .filter(takes)
        .filter_map(|id| Some((id, scene.world.light(id)?.effective_mode())))
        .collect()
}

impl BakeScene {
    pub(super) fn gather_lights(&mut self, scene: &Scene) {
        self.gather_lights_with(scene, assign_channels(scene));
    }

    /// Takes the channels and fallbacks the last lightmap bake recorded, never fresh ones, so
    /// probes match the frame's masking.
    pub fn use_recorded_channels(&mut self, scene: &Scene) {
        let recorded = MaskChannels {
            channels: (scene.lightmaps.channels.keys())
                .filter_map(|&id| scene.mask_channel(id).map(|c| (id, c)))
                .collect(),
            fell_back: (scene.lightmaps.fell_back.iter().copied())
                .filter(|&id| {
                    let light = scene.world.light(id);
                    light.is_some_and(|l| scene.light_mode(id, &l) == LightMode::Baked)
                })
                .collect(),
        };
        self.lights.clear();
        self.gather_lights_with(scene, recorded);
    }

    fn gather_lights_with(&mut self, scene: &Scene, shadowmask: MaskChannels) {
        self.shadowmask = shadowmask;
        for id in scene.world.ids_with_light() {
            if !scene.world.is_active_in_hierarchy(id) {
                continue;
            }
            let light = scene.world.light(id).expect("id came from ids_with_light");
            // World space: a light on a child bakes where it is drawn.
            let position = scene.world.world_position(id);
            let forward = scene
                .world
                .transform_direction(id, Vec3::NEG_Z)
                .try_normalize()
                .unwrap_or(Vec3::NEG_Y);
            let shape = match light.light_type {
                // The sky is `Scene::drawn_ambient`'s, which an Ambient light sets.
                LightType::Ambient => continue,
                LightType::Directional => LightShape::Directional { direction: forward },
                LightType::Point => LightShape::Point {
                    position,
                    range: light.range,
                },
                LightType::Spotlight => LightShape::Spot {
                    position,
                    direction: forward,
                    range: light.range,
                    cos_inner: light.inner_cone.to_radians().cos(),
                    cos_outer: light.outer_cone.to_radians().cos(),
                },
                LightType::Area => LightShape::Area(AreaShape::of(scene, id, &light)),
            };
            let mode = light.effective_mode();
            if mode.bakes_indirect() {
                self.lights.push(BakeLight {
                    shape,
                    radiance: light.color * light.intensity,
                    bakes_direct: mode.bakes_direct() || self.shadowmask.fell_back.contains(&id),
                    mask_channel: self.shadowmask.channels.get(&id).copied(),
                });
            }
        }
    }
}
