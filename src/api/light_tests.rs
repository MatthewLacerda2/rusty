//! Tests for the `Light` namespace (`api::light`).

use std::cell::RefCell;

use super::*;

/// Build a scene with one light-bearing entity; returns `(scene, id)`.
fn scene_with_light() -> (RefCell<Scene>, u32) {
    let mut scene = Scene::default();
    let id = scene.add_entity("Lamp".to_string());
    scene
        .world
        .set_light(id, Some(crate::scene::authoring::default_light()));
    (RefCell::new(scene), id)
}

#[test]
fn set_get_color_intensity_range_type() {
    let (scene, id) = scene_with_light();
    let console = RefCell::new(ConsoleLogs::new());
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &console).unwrap();

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
    let console = RefCell::new(ConsoleLogs::new());
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &console).unwrap();

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

/// An Area light: Baked, refusing the rest with a warning, and sized (#1674).
#[test]
fn area_light_is_baked_and_sized() {
    let (scene, id) = scene_with_light();
    let console = RefCell::new(ConsoleLogs::new());
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &console).unwrap();
        lua.load(format!(
            "Light.SetType({id}, 'Area') Light.SetMode({id}, 'Mixed') \
             Light.SetAreaSize({id}, 1.2, 0.6)"
        ))
        .exec()
        .unwrap();
        let (w, h): (f32, f32) = lua.load(format!("return Light.GetAreaSize({id})")).eval()?;
        assert_eq!((w, h), (1.2, 0.6));
        let mode: String = lua.load(format!("return Light.GetMode({id})")).eval()?;
        assert_eq!(mode, "Baked", "GetMode reports the mode the light acts on");
        Ok(())
    })
    .unwrap();
    assert_eq!(
        scene.borrow().world.light(id).unwrap().effective_mode(),
        LightMode::Baked
    );
    let warned: Vec<_> = console.borrow().since(0).map(|(m, _)| m.clone()).collect();
    assert!(
        warned
            .iter()
            .any(|m| m.contains("Area light is always Baked")),
        "{warned:?}"
    );
}
