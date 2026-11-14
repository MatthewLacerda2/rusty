//! `Debug.Snapshot` reports the scene's one play state (#180, #1302): flipping it
//! must show up in the snapshot.

use crate::dev::session::Session;

#[test]
fn the_scene_play_state_drives_debug_snapshot_play_state() {
    let s = Session::new("").expect("an empty session boots");
    assert!(
        s.eval("return Debug.Snapshot()")
            .unwrap()
            .contains("\"play_state\": \"editor\""),
        "edit mode by default"
    );
    s.world().scene().borrow_mut().set_playing(true);
    assert!(
        s.eval("return Debug.Snapshot()")
            .unwrap()
            .contains("\"play_state\": \"playing\""),
        "the snapshot reads the scene's play state"
    );
}
