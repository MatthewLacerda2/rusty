//! The conversion gate is per thread in this binary too (#1337). The library is
//! linked here without `cfg(test)`, and `cargo test` (cargo-mutants' runner) runs
//! every test below in one process: a world playing in one test must not refuse
//! another test's import.

use rusty::asset::convert::{conversion_allowed, PlayHold};
use std::sync::mpsc;

#[test]
fn a_world_playing_on_another_thread_leaves_this_threads_conversion_on() {
    let (taken, held) = mpsc::channel();
    let (release, released) = mpsc::channel::<()>();
    let player = std::thread::spawn(move || {
        let hold = PlayHold::take();
        taken.send(conversion_allowed()).unwrap();
        released.recv().unwrap();
        drop(hold);
    });
    assert!(
        !held.recv().unwrap(),
        "the playing thread refuses conversion"
    );
    assert!(conversion_allowed(), "this thread is not playing");
    release.send(()).unwrap();
    player.join().unwrap();
}

#[test]
fn this_threads_own_hold_refuses_its_conversion() {
    let hold = PlayHold::take();
    assert!(!conversion_allowed());
    drop(hold);
    assert!(conversion_allowed());
}
