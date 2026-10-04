//! Decals that stick and age (#639), through the `Decals` API and the real play
//! loop (`GameWorld::tick`): `Spawn` returns an id `Remove` takes, an owned decal
//! follows its entity and goes with it, and a lifetime fades out on fixed-dt ticks.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;
use rusty::app::GameWorld;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

fn eval<T: mlua::FromLuaMulti>(scene: &RefCell<Scene>, script: &str) -> mlua::Result<T> {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::decals::register(&lua, scope, scene).unwrap();
        lua.load(script).eval()
    })
}

fn playing(scene: Scene) -> GameWorld {
    let nav = NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0);
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    game.set_playing(true);
    game.tick(DT); // enter Play
    game
}

#[test]
fn spawn_returns_an_id_that_remove_takes() {
    let scene = RefCell::new(Scene::new());
    let (a, b): (u32, u32) = eval(
        &scene,
        "return Decals.Spawn(0,0,0, 0,1,0), Decals.Spawn(1,0,0, 0,1,0, { size = 2 })",
    )
    .unwrap();
    assert_ne!(a, b);
    let removed: (bool, bool) = eval(
        &scene,
        &format!("return Decals.Remove({a}), Decals.Remove({a})"),
    )
    .unwrap();
    assert_eq!(removed, (true, false), "the second remove finds nothing");
    assert_eq!(scene.borrow().decals.len(), 1);
    assert_eq!(scene.borrow().decals[0].id, b);
}

#[test]
fn an_unknown_owner_is_an_error_and_stamps_nothing() {
    let scene = RefCell::new(Scene::new());
    let err = eval::<()>(&scene, "Decals.Spawn(0,0,0, 0,1,0, { owner = 42 })").unwrap_err();
    assert!(err.to_string().contains("42"), "{err}");
    assert!(scene.borrow().decals.is_empty());
}

#[test]
fn an_owned_decal_follows_its_entity_and_goes_when_it_is_destroyed() {
    let mut scene = Scene::new();
    let crate_id = scene.add_entity("Crate".into());
    let scene = RefCell::new(scene);
    eval::<u32>(
        &scene,
        &format!("return Decals.Spawn(0,0.5,0, 0,1,0, {{ owner = {crate_id} }})"),
    )
    .unwrap();
    let mut game = playing(scene.into_inner());
    let at = |game: &GameWorld| {
        let s = game.scene().borrow();
        s.decal_pose(&s.decals[0]).unwrap().centre()
    };
    game.scene()
        .borrow_mut()
        .world
        .transform_mut(crate_id)
        .unwrap()
        .position = Vec3::new(3.0, 0.0, 0.0);
    game.tick(DT);
    assert!(
        at(&game).abs_diff_eq(Vec3::new(3.0, 0.5, 0.0), 1e-5),
        "{}",
        at(&game)
    );
    game.scene().borrow_mut().destroy_entity(crate_id);
    game.tick(DT);
    assert!(
        game.scene().borrow().decals.is_empty(),
        "dropped with its owner"
    );
}

#[test]
fn a_lifetime_fades_out_over_fixed_ticks_then_the_decal_is_gone() {
    let scene = RefCell::new(Scene::new());
    eval::<u32>(
        &scene,
        "return Decals.Spawn(0,0,0, 0,1,0, { lifetime = 1, fade = 0.5 })",
    )
    .unwrap();
    let mut game = playing(scene.into_inner());
    let opacity = |game: &GameWorld| game.scene().borrow().decals[0].opacity();
    for _ in 0..44 {
        game.tick(DT);
    } // 45 ticks of age: 0.75 s
    assert!((opacity(&game) - 0.5).abs() < 1e-3, "{}", opacity(&game));
    // One tick past the lifetime: sixty summed f32 sixtieths fall just short of 1.
    for _ in 0..16 {
        game.tick(DT);
    }
    assert!(
        game.scene().borrow().decals.is_empty(),
        "gone at its lifetime"
    );
}
