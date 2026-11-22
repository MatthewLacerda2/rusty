use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

mod falloff;
pub use falloff::{
    falloff, falloff_near, falloff_window, falloff_window_sq, spot_cone, FALLOFF_EPSILON_SQ,
};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum LightType {
        Ambient = "Ambient",
        Directional = "Directional",
        Point = "Point",
        Spotlight = "Spot",
        /// Unity's Area (Rectangle) light, baked only: `effective_mode` is always `Baked`.
        Area = "Area",
    }
}

rusty_core::named_enum! {
    /// Unity's Light Mode.
    /// * `Realtime`: no bake sees it, so it bounces nowhere.
    /// * `Baked`: never drawn live (Unity's default is `Realtime`).
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum LightMode {
        Realtime = "Realtime",
        Mixed = "Mixed",
        #[default]
        Baked = "Baked",
    }
}

impl LightMode {
    pub fn bakes_direct(self) -> bool {
        self == LightMode::Baked
    }

    pub fn bakes_indirect(self) -> bool {
        self != LightMode::Realtime
    }

    pub fn renders_live(self, capture: bool) -> bool {
        if capture {
            self.bakes_indirect()
        } else {
            self != LightMode::Baked
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LightComponent {
    pub light_type: LightType,
    pub color: Vec3,
    pub intensity: f32,
    pub range: f32,
    pub inner_cone: f32, // Degrees
    pub outer_cone: f32, // Degrees
    /// Off by default, as in Unity (atlas space, a depth pass per tile). The sun ignores it.
    #[serde(default)]
    pub cast_shadows: bool,
    #[serde(default)]
    pub mode: LightMode,
    /// World units; other types ignore it.
    #[serde(default = "unit_area")]
    pub area_size: Vec2,
    /// HDRP's *Shadow Fade Distance*, world units; `0` means no fade.
    #[serde(default)]
    pub shadow_fade_distance: f32,
}

fn unit_area() -> Vec2 {
    Vec2::ONE
}

impl LightComponent {
    pub fn effective_mode(&self) -> LightMode {
        if self.light_type == LightType::Area {
            LightMode::Baked
        } else {
            self.mode
        }
    }
}

impl crate::FirstClass for LightComponent {
    const NAME: &'static str = "Light";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_bake_rules() {
        assert_eq!(LightMode::default(), LightMode::Baked);
        assert!(!LightMode::Realtime.bakes_indirect() && !LightMode::Realtime.bakes_direct());
        assert!(LightMode::Mixed.bakes_indirect() && !LightMode::Mixed.bakes_direct());
        assert!(LightMode::Baked.bakes_indirect() && LightMode::Baked.bakes_direct());
    }

    #[test]
    fn only_unitys_type_names_parse() {
        assert_eq!(LightType::Spotlight.name(), "Spot", "Unity's name (#997)");
        assert_eq!(
            LightType::parse("Spotlight"),
            None,
            "the pre-#997 name is gone"
        );
    }

    #[test]
    fn frames_drop_baked_lights_and_captures_drop_realtime_ones() {
        let live = |capture| LightMode::ALL.map(|m| m.renders_live(capture));
        assert_eq!(live(false), [true, true, false]);
        assert_eq!(live(true), [false, true, true]);
    }

    #[test]
    fn a_light_naming_no_mode_loads_as_baked() {
        let json = r#"{"light_type":"Point","color":[1,1,1],"intensity":1,
            "range":5,"inner_cone":0,"outer_cone":0}"#;
        let l: LightComponent = serde_json::from_str(json).unwrap();
        assert_eq!(l.mode, LightMode::Baked);
        assert_eq!(l.area_size, Vec2::ONE, "Unity's default area size");
    }

    #[test]
    fn an_area_light_is_always_baked() {
        let json = r#"{"light_type":"Area","color":[1,1,1],"intensity":1,
            "range":5,"inner_cone":0,"outer_cone":0,"mode":"Realtime"}"#;
        let l: LightComponent = serde_json::from_str(json).unwrap();
        assert_eq!(l.effective_mode(), LightMode::Baked);
    }
}
