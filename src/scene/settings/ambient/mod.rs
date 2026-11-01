//! src/scene/settings/ambient/ — the scene's ambient (diffuse environment) light:
//! Unity's *Environment Lighting*, `RenderSettings.ambientMode` with its colours
//! (#1647).
//!
//! Three sources, as Unity's `Color`, `Gradient` (`AmbientMode.Trilight`) and
//! `Skybox`:
//! - **Color**: one colour (`Scene::ambient_color`), drawn as a hemisphere, the
//!   colour straight up fading to a quarter of it straight down.
//! - **Gradient**: three colours, [`AmbientSettings`]'s `sky` (up-facing surfaces),
//!   `equator` (sideways) and `ground` (down-facing), blended by the normal's `y`.
//! - **Skybox** (#1675): the sky itself, the panorama or the procedural sky,
//!   projected into L2 SH (Unity's ambient probe, `skybox`), so an HDRI lights
//!   the scene it surrounds. rusty's default stays `Color` (Unity's is `Skybox`),
//!   so a scene that never chose draws as it always has.
//!
//! `ambient_intensity` scales each, as Unity's *Intensity Multiplier*. An active
//! Ambient light entity overrides the scene's source with its one colour, in
//! `Color` semantics (rusty's own entity; Unity has none).
//!
//! **One evaluation, three readers**: [`Scene::drawn_ambient`] is what a frame draws,
//! and its [`AmbientRadiance`] is the one function the forward shader
//! (`lighting::ambient::ambient_radiance`, a GPU test holds the two equal), the
//! lightmap trace's escaping ray, the light-probe fill and the GPU light-probe
//! bake's escaping directions (`render::ibl::probe::capture`, #1668) all evaluate. The shader
//! shades a surface with [`AmbientRadiance::at`]; the bakes take its `incoming`
//! radiance, whose cosine average is `at` again, so an open baked or probe-lit
//! surface matches an unbaked one (exactly in `Color` and `Skybox`, up to L2 in
//! `Gradient`). The procedural sky follows only the `Color` colour
//! (`SkySettings::resolved`) whatever the mode; outside `Skybox` it is apart from
//! the ambient, as Unity's skybox is, and no light-probe or lightmap bake sees it.
//!
//! Pure data. Every write goes through `scene::authoring::environment`.

mod radiance;
mod skybox;
#[cfg(test)]
mod tests;

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub use radiance::AmbientRadiance;
#[cfg(test)]
pub use skybox::panorama_fixture;

use crate::scene::{LightType, Scene, Sh9, DEFAULT_AMBIENT_COLOR};

/// Where the ambient light comes from: Unity's *Environment Lighting → Source*.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmbientMode {
    /// One colour, `Scene::ambient_color`, drawn as a sky-to-ground hemisphere.
    #[default]
    Color,
    /// Three colours: sky, equator and ground (Unity's `AmbientMode.Trilight`).
    Gradient,
    /// The sky the scene draws, panorama or procedural, as L2 SH (Unity's
    /// `AmbientMode.Skybox`).
    Skybox,
}

impl AmbientMode {
    /// Every mode, in the Scene Settings dropdown's order.
    pub const ALL: [Self; 3] = [Self::Color, Self::Gradient, Self::Skybox];

    /// The mode's name, as `Lighting.GetAmbientMode` returns it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Color => "Color",
            Self::Gradient => "Gradient",
            Self::Skybox => "Skybox",
        }
    }

    /// The mode a name means (`"Color"`, `"Gradient"` or `"Skybox"`,
    /// case-insensitive).
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }
}

/// The ambient source and the Gradient mode's three colours. The colours are kept
/// in `Color` mode, as Unity keeps them, so switching back and forth loses nothing.
/// `#[serde(default)]` loads a partial block (or none) with the defaults.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AmbientSettings {
    pub mode: AmbientMode,
    /// Linear colour lighting up-facing surfaces.
    pub sky: Vec3,
    /// Linear colour lighting sideways-facing surfaces.
    pub equator: Vec3,
    /// Linear colour lighting down-facing surfaces.
    pub ground: Vec3,
}

impl Default for AmbientSettings {
    /// `Color` mode; the gradient's colours are the default ambient's hemisphere at
    /// its top, middle and bottom, so switching the default scene to `Gradient`
    /// draws the same light.
    fn default() -> Self {
        Self {
            mode: AmbientMode::Color,
            sky: DEFAULT_AMBIENT_COLOR,
            equator: DEFAULT_AMBIENT_COLOR * 0.625,
            ground: DEFAULT_AMBIENT_COLOR * 0.25,
        }
    }
}

impl AmbientSettings {
    /// Every knob at its default: the scene file leaves the block out.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The ambient a frame draws: what [`Scene::drawn_ambient`] resolves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawnAmbient {
    /// The `Color` mode colour: the scene's, or an active Ambient light's. The
    /// procedural sky follows it whatever the mode.
    pub color: Vec3,
    pub intensity: f32,
    /// The sky, equator and ground colours when the scene's `Gradient` mode is
    /// drawn; `None` draws `color`'s hemisphere.
    pub gradient: Option<[Vec3; 3]>,
    /// The sky's radiance SH at intensity 1 when the scene's `Skybox` mode is
    /// drawn (derived, never saved); it wins over `gradient`.
    pub skybox: Option<Sh9>,
}

impl DrawnAmbient {
    /// The light arriving from each direction, intensity applied.
    pub fn radiance(&self) -> AmbientRadiance {
        if let Some(sh) = self.skybox {
            return AmbientRadiance::Sky(sh.scaled(self.intensity));
        }
        match self.gradient {
            None => AmbientRadiance::Hemisphere(self.color * self.intensity),
            Some([sky, equator, ground]) => AmbientRadiance::Trilight {
                sky: sky * self.intensity,
                equator: equator * self.intensity,
                ground: ground * self.intensity,
            },
        }
    }
}

impl Scene {
    /// The ambient drawn: the last active Ambient light entity's colour and
    /// intensity, in `Color` semantics, else the scene's own source
    /// (`ambient_color`, the gradient or the sky, by `ambient.mode`) and
    /// `ambient_intensity`.
    pub fn drawn_ambient(&self) -> DrawnAmbient {
        let mut drawn = DrawnAmbient {
            color: self.ambient_color,
            intensity: self.ambient_intensity,
            gradient: None,
            skybox: None,
        };
        let has_light = self.world.ids_with_light().into_iter().any(|id| {
            self.world
                .light(id)
                .is_some_and(|l| l.light_type == LightType::Ambient)
                && self.world.is_active_in_hierarchy(id)
        });
        match self.ambient.mode {
            AmbientMode::Color => {}
            AmbientMode::Gradient => {
                drawn.gradient =
                    Some([self.ambient.sky, self.ambient.equator, self.ambient.ground]);
            }
            // An Ambient light overrides the sky, so it is projected only when none does.
            AmbientMode::Skybox if !has_light => {
                drawn.skybox = Some(skybox::sky_radiance(self, self.ambient_color));
            }
            AmbientMode::Skybox => {}
        }
        for id in self.world.ids_with_light() {
            let light = self.world.light(id).expect("id came from ids_with_light");
            if light.light_type == LightType::Ambient && self.world.is_active_in_hierarchy(id) {
                drawn = DrawnAmbient {
                    color: light.color,
                    intensity: light.intensity,
                    gradient: None,
                    skybox: None,
                };
            }
        }
        drawn
    }
}
