//! Play's refresh (#1253): every converted source under the root is brought up to
//! date before Play holds conversion off, at the cost of one `stat` per unchanged
//! source, walking the tree once and never round a symlink loop.

use super::super::{cache_location, depends, gate, refresh_sources, PlayHold};
use super::gate::{indices, stamp, QUAD};
use std::path::Path;

#[test]
fn refreshing_converts_every_source_under_the_root_and_skips_a_broken_one() {
    let root = crate::test_temp::dir().join("assets");
    std::fs::create_dir_all(root.join("enemies")).unwrap();
    let enemy = root.join("enemies").join("enemy.fbx");
    std::fs::write(&enemy, QUAD).unwrap();
    std::fs::write(root.join("broken.fbx"), "not an fbx").unwrap();
    std::fs::write(root.join("notes.txt"), "not a model").unwrap();

    let hold = PlayHold::take();
    assert_eq!(refresh_sources(&root), 0, "nothing converts while held");
    drop(hold);
    assert!(!cache_location(&enemy).unwrap().is_file());

    assert_eq!(refresh_sources(&root), 2, "both sources looked at");
    let cache = cache_location(&enemy).unwrap();
    let before = stamp(&cache);
    let hold = PlayHold::take();
    assert_eq!(indices(&enemy), 6, "served from the refresh in Play");
    drop(hold);
    refresh_sources(&root);
    assert_eq!(
        stamp(&cache),
        before,
        "a current source is not converted again"
    );
}

#[test]
fn a_second_refresh_reads_no_unchanged_source_again() {
    let root = crate::test_temp::dir().join("assets");
    std::fs::create_dir_all(&root).unwrap();
    let enemy = root.join("enemy.fbx");
    std::fs::write(&enemy, QUAD).unwrap();
    refresh_sources(&root);
    let reads = depends::reads(&enemy);
    refresh_sources(&root);
    refresh_sources(&root);
    assert_eq!(depends::reads(&enemy), reads, "length and mtime unchanged");
    assert!(reads >= 1);
}

#[cfg(unix)]
#[test]
fn a_symlinked_folder_is_not_followed() {
    let root = crate::test_temp::dir().join("assets");
    std::fs::create_dir_all(root.join("a")).unwrap();
    std::fs::write(root.join("a").join("enemy.fbx"), QUAD).unwrap();
    std::os::unix::fs::symlink(&root, root.join("a").join("loop")).unwrap();
    assert_eq!(refresh_sources(&root), 1, "the loop is not walked");
}

#[test]
fn a_leading_dot_names_the_same_held_source() {
    let plain = gate::key(Path::new("assets/x.fbx"));
    assert_eq!(gate::key(Path::new("./assets/x.fbx")), plain);
    assert_eq!(gate::key(Path::new("assets/./x.fbx")), plain);
}
