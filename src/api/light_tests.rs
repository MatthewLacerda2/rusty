//! Tests for the `Light` namespace (`api::light`).

use std::cell::RefCell;

use super::*;
use crate::components::LightComponent;

/// Build a scene with one light-bearing entity; returns `(scene, id)`.
fn scene_with_light() -> (RefCell<Scene>, u32) {
    let mut scene = Scene::default();
    let id = scene.add_entity("Lamp".to_string());
    scene.world.set_light(
        id,
        Some(LightComponent {
            light_type: LightType::Point,
            color: Vec3::ONE,
            intensity: 1.5,
            range: 10.0,
            inner_cone: 30.0,
            outer_cone: 45.0,
            cast_shadows: false,
            mode: Default::default(),
        }),
    );
    (RefCell::new(scene), id)
}

#[test]
fn set_get_color_intensity_range_type() {
    let (scene, id) = scene_with_light();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene).unwrap();

        lua.load(format!("Light.SetColor({id}, 0.2, 0.4, 0.6)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetIntensity({id}, 3.0)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetRange({id}, 25.0)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetType({id}, 'Directional')"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetCastShadows({id}, true)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetMode({id}, 'baked')"))
            .exec()
            .unwrap();
        let mode: String = lua.load(format!("return Light.GetMode({id})")).eval()?;
        assert_eq!(mode, "Baked");
        Ok(())
    })
    .unwrap();

    let e = scene.borrow();
    let light = e.world.light(id).unwrap().clone();
    assert_eq!(light.color, Vec3::new(0.2, 0.4, 0.6));
    assert_eq!(light.intensity, 3.0);
    assert_eq!(light.range, 25.0);
    assert_eq!(light.light_type, LightType::Directional);
    assert!(light.cast_shadows);
    assert_eq!(light.mode, LightMode::Baked);
}

#[test]
fn clamps_negatives_and_ignores_unknown_type() {
    let (scene, id) = scene_with_light();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene).unwrap();

        lua.load(format!("Light.SetIntensity({id}, -5.0)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetRange({id}, -2.0)"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetType({id}, 'Bogus')"))
            .exec()
            .unwrap();
        lua.load(format!("Light.SetMode({id}, 'Bogus')"))
            .exec()
            .unwrap();
        Ok(())
    })
    .unwrap();

    let e = scene.borrow();
    let light = e.world.light(id).unwrap().clone();
    assert_eq!(light.intensity, 0.0);
    assert_eq!(light.range, 0.0);
    // Unknown name kept the original Point type.
    assert_eq!(light.light_type, LightType::Point);
    assert_eq!(
        light.mode,
        LightMode::Mixed,
        "unknown mode kept the default"
    );
}
