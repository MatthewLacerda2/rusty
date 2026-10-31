//! The Scene Settings panel's ops vs their bindings: `Graphics.{Get,Set}Fog*` vs
//! `authoring::fog` (#437), `Lighting`'s environment vs `authoring::environment`
//! (#1612).

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;
use rusty::core::settings::GraphicsSettings;
use rusty::scene::authoring::environment as env_ops;
use rusty::scene::authoring::fog as fog_ops;
use rusty::scene::{FogMode, Scene};

#[test]
fn fog_api_and_shared_op_converge_without_a_volume() -> Result<(), Box<dyn std::error::Error>> {
    // Scene fog is scene-level: a bare scene (no post-processing volume) takes it.
    let lua_scene = Rc::new(RefCell::new(Scene::new()));
    let quality = Rc::new(RefCell::new(GraphicsSettings::default()));
    let app = RefCell::default();
    let lua = Lua::new();
    let read: (String, f32, f32, f32, f32) = lua.scope(|s| {
        rusty::api::graphics::register(&lua, s, &lua_scene, &quality, &app).unwrap();
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

#[test]
fn environment_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let lua_scene = RefCell::new(Scene::new());
    let lua = Lua::new();
    let read: (String, f32, f32, String) = lua.scope(|s| {
        rusty::api::lighting::register(&lua, s, &lua_scene).unwrap();
        lua.load(
            r#"
            Lighting.SetSkybox("sky/sunset.hdr")
            Lighting.SetAmbientColor(-1, 0.25, 2)
            Lighting.SetAmbientColor(0 / 0, 1, 1) -- NaN: refused whole
            Lighting.SetAmbientIntensity(-3)
            Lighting.SetAmbientIntensity(1.5)
            local sky = Lighting.GetSkybox()
            local _, g, _ = Lighting.GetAmbientColor()
            local i = Lighting.GetAmbientIntensity()
            Lighting.SetSkybox("")
            return sky, g, i, Lighting.GetSkybox()
        "#,
        )
        .eval()
    })?;

    let mut op_scene = Scene::new();
    env_ops::set_skybox(&mut op_scene, "sky/sunset.hdr");
    env_ops::set_ambient_color(&mut op_scene, Vec3::new(-1.0, 0.25, 2.0));
    env_ops::set_ambient_color(&mut op_scene, Vec3::new(f32::NAN, 1.0, 1.0));
    env_ops::set_ambient_intensity(&mut op_scene, 1.5);
    env_ops::set_skybox(&mut op_scene, "");

    let lua_scene = lua_scene.borrow();
    assert_eq!(lua_scene.ambient_color, Vec3::new(0.0, 0.25, 2.0));
    assert_eq!(lua_scene.ambient_color, op_scene.ambient_color);
    assert_eq!(lua_scene.ambient_intensity, op_scene.ambient_intensity);
    assert_eq!(lua_scene.skybox_path, op_scene.skybox_path);
    assert_eq!(
        read,
        ("sky/sunset.hdr".to_string(), 0.25, 1.5, String::new()),
        "an empty path is the procedural sky"
    );
    Ok(())
}
