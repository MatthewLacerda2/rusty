mod radiance;
mod skybox;
#[cfg(test)]
mod tests;

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub use radiance::AmbientRadiance;
pub use skybox::procedural_sky_color;

use crate::{LightType, Scene, Sh9, DEFAULT_AMBIENT_COLOR};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmbientMode {
    #[default]
    Color,
    Gradient,
    Skybox,
}

impl AmbientMode {
    pub const ALL: [Self; 3] = [Self::Color, Self::Gradient, Self::Skybox];

    pub fn name(self) -> &'static str {
        match self {
            Self::Color => "Color",
            Self::Gradient => "Gradient",
            Self::Skybox => "Skybox",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|m| m.name().eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AmbientSettings {
    pub mode: AmbientMode,
    pub sky: Vec3,
    pub equator: Vec3,
    pub ground: Vec3,
}

impl Default for AmbientSettings {
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
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawnAmbient {
    pub color: Vec3,
    pub intensity: f32,
    pub gradient: Option<[Vec3; 3]>,
    pub skybox: Option<Sh9>,
}

impl DrawnAmbient {
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
