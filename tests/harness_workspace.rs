//! The harness runs a workspace of its own (#782): a stale or edited copy of a
//! bundled script — what a developer's git-ignored `./project` keeps on purpose —
//! never reaches a test.

use rusty::dev::harness::Harness;
use rusty::scene::default_scene::PLAYER_CONTROLLER_SCRIPT;

/// A wrong controller: it announces itself and leaves the Player inert.
const STALE: &str = r#"print("[stale] the workspace copy ran"); return {}"#;

#[test]
fn a_stale_player_controller_in_the_workspace_is_replaced_by_the_bundled_one() {
    let dir = crate::temp::dir().join("rusty_stale_controller");
    let planted = Harness::workspace_of(&dir).join(PLAYER_CONTROLLER_SCRIPT);
    std::fs::create_dir_all(planted.parent().unwrap()).unwrap();
    std::fs::write(&planted, STALE).unwrap();

    let h = Harness::new(&dir, "");
    let resolved = h
        .world
        .borrow()
        .script_manager()
        .resolve_script(PLAYER_CONTROLLER_SCRIPT);
    assert_eq!(
        resolved, planted,
        "the scene's controller resolves in the run's workspace"
    );
    h.step(60);
    let console = h.console.borrow();
    let ran = |needle: &str| console.messages.iter().any(|m| m.0.contains(needle));
    assert!(
        !ran("[stale]"),
        "the stale copy ran instead of the bundled one"
    );
    assert!(!ran("Error"), "a script failed: {:?}", console.messages);
    let bundled = std::fs::read("engine/scripts/player_controller.lua").unwrap();
    assert_eq!(
        std::fs::read(&planted).unwrap(),
        bundled,
        "the workspace holds the bundled source"
    );
}
