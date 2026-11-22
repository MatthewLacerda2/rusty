//! Which of a `Shadowmask` bake's four mask channels each `Mixed` light's static occlusion goes in
//! (Unity's `Light.bakingOutput.occlusionMaskChannel`). Channels are an overlap colouring of the
//! lights' reach (directional: everywhere; point and spot: their range's sphere), taken in a fixed
//! order (directional first, then entity), lowest free channel first; a light with none falls back
//! to `Baked`, as in Unity.

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;

use rusty_components::{LightMode, LightType};
use rusty_scene::lighting::MixedLightingMode;
use rusty_scene::Scene;

pub const MASK_CHANNELS: u8 = 4;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaskChannels {
    pub channels: BTreeMap<u32, u8>,
    pub fell_back: BTreeSet<u32>,
}

impl MaskChannels {
    pub fn any(&self) -> bool {
        !self.channels.is_empty()
    }
}

#[derive(Clone, Copy, Debug)]
enum Reach {
    Everywhere,
    Sphere(Vec3, f32),
}

impl Reach {
    fn overlaps(self, other: Reach) -> bool {
        match (self, other) {
            (Reach::Sphere(a, ra), Reach::Sphere(b, rb)) => a.distance(b) < ra + rb,
            _ => true,
        }
    }
}

pub fn assign_channels(scene: &Scene) -> MaskChannels {
    let mut out = MaskChannels::default();
    if scene.lighting_settings.mixed_lighting_mode != MixedLightingMode::Shadowmask {
        return out;
    }
    let mut lights: Vec<(u32, Reach)> = masked_lights(scene);
    // Directional first (they meet every light), then entity order.
    lights.sort_by_key(|(id, reach)| (!matches!(reach, Reach::Everywhere), *id));
    let mut placed: Vec<(Reach, u8)> = Vec::new();
    for (id, reach) in lights {
        let taken = |c: u8| placed.iter().any(|(r, k)| *k == c && r.overlaps(reach));
        match (0..MASK_CHANNELS).find(|&c| !taken(c)) {
            Some(c) => {
                placed.push((reach, c));
                out.channels.insert(id, c);
            }
            None => {
                out.fell_back.insert(id);
            }
        }
    }
    out
}

fn masked_lights(scene: &Scene) -> Vec<(u32, Reach)> {
    let world = &scene.world;
    let mut out = Vec::new();
    for id in world.ids_with_light() {
        let Some(light) = world.light(id) else {
            continue;
        };
        let lit = light.intensity > 0.0 && light.intensity.is_finite();
        if !world.is_active_in_hierarchy(id) || !lit || light.effective_mode() != LightMode::Mixed {
            continue;
        }
        let reach = match light.light_type {
            LightType::Directional => Reach::Everywhere,
            LightType::Point | LightType::Spotlight if light.cast_shadows => {
                let range = light.range.max(0.0);
                if !range.is_finite() || range == 0.0 {
                    continue;
                }
                Reach::Sphere(world.world_position(id), range)
            }
            _ => continue,
        };
        out.push((id, reach));
    }
    out
}
