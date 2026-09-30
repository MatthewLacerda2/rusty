//! `Graphics.{Get,Set}Fog*` bindings vs `authoring::fog` (#437).

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;
use rusty::core::quality::QualityPreset;
use rusty::scene::authoring::fog as fog_ops;
use rusty::scene::{FogMode, Scene};

#[test]
fn fog_api_and_shared_op_converge_without_a_volume() -> Result<(), Box<dyn std::error::Error>> {
    // Scene fog is scene-level: a bare scene (no visual-correction volume) takes it.
    let lua_scene = Rc::new(RefCell::new(Scene::new()));
    let quality = Rc::new(RefCell::new(QualityPreset::High));
    let lua = Lua::new();
    let read: (String, f32, f32, f32, f32) = lua.scope(|s| {
        rusty::api::graphics::register(&lua, s, &lua_scene, &quality).unwrap();
        lua.load(
            r#"
            Graphics.SetFogMode("ExponentialSquared")
            Graphics.SetFogMode("Thick") -- unknown: ignored
            Graphics.SetFogColor(-1, 0.5, 2)
            Graphics.SetFogDensity(-0.1)
            Graphics.SetFogStart(5)
            Graphics.SetFogEnd(-1)
            Graphics.SetFogHeightFalloff(0.3)
            Graphics.SetFogBaseHeight(-2)
            local _, g, _ = Graphics.GetFogColor()
            return Graphics.GetFogMode(), g, Graphics.GetFogDensity(),
                Graphics.GetFogStart(), Graphics.GetFogBaseHeight()
        "#,
        )
        .eval()
    })?;

    let mut op_scene = Scene::new();
    let fog = &mut op_scene.fog;
    fog_ops::set_mode(fog, FogMode::ExponentialSquared);
    fog_ops::set_color(fog, Vec3::new(-1.0, 0.5, 2.0));
    fog_ops::set_density(fog, -0.1);
    fog_ops::set_start(fog, 5.0);
    fog_ops::set_end(fog, -1.0);
    fog_ops::set_height_falloff(fog, 0.3);
    fog_ops::set_base_height(fog, -2.0);

    assert_eq!(lua_scene.borrow().fog, op_scene.fog);
    assert_eq!(
        op_scene.fog.color,
        Vec3::new(0.0, 0.5, 2.0),
        "single-sourced clamp"
    );
    assert_eq!(
        read,
        ("ExponentialSquared".to_string(), 0.5, 0.0, 5.0, -2.0)
    );
    Ok(())
}
