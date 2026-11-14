//! Shadowmask channels (#1695): which of a `Shadowmask` bake's four mask channels
//! each `Mixed` light's static occlusion goes in, Unity's
//! `Light.bakingOutput.occlusionMaskChannel`.
//!
//! As in Unity, the channels are an overlap colouring of the lights' reach: two
//! lights whose light can meet on one texel never share one, so a texel holds at
//! most four. A directional light reaches everywhere, so it overlaps every light;
//! a point or spot light reaches the sphere of its range. Lights are coloured in a
//! fixed order (directional first, then by entity), each taking the lowest channel
//! no overlapping light already has. One that finds all four taken falls back to
//! `Baked`, as Unity bakes such a light: its direct light goes into the lightmaps
//! and probes, and the frame no longer draws it.
//!
//! A light takes part when it is active, lit, `Mixed` and casts shadows (a
//! directional light always does, through the cascades): Unity gives a channel only
//! to a Mixed light with shadows on, since the mask *is* its shadow.

use std::collections::{BTreeMap, BTreeSet};

use glam::Vec3;

use rusty_scene::lighting::MixedLightingMode;
use rusty_scene::Scene;
use rusty_components::{LightMode, LightType};

/// How many lights a texel's mask holds: its four channels.
pub const MASK_CHANNELS: u8 = 4;

/// A `Shadowmask` bake's channels: the lights given one, and those that fell back.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MaskChannels {
    pub channels: BTreeMap<u32, u8>,
    pub fell_back: BTreeSet<u32>,
}

impl MaskChannels {
    /// Whether any light has a channel: the bake then writes mask pages.
    pub fn any(&self) -> bool {
        !self.channels.is_empty()
    }
}

/// Where one masked light reaches: everywhere, or its range's sphere.
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

/// The channels a bake of `scene` gives its lights: none unless its Mixed
/// Lighting Mode is `Shadowmask`.
pub fn assign_channels(scene: &Scene) -> MaskChannels {
    let mut out = MaskChannels::default();
    if scene.lighting_settings.mixed_lighting_mode != MixedLightingMode::Shadowmask {
        return out;
    }
    let mut lights: Vec<(u32, Reach)> = masked_lights(scene);
    // Directional first (they meet every light), then entity order: deterministic.
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

/// The lights a `Shadowmask` bake masks, each with where it reaches.
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
