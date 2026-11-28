//! Authoring several scene files in edit mode, through the Lua API an agent drives.

use super::fixture::world_at;
use super::scene_load_additive_tests::scenes;
use super::GameWorld;

fn eval(gw: &GameWorld, code: &str) -> String {
    gw.script_manager().eval(code).unwrap()
}

fn fails(gw: &GameWorld, code: &str) -> String {
    gw.script_manager().eval(code).unwrap_err()
}

fn names(path: &str) -> Vec<String> {
    let data = rusty_scene::read_scene_file(path).unwrap();
    data.entities.iter().map(|e| e.name.clone()).collect()
}

fn of(gw: &GameWorld, name: &str) -> String {
    eval(
        gw,
        &format!("return Scene.Of(Scene.FindEntityByName('{name}'))"),
    )
}

#[test]
fn an_edit_mode_additive_load_joins_now_and_save_writes_each_file() {
    let [base, a, _] = scenes("authoring_save");
    let (mut gw, _) = world_at(&base);
    gw.init_edit_runtime().unwrap();
    eval(&gw, &format!("Scene.Load('{a}', 'Additive')"));
    assert_eq!(of(&gw, "AChild"), a);
    eval(
        &gw,
        &format!("Scene.MoveTo(Scene.FindEntityByName('Base'), '{a}')"),
    );
    assert_eq!(of(&gw, "BaseChild"), a, "the children follow their root");
    assert_eq!(
        eval(&gw, "return Scene.Save()"),
        base,
        "the active scene's path"
    );
    assert!(names(&base).is_empty());
    assert_eq!(names(&a), ["A", "AChild", "Base", "BaseChild"]);
    let child = fails(
        &gw,
        &format!("Scene.MoveTo(Scene.FindEntityByName('AChild'), '{base}')"),
    );
    assert!(child.contains("only a root"), "{child}");
}

#[test]
fn play_then_stop_keeps_the_loaded_scenes_and_their_members() {
    let [base, a, _] = scenes("authoring_play");
    let (mut gw, _) = world_at(&base);
    gw.init_edit_runtime().unwrap();
    eval(&gw, &format!("Scene.Load('{a}', 'Additive')"));
    eval(&gw, &format!("Scene.SetActiveScene('{a}')"));
    gw.set_playing(true);
    gw.tick(1.0 / 60.0);
    gw.set_playing(false);
    gw.poll_transition();
    let loaded = eval(&gw, "return table.concat(Scene.GetLoaded(), ' ')");
    assert_eq!(loaded, format!("{base} {a}"));
    assert_eq!(of(&gw, "A"), a);
    assert_eq!(eval(&gw, "return Scene.GetActivePath()"), a);
}

#[test]
fn new_adds_an_empty_scene_that_save_writes_and_unload_removes_at_once() {
    let [base, a, _] = scenes("authoring_new");
    let (mut gw, _) = world_at(&base);
    gw.init_edit_runtime().unwrap();
    let fresh = a.replace("a.scene", "fresh.scene");
    eval(
        &gw,
        &format!("Scene.New('{fresh}'); Scene.SetActiveScene('{fresh}')"),
    );
    eval(&gw, "Scene.CreateEntity('Lamp')");
    assert_eq!(of(&gw, "Lamp"), fresh);
    eval(&gw, "Scene.Save()");
    assert_eq!(names(&fresh), ["Lamp"]);
    assert_eq!(names(&base), ["Base", "BaseChild"], "the base kept its own");
    let taken = fails(&gw, &format!("Scene.New('{a}')"));
    assert!(taken.contains("already at"), "{taken}");
    eval(&gw, &format!("Scene.Unload('{fresh}')"));
    assert_eq!(
        eval(&gw, "return tostring(Scene.FindEntityByName('Lamp'))"),
        "nil"
    );
    let parent = fails(
        &gw,
        &format!(
            "Scene.Load('{a}', 'Additive'); \
             Scene.SetParent(Scene.FindEntityByName('A'), Scene.FindEntityByName('Base'))"
        ),
    );
    assert!(parent.contains("another loaded scene"), "{parent}");
}
