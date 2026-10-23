//! Running Blender with fakes (#1072): a failed version is asked once, and a run
//! past its limit is stopped.

use super::super::run::version_within;
use super::locate::{fake, script, search};
use std::time::Duration;

#[test]
fn a_failed_version_is_asked_once() {
    let dir = crate::test_temp::dir();
    let count = dir.join("asked");
    script(
        &dir,
        &format!("echo x >> '{}'\necho hello", count.display()),
    );
    let search = search(None, Some(&dir));
    for _ in 0..3 {
        let why = search.find().expect_err("no version").to_string();
        assert!(why.contains("reported no Blender version"), "{why}");
    }
    let asked = std::fs::read_to_string(&count).unwrap();
    assert_eq!(asked.lines().count(), 1, "started once");
}

#[test]
fn a_program_that_does_not_answer_is_stopped_at_its_limit() {
    let dir = crate::test_temp::dir();
    // `exec`: the shell becomes the sleep, so stopping it ends the pipes too.
    let hung = script(&dir, "exec sleep 30");
    let why = version_within(&hung, Duration::from_millis(100)).expect_err("stopped");
    let why = why.to_string();
    assert!(why.contains("--version did not finish within"), "{why}");
}

#[test]
fn a_version_within_its_limit_is_read() {
    let dir = crate::test_temp::dir();
    let blender = fake(&dir, "Blender 5.0.1");
    let version = version_within(&blender, Duration::from_secs(10)).expect("answered");
    assert_eq!(version.expect("a version"), "5.0.1");
}

#[test]
fn the_export_limit_is_ten_minutes_unless_set_in_seconds() {
    let mut search = search(None, None);
    assert_eq!(search.export_timeout().unwrap(), Duration::from_secs(600));
    search.timeout = Some("30".into());
    assert_eq!(search.export_timeout().unwrap(), Duration::from_secs(30));
    for bad in ["0", "ten", "-5"] {
        search.timeout = Some(bad.into());
        let why = search.export_timeout().expect_err(bad).to_string();
        assert!(why.contains("RUSTY_BLENDER_TIMEOUT"), "{why}");
    }
}

#[test]
fn a_program_that_could_not_start_is_asked_again() {
    use std::os::unix::fs::PermissionsExt;
    let dir = crate::test_temp::dir();
    let blender = fake(&dir, "Blender 5.0.1");
    let mode = |m| std::fs::set_permissions(&blender, std::fs::Permissions::from_mode(m));
    mode(0o644).unwrap();
    let search = search(None, Some(&dir));
    let why = search.find().expect_err("not executable").to_string();
    assert!(why.contains("starting"), "{why}");
    // The same length and mtime: only an answer not remembered is asked again.
    mode(0o755).unwrap();
    assert_eq!(search.find().expect("asked again").version, "5.0.1");
}
