//! The `LightProbeProxyVolume` namespace (#1672), through Lua: setters route
//! through the shared ops, getters read back, and an entity without a volume reads
//! `nil` and its setters raise.

use std::cell::RefCell;

use mlua::Lua;

use super::register;
use crate::components::{LightProbeProxyVolumeComponent, ProxyBoundsMode, ProxyQuality};
use crate::scene::Scene;

#[test]
fn setters_write_through_the_shared_ops_and_getters_read_back() {
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
        let dims: (u32, u32, u32) = lua
            .load("return LightProbeProxyVolume.GetGridResolution(id)")
            .eval()?;
        assert_eq!(dims, (1, 8, 32), "powers of two up to 32");
        let density: f32 = lua
            .load("return LightProbeProxyVolume.GetProbeDensity(id)")
            .eval()?;
        assert_eq!(density, 0.1);
        let none: mlua::Value = lua
            .load("return LightProbeProxyVolume.GetQuality(bare)")
            .eval()?;
        assert!(none.is_nil(), "no volume reads nil");
        let raised = lua
            .load("LightProbeProxyVolume.SetQuality(bare, 'Low')")
            .exec();
        assert!(raised
            .unwrap_err()
            .to_string()
            .contains("has no LightProbeProxyVolume"));
        let bad = lua
            .load("LightProbeProxyVolume.SetQuality(id, 'L2')")
            .exec();
        assert!(bad.unwrap_err().to_string().contains("Low, Normal"));
        Ok(())
    })
    .unwrap();
    let s = scene.borrow();
    let v = s.world.light_probe_proxy_volume(id).unwrap();
    assert_eq!(
        (v.bounds_mode, v.quality),
        (ProxyBoundsMode::Custom, ProxyQuality::Low)
    );
    assert_eq!(v.origin, glam::Vec3::Y);
    assert!(
        s.world.light_probe_proxy_volume(bare).is_none(),
        "setters never attach"
    );
}
