//! src/scene/authoring/light.rs — Shared light-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `LightComponent` field by field (colour, intensity, range, type, and the
//! spotlight cone angles), including the validation each write owns (the `≥ 0`
//! clamp on intensity/range, the inner≤outer cone ordering).
//!
//! BOTH callers route through here so the editor and the API can never drift: the
//! inspector's Light card (`editor::inspector::components::render`) and the Lua
//! `Light.*` namespace (`api::light`) are siblings over these same Rust ops (#287).
//! Each op takes `&mut Entity` and is a no-op when the entity has no light, so a
//! caller never has to re-check first.
//!
//! Allowed deps: components (the `LightComponent`/`LightType` data). Pure: no
//! wall-clock, no RNG.

use glam::Vec3;

use crate::components::{Entity, LightType};

/// Set the light's linear RGB colour.
pub fn set_color(entity: &mut Entity, rgb: Vec3) {
    if let Some(l) = &mut entity.light {
        l.color = rgb;
    }
}

/// Set the light's intensity, clamped to `≥ 0` (a negative emit is meaningless).
pub fn set_intensity(entity: &mut Entity, value: f32) {
    if let Some(l) = &mut entity.light {
        l.intensity = value.max(0.0);
    }
}

/// Set the light's range, clamped to `≥ 0`.
pub fn set_range(entity: &mut Entity, value: f32) {
    if let Some(l) = &mut entity.light {
        l.range = value.max(0.0);
    }
}

/// Set the light's kind (typed — string parsing stays in the API adapter / card).
pub fn set_type(entity: &mut Entity, light_type: LightType) {
    if let Some(l) = &mut entity.light {
        l.light_type = light_type;
    }
}

/// Set the spotlight cone angles (degrees), keeping `inner ≤ outer` so the inner
/// cone never exceeds the outer one. The validation lives HERE, once.
pub fn set_cones(entity: &mut Entity, inner: f32, outer: f32) {
    if let Some(l) = &mut entity.light {
        l.outer_cone = outer;
        l.inner_cone = inner.min(outer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::LightComponent;
    use crate::scene::Scene;

    /// A scene with one light-bearing entity; returns `(scene, id)`.
    fn scene_with_light() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Lamp".to_string());
        if let Some(mut e) = scene.get_entity_mut(id) {
            e.light = Some(LightComponent {
                light_type: LightType::Point,
                color: Vec3::ONE,
                intensity: 1.5,
                range: 10.0,
                inner_cone: 30.0,
                outer_cone: 45.0,
            });
        }
        (scene, id)
    }

    #[test]
    fn ops_write_through_with_validation() {
        let (mut scene, id) = scene_with_light();
        let mut e = scene.get_entity_mut(id).unwrap();
        set_color(&mut e, Vec3::new(0.2, 0.4, 0.6));
        set_intensity(&mut e, -5.0);
        set_range(&mut e, -2.0);
        set_type(&mut e, LightType::Directional);
        set_cones(&mut e, 80.0, 40.0);
        let l = e.light.as_ref().unwrap();
        assert_eq!(l.color, Vec3::new(0.2, 0.4, 0.6));
        assert_eq!(l.intensity, 0.0, "intensity clamps to 0");
        assert_eq!(l.range, 0.0, "range clamps to 0");
        assert_eq!(l.light_type, LightType::Directional);
        assert_eq!(l.outer_cone, 40.0);
        assert_eq!(l.inner_cone, 40.0, "inner clamps to outer");
    }

    #[test]
    fn op_on_lightless_entity_is_a_no_op() {
        let mut scene = Scene::new();
        let id = scene.add_entity("Empty".to_string());
        let mut e = scene.get_entity_mut(id).unwrap();
        set_intensity(&mut e, 5.0); // no panic, no light
        assert!(e.light.is_none());
    }
}
