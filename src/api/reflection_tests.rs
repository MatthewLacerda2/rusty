//! Tests for the `Reflection` namespace (`reflection.rs`).

use std::cell::RefCell;

use super::*;

fn empty_scene() -> RefCell<Scene> {
    RefCell::new(Scene::default())
}

#[test]
fn add_count_remove() {
    let scene = empty_scene();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene).unwrap();
        lua.load("Reflection.Add(0,0,0, 5,5,5)").exec().unwrap();
        lua.load("Reflection.Add(10,0,0, 5,5,5)").exec().unwrap();
        let n: u32 = lua.load("return Reflection.Count()").eval().unwrap();
        assert_eq!(n, 2);
        lua.load("Reflection.Remove(0)").exec().unwrap();
        let n: u32 = lua.load("return Reflection.Count()").eval().unwrap();
        assert_eq!(n, 1);
        Ok(())
    })
    .unwrap();
}

#[test]
fn set_box_and_cubemap() {
    let scene = empty_scene();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene).unwrap();
        lua.load("Reflection.Add(0,0,0, 1,1,1)").exec().unwrap();
        lua.load("Reflection.SetBox(0, -3,-3,-3, 3,3,3)")
            .exec()
            .unwrap();
        lua.load("Reflection.SetCubemap(0, 'room.ktx2')")
            .exec()
            .unwrap();
        Ok(())
    })
    .unwrap();
    let s = scene.borrow();
    let p = &s.reflection_probes.probes[0];
    assert_eq!(p.box_min, Vec3::splat(-3.0));
    assert_eq!(p.box_max, Vec3::splat(3.0));
    assert_eq!(p.cubemap_path, "room.ktx2");
}

#[test]
fn set_and_get_every_setting() {
    let scene = empty_scene();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene).unwrap();
        lua.load(
            "Reflection.Add(0,0,0, 1,1,1)
             Reflection.Set(0, { boxProjection = false, blendDistance = 2,
               importance = 3, resolution = 300, nearClip = 0.2, farClip = 50,
               shadows = false, intensity = 1.5, position = { x = 1, y = 2, z = 3 } })",
        )
        .exec()
        .unwrap();
        let r: (bool, u32, f32, f32, bool) = lua
            .load("local p = Reflection.Get(0) return p.boxProjection, p.resolution, p.farClip, p.position.y, Reflection.Get(4) == nil")
            .eval()
            .unwrap();
        assert_eq!(r, (false, 256, 50.0, 2.0, true));
        lua.load("Reflection.Set(0, { resolution = 0 })").exec().unwrap();
        Ok(())
    })
    .unwrap();
    let s = scene.borrow();
    let p = &s.reflection_probes.probes[0];
    assert_eq!((p.importance, p.resolution, p.intensity), (3, None, 1.5));
    assert_eq!(
        (p.blend_distance, p.near_clip, p.shadows),
        (2.0, 0.2, false)
    );
}
