//! The seed manifest's four outcomes (#746): a file is overwritten only with proof
//! that nobody edited it.

use super::{SeedManifest, SeedOutcome};
use std::path::PathBuf;

/// A fresh workspace per test: its manifest path and one seeded file's path.
fn workspace(name: &str) -> (PathBuf, PathBuf) {
    let root = crate::test_temp::dir().join("seed_manifest").join(name);
    std::fs::remove_dir_all(&root).ok();
    std::fs::create_dir_all(&root).unwrap();
    (root.join(".seeded"), root.join("scripts").join("bot.lua"))
}

fn seed(manifest: &PathBuf, dest: &PathBuf, bundled: &str) -> SeedOutcome {
    let mut m = SeedManifest::load(manifest);
    let outcome = m.seed(dest, bundled.as_bytes(), "delete it");
    m.save().unwrap();
    outcome
}

fn read(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn a_missing_file_is_written_and_recorded() {
    let (manifest, dest) = workspace("missing");
    assert_eq!(seed(&manifest, &dest, "v1"), SeedOutcome::Written);
    assert_eq!(read(&dest), "v1");
    assert!(read(&manifest).contains("bot.lua"));
    assert_eq!(seed(&manifest, &dest, "v1"), SeedOutcome::UpToDate);
}

#[test]
fn an_unedited_file_is_refreshed_when_the_default_changes() {
    let (manifest, dest) = workspace("refresh");
    seed(&manifest, &dest, "v1");
    assert_eq!(seed(&manifest, &dest, "v2"), SeedOutcome::Refreshed);
    assert_eq!(read(&dest), "v2");
    // The new version is what's recorded, so the next change refreshes too.
    assert_eq!(seed(&manifest, &dest, "v3"), SeedOutcome::Refreshed);
    assert_eq!(read(&dest), "v3");
}

#[test]
fn an_edited_file_is_kept() {
    let (manifest, dest) = workspace("edited");
    seed(&manifest, &dest, "v1");
    std::fs::write(&dest, "mine").unwrap();
    assert_eq!(seed(&manifest, &dest, "v2"), SeedOutcome::Kept);
    assert_eq!(read(&dest), "mine");
    // Still the user's on the next boot: the record is not moved to their edit.
    assert_eq!(seed(&manifest, &dest, "v3"), SeedOutcome::Kept);
}

#[test]
fn a_file_seeded_before_the_manifest_is_kept_unless_it_matches() {
    let (manifest, dest) = workspace("pre_manifest");
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::write(&dest, "old default").unwrap();
    assert_eq!(seed(&manifest, &dest, "v2"), SeedOutcome::Kept);
    assert_eq!(read(&dest), "old default");
    // Deleting it takes the new default, and from then on it is tracked.
    std::fs::remove_file(&dest).unwrap();
    assert_eq!(seed(&manifest, &dest, "v2"), SeedOutcome::Written);
    assert_eq!(seed(&manifest, &dest, "v3"), SeedOutcome::Refreshed);
}

#[test]
fn a_matching_untracked_file_is_adopted() {
    let (manifest, dest) = workspace("adopt");
    std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
    std::fs::write(&dest, "v1").unwrap();
    assert_eq!(seed(&manifest, &dest, "v1"), SeedOutcome::UpToDate);
    assert_eq!(seed(&manifest, &dest, "v2"), SeedOutcome::Refreshed);
}

#[test]
fn saving_merges_over_entries_another_run_wrote() {
    let (manifest, dest) = workspace("merge");
    let other = dest.with_file_name("player.lua");
    // Two runs load the same (empty) manifest, each seeds a different file.
    let mut a = SeedManifest::load(&manifest);
    let mut b = SeedManifest::load(&manifest);
    a.seed(&dest, b"a", "");
    b.seed(&other, b"b", "");
    a.save().unwrap();
    b.save().unwrap();
    let text = read(&manifest);
    assert!(
        text.contains("bot.lua") && text.contains("player.lua"),
        "{text}"
    );
}
