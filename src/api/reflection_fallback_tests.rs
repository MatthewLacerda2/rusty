//! `Reflection.SetFallback` / `GetFallback` (#1697): one fallback probe a scene
//! names, as Unity's default reflection probe.

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
