use glam::{Vec2, Vec3};

use rusty_components::{LightComponent, LightMode, LightType};
use rusty_scene::create::RUNTIME_SHADOW_FADE_RANGES;
use rusty_scene::rules::sanitize::{clamped, finite2_or, finite3, non_negative};

pub fn set_color(l: &mut LightComponent, rgb: Vec3) {
    l.color = finite3(rgb).max(Vec3::ZERO);
}

pub fn set_intensity(l: &mut LightComponent, value: f32) {
    l.intensity = non_negative(value);
}

/// A shadow fade still at the runtime default follows the range; `0` (no fade) stays.
pub fn set_range(l: &mut LightComponent, value: f32) {
    let follows = l.shadow_fade_distance > 0.0
        && l.shadow_fade_distance == l.range * RUNTIME_SHADOW_FADE_RANGES;
    l.range = non_negative(value);
    if follows {
        l.shadow_fade_distance = l.range * RUNTIME_SHADOW_FADE_RANGES;
    }
}

pub use rusty_scene::create::defaults::DEFAULT_CONES;

/// An Area light keeps its stored `mode` (it acts as `Baked`), so Mixed to Area and back is Mixed.
pub fn set_type(l: &mut LightComponent, light_type: LightType) {
    l.light_type = light_type;
    if l.light_type == LightType::Spotlight && l.inner_cone == 0.0 && l.outer_cone == 0.0 {
        set_cones(l, DEFAULT_CONES.0, DEFAULT_CONES.1);
    }
}

/// `Component.Get` reads the mode an Area light acts on (`Baked`), so a copied table sets
/// unrefused.
pub fn shown(l: &LightComponent, value: &mut serde_json::Value) {
    value["mode"] = serde_json::json!(l.effective_mode().name());
}

/// An Area light refuses anything but `Baked`: `false` then, for the caller to warn.
pub fn set_mode(l: &mut LightComponent, mode: LightMode) -> bool {
    if l.light_type == LightType::Area && mode != LightMode::Baked {
        return false;
    }
    l.mode = mode;
    true
}

/// World units: intensity scales by the panel's area, so a zero-area one emits nothing.
pub const MIN_AREA_SIDE: f32 = 0.01;

pub fn set_area_size(l: &mut LightComponent, size: Vec2) {
    l.area_size = finite2_or(size, l.area_size).max(Vec2::splat(MIN_AREA_SIDE));
}

/// A half-angle in degrees, so the full cone stays within 180°.
pub const MAX_CONE: f32 = 90.0;

pub fn set_cones(l: &mut LightComponent, inner: f32, outer: f32) {
    l.outer_cone = clamped(outer, 0.0, MAX_CONE);
    l.inner_cone = clamped(inner, 0.0, l.outer_cone);
}

pub fn set_cast_shadows(l: &mut LightComponent, on: bool) {
    l.cast_shadows = on;
}

