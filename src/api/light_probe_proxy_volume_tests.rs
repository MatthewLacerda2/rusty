//! The `LightProbeProxyVolume` namespace (#1672), through Lua: setters route
//! through the shared ops, getters read back, and an entity without a volume reads
//! `nil` and its setters raise.

use std::cell::RefCell;

use mlua::Lua;

use super::register;
use crate::components::{LightProbeProxyVolumeComponent, ProxyBoundsMode, ProxyQuality};
use crate::scene::Scene;

/// A scene with `id` (carrying a default volume) and `bare` (none); runs `body`
/// with both bound as Lua globals, then hands the scene back.
fn with_locker(body: impl FnOnce(&Lua) -> mlua::Result<()>) -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("locker".to_string());
    let volume = Some(LightProbeProxyVolumeComponent::default());
    scene.world.set_light_probe_proxy_volume(id, volume);
    let bare = scene.add_entity("bare".to_string());
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene).unwrap();
        lua.globals().set("id", id)?;
        lua.globals().set("bare", bare)?;
        body(&lua)
    })
    .unwrap();
    (scene.into_inner(), id, bare)
}

#[test]
fn setters_write_through_the_shared_ops_and_getters_read_back() {
    let (scene, id, bare) = with_locker(|lua| {
        lua.load(
            "LightProbeProxyVolume.SetBoundingBoxMode(id, 'custom')
             LightProbeProxyVolume.SetSize(id, 1, -2, 0)
             LightProbeProxyVolume.SetOrigin(id, 0, 1, 0)
             LightProbeProxyVolume.SetResolutionMode(id, 'Custom')
             LightProbeProxyVolume.SetGridResolution(id, 1, 5, 64)
             LightProbeProxyVolume.SetProbeDensity(id, 0)
             LightProbeProxyVolume.SetQuality(id, 'L0')",
        )
        .exec()?;
        let size: (f32, f32, f32) = lua
            .load("return LightProbeProxyVolume.GetSize(id)")
            .eval()?;
        assert_eq!(size, (1.0, 2.0, 1e-3), "a size is positive and never flat");
        let get = "return LightProbeProxyVolume.GetGridResolution(id)";
        let dims: (u32, u32, u32) = lua.load(get).eval()?;
        assert_eq!(dims, (1, 8, 32), "powers of two up to 32");
        let get = "return LightProbeProxyVolume.GetProbeDensity(id)";
        assert_eq!(lua.load(get).eval::<f32>()?, 0.1);
        Ok(())
    });
    let v = scene.world.light_probe_proxy_volume(id).unwrap();
    let modes = (v.bounds_mode, v.quality);
    assert_eq!(modes, (ProxyBoundsMode::Custom, ProxyQuality::Low));
    assert_eq!(v.origin, glam::Vec3::Y);
    let attached = scene.world.light_probe_proxy_volume(bare).is_some();
    assert!(!attached, "setters never attach");
}

#[test]
fn a_missing_volume_reads_nil_and_a_bad_name_or_entity_raises() {
    with_locker(|lua| {
        let get = "return LightProbeProxyVolume.GetQuality(bare)";
        assert!(
            lua.load(get).eval::<mlua::Value>()?.is_nil(),
            "no volume reads nil"
        );
        let raised = lua
            .load("LightProbeProxyVolume.SetQuality(bare, 'Low')")
            .exec();
        let message = raised.unwrap_err().to_string();
        assert!(
            message.contains("has no LightProbeProxyVolume"),
            "{message}"
        );
        let bad = lua
            .load("LightProbeProxyVolume.SetQuality(id, 'L2')")
            .exec();
        let message = bad.unwrap_err().to_string();
        assert!(message.contains("Low, Normal"), "{message}");
        Ok(())
    });
}
