//! `Light.*` bindings vs `authoring::light`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::components::{LightComponent, LightType};
use rusty::scene::authoring::light as light_ops;
use rusty::scene::Scene;

/// Attach a default-ish light to a fresh entity named `name`; returns its id.
fn entity_with_light(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.set_light(
        id,
        Some(LightComponent {
            light_type: LightType::Point,
            color: Vec3::ONE,
            intensity: 1.0,
            range: 5.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
        }),
    );
    id
}

#[test]
fn light_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_light(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_light(&mut scene.borrow_mut(), "ViaOp");

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::light::register(&lua, s, &scene).unwrap();
        lua.load(format!(
            r#"
            Light.SetColor({via_lua}, 0.2, 0.4, 0.6)
            Light.SetIntensity({via_lua}, -5.0)
            Light.SetRange({via_lua}, -2.0)
            Light.SetType({via_lua}, "Directional")
        "#
        ))
        .exec()
        .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.light_mut(via_op).unwrap();
        light_ops::set_color(&mut e, Vec3::new(0.2, 0.4, 0.6));
        light_ops::set_intensity(&mut e, -5.0); // clamped to 0 by the shared op
        light_ops::set_range(&mut e, -2.0); // clamped to 0 by the shared op
        light_ops::set_type(&mut e, LightType::Directional);
    }

    let sc = scene.borrow();
    let a = sc.world.light(via_lua).unwrap().clone();
    let b = sc.world.light(via_op).unwrap().clone();
    assert_eq!(a.color, b.color);
    assert_eq!(a.intensity, b.intensity);
    assert_eq!(a.range, b.range);
    assert_eq!(a.light_type, b.light_type);
    // The clamp is single-sourced in the op the binding calls.
    assert_eq!(a.intensity, 0.0);
    assert_eq!(a.range, 0.0);
    Ok(())
}
