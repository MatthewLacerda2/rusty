//! A mesh's small-object cull (#1714) read from Lua: `Component.Get` reports `0`
//! for a setting the file leaves out, as `Mesh.Get*` does, so a script compares it
//! without a nil check.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::scene::Scene;

use crate::play::{playing, run};

#[test]
fn component_get_reads_an_unset_cull_as_zero_like_mesh_get() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let mut game = playing(scene.clone(), 1.0);
    run(&mut game, 1);
    let eval = |line: &str| game.script_manager().eval(line);
    let read = eval(
        r#"can = Scene.CreateEntity("Can", "Cylinder")
        local m = Component.Get(can, "Mesh")
        local before = m.max_draw_distance .. " " .. m.min_screen_size
        Mesh.SetMaxDrawDistance(can, 30)
        local after = Component.Get(can, "Mesh").max_draw_distance
        return before .. " " .. after .. " " .. Mesh.GetMinScreenSize(can)"#,
    )
    .unwrap();
    assert_eq!(read, "0.0 0.0 30.0 0.0");
}