pub fn set_shadow_fade_distance(l: &mut LightComponent, value: f32) {
    l.shadow_fade_distance = non_negative(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    use rusty_scene::Scene;

    fn scene_with_light() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Lamp".to_string());
        let c = crate::authoring::default_light();
        scene.world.set_light(id, Some(c));
        (scene, id)
    }

    #[test]
    fn ops_write_through_with_validation() {
        let (mut scene, id) = scene_with_light();
        let mut e = scene.world.light_mut(id).unwrap();
        set_color(&mut e, Vec3::new(0.2, 0.4, 0.6));
        set_intensity(&mut e, -5.0);
        set_range(&mut e, -2.0);
        set_type(&mut e, LightType::Directional);
        set_cones(&mut e, 80.0, 40.0);
        set_mode(&mut e, LightMode::Baked);
        let l = &*e;
        assert_eq!(l.color, Vec3::new(0.2, 0.4, 0.6));
        assert_eq!(l.intensity, 0.0, "intensity clamps to 0");
        assert_eq!(l.range, 0.0, "range clamps to 0");
        assert_eq!(l.light_type, LightType::Directional);
        assert_eq!(l.outer_cone, 40.0);
        assert_eq!(l.inner_cone, 40.0, "inner clamps to outer");
        assert_eq!(l.mode, LightMode::Baked);
    }

    #[test]
    fn spot_seeds_only_zero_cones() {
        let (mut scene, id) = scene_with_light();
        let mut e = scene.world.light_mut(id).unwrap();
        set_cones(&mut e, 0.0, 0.0);
        set_type(&mut e, LightType::Spotlight);
        assert_eq!((e.inner_cone, e.outer_cone), DEFAULT_CONES);
        set_cones(&mut e, 10.0, 20.0);
        set_type(&mut e, LightType::Spotlight);
        assert_eq!((e.inner_cone, e.outer_cone), (10.0, 20.0));
    }

    #[test]
    fn non_finite_inputs_take_the_fallback() {
        let (mut scene, id) = scene_with_light();
        let mut e = scene.world.light_mut(id).unwrap();
        set_intensity(&mut e, f32::INFINITY);
        set_color(&mut e, Vec3::new(f32::NAN, 0.5, f32::INFINITY));
        set_cones(&mut e, f32::NAN, f32::INFINITY);
        assert_eq!(e.intensity, 0.0);
        assert_eq!(e.color, Vec3::ZERO, "a colour is refused whole");
        assert_eq!((e.inner_cone, e.outer_cone), (0.0, 0.0));
        set_cones(&mut e, 10.0, 500.0);
        assert_eq!((e.inner_cone, e.outer_cone), (10.0, MAX_CONE));
        set_shadow_fade_distance(&mut e, 30.0);
        assert_eq!(e.shadow_fade_distance, 30.0);
        for bad in [f32::NAN, f32::INFINITY, -4.0] {
            set_shadow_fade_distance(&mut e, bad);
            assert_eq!(e.shadow_fade_distance, 0.0, "{bad} turns the fade off");
        }
    }

    #[test]
    fn area_lights_are_baked_and_sized() {
        let (mut scene, id) = scene_with_light();
        let mut e = scene.world.light_mut(id).unwrap();
        set_mode(&mut e, LightMode::Mixed);
        set_type(&mut e, LightType::Area);
        assert_eq!(e.effective_mode(), LightMode::Baked);
        assert!(!set_mode(&mut e, LightMode::Realtime), "refused");
        set_type(&mut e, LightType::Point);
        assert_eq!(
            e.mode,
            LightMode::Mixed,
            "the stored mode survives the round trip"
        );
        set_type(&mut e, LightType::Area);
        set_area_size(&mut e, Vec2::new(1.2, 0.0));
        assert_eq!(e.area_size, Vec2::new(1.2, MIN_AREA_SIDE));
        set_area_size(&mut e, Vec2::new(2.0, f32::NAN));
        assert_eq!(e.area_size, Vec2::new(1.2, MIN_AREA_SIDE), "refused whole");
    }

    #[test]
    fn get_reports_an_area_lights_effective_mode() {
        let (mut scene, id) = scene_with_light();
        let mut e = scene.world.light_mut(id).unwrap();
        set_mode(&mut e, LightMode::Mixed);
        set_type(&mut e, LightType::Area);
        let mut value = serde_json::to_value(&*e).unwrap();
        shown(&e, &mut value);
        assert_eq!(value["mode"], "Baked");
        let copy: LightComponent = serde_json::from_value(value).unwrap();
        assert_eq!(copy.mode, LightMode::Baked);
    }

    #[test]
    fn op_on_lightless_entity_is_a_no_op() {
        let mut scene = Scene::new();
        let id = scene.add_entity("Empty".to_string());
        if let Some(mut l) = scene.world.light_mut(id) {
            set_intensity(&mut l, 5.0);
        }
        assert!(!scene.world.has_light(id));
    }
}
