//! The procedural sky's settings (#1620): `Lighting.{Get,Set}Sky*Color` and
//! `Lighting.{Get,Set}Cloud*` vs `authoring::environment`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::scene::authoring::environment as env_ops;
use rusty::scene::Scene;

type Read = (Option<f32>, f32, f32, bool, f32);

#[test]
fn sky_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let lua_scene = RefCell::new(Scene::new());
    let lua = Lua::new();
    let read: Read = lua.scope(|s| {
        rusty::api::lighting::register(&lua, s, &lua_scene).unwrap();
        lua.load(
            r#"
            local unset = Lighting.GetSkyZenithColor()
            Lighting.SetSkyZenithColor(-1, 0.4, 0.9)
            Lighting.SetSkyZenithColor(0 / 0, 1, 1) -- NaN: refused whole
            Lighting.SetSkyHorizonColor(1, 0.8, 0.6)
            Lighting.SetSkyGroundColor(0.2, 0.2, 0.2)
            Lighting.SetSkyGroundColor() -- follows the zenith again
            Lighting.SetCloudCoverage(3)
            Lighting.SetCloudOpacity(0.25)
            Lighting.SetCloudOpacity(math.huge) -- refused
            local _, g, _ = Lighting.GetSkyZenithColor()
            local partial = pcall(Lighting.SetSkyHorizonColor, 1, 2)
            return unset, g, Lighting.GetCloudCoverage(), partial,
                Lighting.GetCloudOpacity()
        "#,
        )
        .eval()
    })?;

    let mut op_scene = Scene::new();
    let sky = &mut op_scene.sky;
    env_ops::set_sky_zenith(sky, Some(Vec3::new(-1.0, 0.4, 0.9)));
    env_ops::set_sky_zenith(sky, Some(Vec3::new(f32::NAN, 1.0, 1.0)));
    env_ops::set_sky_horizon(sky, Some(Vec3::new(1.0, 0.8, 0.6)));
    env_ops::set_sky_ground(sky, Some(Vec3::splat(0.2)));
    env_ops::set_sky_ground(sky, None);
    env_ops::set_cloud_coverage(sky, 3.0);
    env_ops::set_cloud_opacity(sky, 0.25);
    env_ops::set_cloud_opacity(sky, f32::INFINITY);

    let lua_sky = lua_scene.borrow().sky;
    assert_eq!(lua_sky, op_scene.sky);
    assert_eq!(lua_sky.zenith, Some(Vec3::new(0.0, 0.4, 0.9)));
    assert_eq!(lua_sky.ground, None);
    assert_eq!(
        read,
        (None, 0.4, 1.0, false, 0.25),
        "a partial colour errors"
    );
    Ok(())
}
