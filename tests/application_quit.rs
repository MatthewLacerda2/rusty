//! `Application.Quit()` in the headless harness ends the run (#431): once the game
//! asks to quit, further steps are no-ops and `results.json` records it.

use rusty::dev::harness::Harness;

#[test]
fn quit_ends_a_headless_run() {
    let out = crate::temp::dir().join(format!("rusty_quit_{}", std::process::id()));
    let h = Harness::new(&out, "");
    h.step(5);
    assert_eq!(h.frame(), 5);

    h.world
        .borrow()
        .script_manager()
        .eval("Application.Quit()")
        .unwrap();
    h.step(100);
    assert_eq!(h.frame(), 5, "no tick runs after Quit");

    let results = std::fs::read_to_string(h.write_results().unwrap()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&results).unwrap();
    assert_eq!(json["quit"], true);
    assert_eq!(json["frames"], 5);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_run_that_never_quits_reports_quit_false() {
    let out = crate::temp::dir().join(format!("rusty_noquit_{}", std::process::id()));
    let h = Harness::new(&out, "");
    h.step(3);
    let results = std::fs::read_to_string(h.write_results().unwrap()).unwrap();
    let json: serde_json::Value = serde_json::from_str(&results).unwrap();
    assert_eq!(json["quit"], false);
    let _ = std::fs::remove_dir_all(&out);
}
