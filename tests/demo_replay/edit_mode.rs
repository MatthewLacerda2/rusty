//! What a host does before Play: edit-mode lines replay on an edit runtime, the
//! Play verb is no act, and a paused edit frame's acts are dropped with it.

use rusty::dev::demo::{self, Demo, Destination};
use rusty::dev::harness::Harness;

use super::replay;

#[test]
fn edit_mode_lines_replay_and_the_play_verb_is_no_act() {
    let dir = crate::temp::dir().join("demo_edit_mode");
    let path = dir.join("run.demo");
    let h = Harness::new(dir.join("record"), "");
    {
        let mut world = h.world.borrow_mut();
        demo::record(&mut world, Destination::File(path.clone()));
        world.set_playing(false); // an editor in edit mode, paused
        world.init_edit_runtime().unwrap();
        world.script_manager().eval("dropped = 1").unwrap();
        world.poll_transition();
        let moved = "Transform.SetPosition(Scene.FindEntityByName('Player'), 4, 0, 4)";
        world.script_manager().eval(moved).unwrap();
        world.script_manager().eval("Editor.EnterPlay()").unwrap();
    }
    h.step(130);
    drop(h);

    let demo = Demo::read(&path).unwrap();
    let before: Vec<&str> = demo
        .header
        .before_play
        .iter()
        .filter_map(|act| act.get("eval")?.as_str())
        .collect();
    assert!(
        matches!(before[..], [line] if line.contains("SetPosition")),
        "only the line just before Play, not the verb: {before:?}"
    );
    // The line moved the player: had it failed (no runtime), the state differs.
    let verdict = replay(&dir, demo, |_| {});
    assert_eq!((verdict.diverged, verdict.checked), (None, 2));
}
