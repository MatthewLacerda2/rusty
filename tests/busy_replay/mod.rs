//! Replay of a busy scene (#764): the same inputs give the same world, run twice in
//! one process and once in another. Std's `RandomState` keys every hash map from
//! per-process entropy, so a sim map whose iteration order reaches behaviour shows
//! up here as a mismatch — the empty-scene replay in `harness_determinism` can't.

mod scene;

const FRAMES: u32 = 240;
/// Set on the child process: where it writes its run.
const OUT_VAR: &str = "RUSTY_BUSY_REPLAY_OUT";

#[test]
fn busy_scene_replays_in_one_process() {
    let a = scene::run("rusty_busy_a", FRAMES);
    assert!(a.contains("[busy] hit"), "boxes never collided:\n{a}");
    assert!(a.contains("[busy] t "), "the timer never fired:\n{a}");
    assert!(!a.contains("Error"), "a script failed:\n{a}");
    assert_eq!(
        a,
        scene::run("rusty_busy_b", FRAMES),
        "same process, two runs"
    );
}

#[test]
fn busy_scene_replays_across_processes() {
    let here = scene::run("rusty_busy_parent", FRAMES);
    let out = crate::temp::dir().join("rusty_busy_child.txt");
    let status = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args(["--exact", "busy_replay::child_run", "--include-ignored"])
        .env(OUT_VAR, &out)
        .status()
        .expect("spawn the child process");
    assert!(status.success(), "child run failed");
    let there = std::fs::read_to_string(&out).expect("child output");
    assert_eq!(here, there, "a second process diverged from this one");
}

/// The child half of the cross-process test; a no-op unless that test spawned it.
#[test]
#[ignore = "run by busy_scene_replays_across_processes in a child process"]
fn child_run() {
    if let Some(out) = std::env::var_os(OUT_VAR) {
        let run = scene::run("rusty_busy_child", FRAMES);
        std::fs::write(out, run).expect("write child run");
    }
}
