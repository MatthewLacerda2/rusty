//! `Editor.Undo/Redo/UndoName/RedoName` (#1631): every edit-mode API call is one
//! Undo step, undone and redone to the very scene document it started from.

use super::super::session::Session;

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

fn run(s: &mut Session, line: &str) -> String {
    s.command(line)
        .unwrap_or_else(|e| panic!("`{line}` failed: {e}"))
}

/// The scene as `Scene.Save` would write it, but for the id allocator: an
/// undone create's id stays retired, so the allocator may only have moved on.
fn document(s: &Session) -> String {
    let scene = s.world().world.scene.borrow();
    let mut data = crate::scene::to_scene_data(&scene);
    data.next_entity_id = 0;
    serde_json::to_string(&data).expect("a scene serializes")
}

/// The id allocator as saved.
fn next_id(s: &Session) -> u32 {
    crate::scene::to_scene_data(&s.world().world.scene.borrow()).next_entity_id
}

#[test]
fn api_edits_undo_and_redo_to_identical_documents() {
    let mut s = session();
    let start = document(&s);
    let id = run(&mut s, "Scene.CreateEntity('Crate', 'Box')");
    let steps = [
        format!("Scene.SetName({id}, 'Barrel')"),
        format!("Transform.SetPosition({id}, 1, 2, 3)"),
        format!("Scene.AddComponent({id}, 'Light')"),
        "Lighting.SetAmbientIntensity(0.1)".to_string(),
    ];
    for line in &steps {
        run(&mut s, line);
    }
    let (end, end_id) = (document(&s), next_id(&s));
    assert_eq!(run(&mut s, "Editor.UndoName()"), steps[3]);
    for _ in 0..=steps.len() {
        run(&mut s, "Editor.Undo()");
    }
    assert_eq!(
        document(&s),
        start,
        "every step undone: the scene it started as"
    );
    assert_eq!(next_id(&s), end_id, "the undone create's id stays retired");
    assert_eq!(run(&mut s, "Editor.Undo()"), "nil", "nothing left to undo");
    for _ in 0..=steps.len() {
        run(&mut s, "Editor.Redo()");
    }
    assert_eq!(
        document(&s),
        end,
        "every step redone: the scene it ended as"
    );
    assert_eq!(run(&mut s, "Editor.RedoName()"), "nil");
}

#[test]
fn destroy_and_reparent_come_back_under_the_same_ids() {
    let mut s = session();
    let parent = run(&mut s, "Scene.CreateEntity('Parent', 'Box')");
    let child = run(&mut s, "Scene.CreateEntity('Child', 'Sphere')");
    let before = document(&s);
    run(&mut s, &format!("Scene.SetParent({child}, {parent})"));
    run(&mut s, &format!("Scene.DestroyEntity({parent})"));
    assert_eq!(run(&mut s, &format!("Scene.GetName({child})")), "nil");
    assert_eq!(
        run(&mut s, "Editor.Undo()"),
        format!("Scene.DestroyEntity({parent})")
    );
    assert_eq!(run(&mut s, &format!("Scene.GetParent({child})")), parent);
    run(&mut s, "Editor.Undo()");
    assert_eq!(
        document(&s),
        before,
        "ids, order and the hierarchy as they were"
    );
    run(&mut s, "Editor.Redo()");
    run(&mut s, "Editor.Redo()");
    assert_eq!(run(&mut s, &format!("Scene.GetName({parent})")), "nil");
}

#[test]
fn a_new_edit_clears_redo_and_play_records_nothing() {
    let mut s = session();
    let id = run(&mut s, "Scene.CreateEntity('A')");
    run(&mut s, "Editor.Undo()");
    assert_eq!(run(&mut s, "Editor.RedoName()"), "Scene.CreateEntity('A')");
    run(&mut s, "Scene.CreateEntity('B')");
    assert_eq!(
        run(&mut s, "Editor.RedoName()"),
        "nil",
        "a new edit clears Redo"
    );
    let b = run(&mut s, "Scene.FindEntityByName('B')");
    assert_ne!(id, b, "the undone id is never handed out again");

    run(&mut s, "Editor.EnterPlay()");
    assert_eq!(
        run(&mut s, "Editor.UndoName()"),
        "nil",
        "Play clears the history"
    );
    run(&mut s, &format!("Scene.SetName({b}, 'Played')"));
    assert_eq!(
        run(&mut s, "Editor.Undo()"),
        "nil",
        "nothing is undone in Play"
    );
    run(&mut s, "Editor.ExitPlay()");
    assert_eq!(
        run(&mut s, "Editor.UndoName()"),
        "nil",
        "nothing was recorded in Play"
    );
}
