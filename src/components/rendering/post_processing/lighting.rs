//! The volume's lighting settings (#435, #436): the sun's shadow cascades and
//! screen-space ambient occlusion, HDRP's Shadows and Ambient Occlusion overrides.
//! They shape the lighting passes, not the post chain, so they are no post effect.

use serde::{Deserialize, Serialize};

/// How the sun's cascaded shadow map covers the view (#435) — HDRP's Shadows volume
/// override: how many cascades split the view, and how far from the camera shadows
/// reach. Without an active volume the renderer uses [`ShadowSettings::default`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ShadowSettings {
    /// Cascades the view is split into, `1..=4`. More cascades keep near shadows
    /// sharp over a longer reach, at one more depth pass each.
    pub cascades: u32,
    /// Distance from the camera, in world units, past which nothing casts or receives
    /// a sun shadow (they fade out over its last tenth).
    pub distance: f32,
}

impl ShadowSettings {
    /// The cascade-count range the shadow map supports.
    pub const MAX_CASCADES: u32 = 4;
    /// The shortest shadow reach a write accepts.
    pub const MIN_DISTANCE: f32 = 1.0;
}

impl Default for ShadowSettings {
    /// Four cascades over 100 m: the whole of a large room or a street-scale outdoor
    /// space shadowed, with the first cascade a few metres deep for first-person
    /// contact shadows. Unity's High tier is 4 cascades over 150 m; 100 m spends the
    /// same four maps on less ground, since a shooter's fights are closer than that.
    fn default() -> Self {
        Self {
            cascades: Self::MAX_CASCADES,
            distance: 100.0,
        }
    }
}

/// Screen-space ambient occlusion (#436) — HDRP's Ambient Occlusion volume override.
/// Darkens only the ambient/indirect light where geometry crowds a point (a crate's
/// foot on the floor, a room's corners), never direct light. The quality tier decides
/// the resolution and sample count; these are the look.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SsaoSettings {
    /// Whether the AO passes run at all. Off costs nothing.
    pub active: bool,
    /// World-space reach of the occlusion search: how far from a surface another
    /// surface still shades it.
    pub radius: f32,
    /// How strongly occlusion darkens the ambient term; `0` is none, `1` the
    /// physically-plausible amount, above `1` exaggerated.
    pub intensity: f32,
}

impl SsaoSettings {
    /// The shortest radius a write accepts.
    pub const MIN_RADIUS: f32 = 0.01;
    /// The strongest intensity a write accepts.
    pub const MAX_INTENSITY: f32 = 4.0;
}

impl Default for SsaoSettings {
    /// On, half a metre, plain strength: the contact and corner shading an interior
    /// needs, without the dark halo a wider radius paints around a character.
    fn default() -> Self {
        Self {
            active: true,
            radius: 0.5,
            intensity: 1.0,
        }
    }
}
