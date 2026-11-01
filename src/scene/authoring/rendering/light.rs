//! src/scene/authoring/rendering/light.rs — Shared light-authoring ops.
//!
//! The ONE place the engine knows how to mutate an entity's first-class
//! `LightComponent` field by field (colour, intensity, range, type, the
//! spotlight cone angles, the shadow toggle and an Area light's size), including the validation each write owns (the `≥ 0`
//! clamp on intensity/range, the inner≤outer cone ordering).
//!
//! BOTH callers route through here so the editor and the API can never drift: the
//! inspector's Light card (`editor::inspector::components::render`) and the Lua
//! `Light.*` namespace (`api::light`) are siblings over these same Rust ops (#287).
//! Each op takes `&mut LightComponent`; the caller's accessor
//! (`world.light_mut(id)`) carries the no-op-when-absent semantics (#344).
//!
//! Pure: no wall-clock, no RNG.

use glam::{Vec2, Vec3};

use crate::components::{LightComponent, LightMode, LightType};
use crate::scene::authoring::sanitize::{clamped, finite3, non_negative};

/// Set the light's linear RGB colour.
pub fn set_color(l: &mut LightComponent, rgb: Vec3) {
    l.color = finite3(rgb).max(Vec3::ZERO);
}

/// Set the light's intensity, clamped to `≥ 0` (a negative emit is meaningless).
pub fn set_intensity(l: &mut LightComponent, value: f32) {
    l.intensity = non_negative(value);
}

/// Set the light's range, clamped to `≥ 0`.
pub fn set_range(l: &mut LightComponent, value: f32) {
    l.range = non_negative(value);
}

/// The cone angles (degrees) a new light carries, and a spot seeds when it has none.
pub const DEFAULT_CONES: (f32, f32) = (30.0, 45.0);

/// Set the light's kind (typed — string parsing stays in the API adapter / card).
/// Turning a light into a spot whose cones are both still zero seeds
/// [`DEFAULT_CONES`], so the spot is visible at once whichever surface switched it
/// (#997).
/// Turning one into an Area light makes it `Baked` (#1674): Unity's built-in
/// pipeline bakes area lights only.
pub fn set_type(l: &mut LightComponent, light_type: LightType) {
    l.light_type = light_type;
    if l.light_type == LightType::Spotlight && l.inner_cone == 0.0 && l.outer_cone == 0.0 {
        set_cones(l, DEFAULT_CONES.0, DEFAULT_CONES.1);
    }
    if l.light_type == LightType::Area {
        l.mode = LightMode::Baked;
    }
}

/// Set how the light takes part in baked lighting (Realtime / Mixed / Baked, #438).
/// An Area light refuses anything but `Baked` (#1674) and keeps it: the return is
/// `false` then, for the caller to warn.
pub fn set_mode(l: &mut LightComponent, mode: LightMode) -> bool {
    if l.light_type == LightType::Area && mode != LightMode::Baked {
        return false;
    }
    l.mode = mode;
    true
}

/// The smallest side an Area light keeps (world units): a zero-area panel would
/// emit nothing and divide by nothing useful.
pub const MIN_AREA_SIDE: f32 = 0.01;

/// Set an Area light's width and height (Unity's `areaSize`, #1674), each at
/// least [`MIN_AREA_SIDE`]; NaN or infinite takes that floor.
pub fn set_area_size(l: &mut LightComponent, size: Vec2) {
    let side = |v: f32| {
        if v.is_finite() {
            v.max(MIN_AREA_SIDE)
        } else {
            MIN_AREA_SIDE
        }
    };
    l.area_size = Vec2::new(side(size.x), side(size.y));
}

/// The widest cone half-angle (degrees): Unity's spot angle tops out under 180°
/// full, and the inspector's slider stops here.
pub const MAX_CONE: f32 = 90.0;

/// Set the spotlight cone half-angles (degrees): `outer` in `[0, MAX_CONE]`, and
/// `inner` in `[0, outer]` so the inner cone never exceeds the outer one. NaN or
/// infinite takes `0`. The validation lives HERE, once.
pub fn set_cones(l: &mut LightComponent, inner: f32, outer: f32) {
    l.outer_cone = clamped(outer, 0.0, MAX_CONE);
    l.inner_cone = clamped(inner, 0.0, l.outer_cone);
}

/// Set whether a point or spot light casts shadows through the shadow atlas (#468).
pub fn set_cast_shadows(l: &mut LightComponent, on: bool) {
    l.cast_shadows = on;
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::scene::Scene;

    /// A scene with one light-bearing entity; returns `(scene, id)`.
    fn scene_with_light() -> (Scene, u32) {
        let mut scene = Scene::new();
        let id = scene.add_entity("Lamp".to_string());
        let c = crate::scene::authoring::default_light();
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

    /// Switching to a spot seeds zero cones, whichever surface switched it (#997);
    /// authored cones are left alone.
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

    /// `math.huge` and NaN from Lua take the fallback, never the field (#1361).
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
    }

    /// An Area light is Baked and stays so; its size keeps a floor (#1674).
    #[test]
    fn area_lights_are_baked_and_sized() {
        let (mut scene, id) = scene_with_light();
        let mut e = scene.world.light_mut(id).unwrap();
        set_type(&mut e, LightType::Area);
        assert_eq!(e.mode, LightMode::Baked);
        assert!(!set_mode(&mut e, LightMode::Realtime), "refused");
        assert_eq!(e.mode, LightMode::Baked);
        set_area_size(&mut e, Vec2::new(1.2, f32::NAN));
        assert_eq!(e.area_size, Vec2::new(1.2, MIN_AREA_SIDE));
    }

    #[test]
    fn op_on_lightless_entity_is_a_no_op() {
        let mut scene = Scene::new();
        let id = scene.add_entity("Empty".to_string());
        // The accessor is the no-op gate now: no light, no guard, no write.
        if let Some(mut l) = scene.world.light_mut(id) {
            set_intensity(&mut l, 5.0);
        }
        assert!(!scene.world.has_light(id));
    }
}
