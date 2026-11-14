//! `Reflection.SetFallback` / `GetFallback` (#1697): one fallback probe a scene
//! names, as Unity's default reflection probe; and the volume's `Set` fields (#1995),
//! kept here for the size cap of `reflection_tests.rs`.

use crate::api::bind;
use std::cell::RefCell;
use std::rc::Rc;

use super::*;

#[test]
fn set_fallback_names_one_probe_and_nil_clears_it() {
    let scene = Rc::new(RefCell::new(Scene::default()));
    let lua = Lua::new();
    register(&lua, bind(&lua, &scene)).unwrap();
    let none: Option<u32> = lua.load("return Reflection.GetFallback()").eval().unwrap();
    assert_eq!(none, None);
    lua.load("Reflection.Add(0,0,0, 1,1,1); Reflection.Add(5,0,0, 1,1,1)")
        .exec()
        .unwrap();
    let ok: bool = lua.load("return Reflection.SetFallback(0)").eval().unwrap();
    assert!(ok);
    let ok: bool = lua.load("return Reflection.SetFallback(1)").eval().unwrap();
    assert!(ok);
    let got: Option<u32> = lua.load("return Reflection.GetFallback()").eval().unwrap();
    assert_eq!(got, Some(1), "naming another moves the flag");
    let flag: bool = lua
        .load("return Reflection.Get(0).fallback")
        .eval()
        .unwrap();
    assert!(!flag);
    let out: bool = lua.load("return Reflection.SetFallback(7)").eval().unwrap();
    assert!(!out, "an index past the set changes nothing");
    assert_eq!(scene.borrow().reflection_probes.fallback(), Some(1));
    lua.load("Reflection.SetFallback(nil)").exec().unwrap();
    assert_eq!(scene.borrow().reflection_probes.fallback(), None);
}

/// A new cube drops the old one's stored irradiance (#1697), through `SetCubemap`
/// and `Set{ cubemap }` alike; a round trip that keeps the path keeps it. The
/// fallback stays manual through `Set{ manual = false }`, so a bake keeps it.
#[test]
fn repointing_a_probe_drops_its_irradiance_and_the_fallback_stays_manual() {
    use rusty_scene::lighting::sh::Sh9;
    let scene = Rc::new(RefCell::new(Scene::default()));
    let lua = Lua::new();
    register(&lua, bind(&lua, &scene)).unwrap();
    let run = |code: &str| lua.load(code).exec().unwrap();
    let normalised = || -> bool {
        lua.load("return Reflection.Get(0).normalised")
            .eval()
            .unwrap()
    };
    let store = || {
        let set = &mut scene.borrow_mut().reflection_probes;
        set.set_cubemap(0, "day.ktx2".into());
        set.set_irradiance(0, Some(Sh9::zero()));
    };
    run("Reflection.Add(0,0,0, 1,1,1)");
    store();
    run("local p = Reflection.Get(0); Reflection.Set(0, p)");
    assert!(normalised(), "a round trip keeps it");
    run("Reflection.SetCubemap(0, 'night.ktx2')");
    assert!(!normalised(), "SetCubemap to another cube");
    store();
    run("Reflection.SetCubemap(0, '')");
    assert!(!normalised(), "SetCubemap to none");
    store();
    run("Reflection.Set(0, { cubemap = 'night.ktx2' })");
    assert!(!normalised(), "Set with a cubemap");
    run("Reflection.SetFallback(0); Reflection.Set(0, { manual = false })");
    assert!(scene.borrow().reflection_probes.probes[0].manual);
}

#[test]
fn set_and_get_the_volume_shape_and_rotation() {
    let scene = Rc::new(RefCell::new(Scene::default()));
    let lua = Lua::new();
    register(&lua, bind(&lua, &scene)).unwrap();
    lua.load(
        "Reflection.Add(0,0,0, 1,1,1) Reflection.Set(0, Reflection.Get(0))
        Reflection.Add(0,0,0, 1,1,1)
        Reflection.Set(1, { shape = 'Sphere', radius = 3, rotation = { x = 0, y = 90, z = 0 } })",
    )
    .exec()
    .unwrap();
    let r: (String, f32, f32) = lua
        .load("local p = Reflection.Get(1) return p.shape, p.radius, p.rotation.y")
        .eval()
        .unwrap();
    assert_eq!((r.0.as_str(), r.1), ("Sphere", 3.0));
    assert!((r.2 - 90.0).abs() < 1e-3, "{r:?}");
    assert_eq!(
        scene.borrow().reflection_probes.probes[0].rotation,
        glam::Quat::IDENTITY
    );
    assert!(lua
        .load("Reflection.Set(0, { shape = 'Cone' })")
        .exec()
        .is_err());
}
