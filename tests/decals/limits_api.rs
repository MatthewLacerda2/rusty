//! The decal budgets through the `Decals` API (#1517): per owner, so one owner's
//! marks never evict another's; `SetLimits` / `GetLimits`; and entering Play
//! applying the project's budgets.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::core::settings::decals::DecalLimits;
use rusty::scene::Scene;

fn eval<T: mlua::FromLuaMulti>(scene: &RefCell<Scene>, script: &str) -> mlua::Result<T> {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::decals::register(&lua, scope, scene).unwrap();
        lua.load(script).eval()
    })
}

#[test]
fn shooting_the_floor_keeps_the_crate_mark() {
    let mut scene = Scene::new();
    let floor = scene.add_entity("Floor".into());
    let crate_id = scene.add_entity("Crate".into());
    let scene = RefCell::new(scene);
    let script = format!(
        "local mark = Decals.Spawn(0,0,0, 0,1,0, {{ owner = {crate_id} }})
         for i = 1, 257 do Decals.Spawn(i * 0.01,0,0, 0,1,0, {{ owner = {floor} }}) end
         return Decals.Count(), Decals.Remove(mark)"
    );
    let (count, removed): (u32, bool) = eval(&scene, &script).unwrap();
    assert_eq!(count, 259, "256 whole floor marks, one fading, the crate's");
    assert!(removed, "the crate's mark was never evicted");
}

#[test]
fn set_limits_takes_a_partial_table_and_get_limits_reads_it_back() {
    let scene = RefCell::new(Scene::new());
    let got: (u32, u32, u32) = eval(
        &scene,
        "Decals.SetLimits{ world = 2 }
         local l = Decals.GetLimits()
         return l.per_owner, l.world, l.total",
    )
    .unwrap();
    assert_eq!(got, (256, 2, 4096));
}

#[test]
fn lowering_the_world_limit_fades_the_oldest_excess() {
    let scene = RefCell::new(Scene::new());
    eval::<()>(
        &scene,
        "for i = 1, 5 do Decals.Spawn(i,0,0, 0,1,0) end
         Decals.SetLimits{ world = 2 }",
    )
    .unwrap();
    let fading: Vec<bool> = scene.borrow().decals.iter().map(|d| d.retiring).collect();
    assert_eq!(fading, [true, true, true, false, false]);
}

#[test]
fn an_unknown_limit_is_an_error_naming_the_valid_ones() {
    let scene = RefCell::new(Scene::new());
    let err = eval::<()>(&scene, "Decals.SetLimits{ owner = 3 }").unwrap_err();
    assert!(err.to_string().contains("per_owner, world, total"), "{err}");
    assert_eq!(scene.borrow().decals.limits(), DecalLimits::default());
}

#[test]
fn entering_play_applies_the_project_budgets() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let mut game = crate::play::playing(Rc::clone(&scene), 20.0);
    let project = DecalLimits {
        max_per_owner: 3,
        max_world: 5,
        max_total: 7,
    };
    game.resources
        .application
        .borrow_mut()
        .set_decal_limits(project);
    game.tick(1.0 / 60.0); // enter Play
    assert_eq!(scene.borrow().decals.limits(), project);
}
