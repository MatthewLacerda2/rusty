//! `Editor.*` in the headless harness (#1360): a standalone run has no edit mode, so
//! `Editor.ExitPlay()` is refused and the run keeps playing.

use rusty::dev::harness::Harness;

#[test]
fn the_harness_refuses_exit_play() {
    let out = crate::temp::dir().join(format!("rusty_editor_play_{}", std::process::id()));
    let h = Harness::new(&out, "");
    h.step(1);
    let exit = h.world.borrow().script_manager().eval("Editor.ExitPlay()");
    assert_eq!(exit.as_deref(), Ok("false"), "nothing to return to");
    h.step(2);
    assert_eq!(h.frame(), 3, "the run kept playing");
    let _ = std::fs::remove_dir_all(&out);
}
