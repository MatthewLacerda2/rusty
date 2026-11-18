//! A bake's scratch folder (#2312): gone when its pages are, on every exit; a
//! killed bake's removed by the next bake of the scene, a live one's never.

use std::path::{Path, PathBuf};

use super::spill::Spill;
use super::tests::{quick, scene, scene_path};
use super::*;
use rusty_asset::lightmap_page::DIR_SUFFIX;

/// The spill folders beside `scene_path`.
fn spills(scene_path: &str) -> Vec<PathBuf> {
    let path = Path::new(scene_path);
    let prefix = format!("{}.spill-", path.file_name().unwrap().to_string_lossy());
    let entries = std::fs::read_dir(path.parent().unwrap()).unwrap();
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy();
            name.starts_with(&prefix) && name.ends_with(DIR_SUFFIX)
        })
        .collect();
    found.sort();
    found
}

#[test]
fn a_spill_holds_its_files_until_dropped() {
    let path = scene_path("spill_dropped");
    let mut spill = Spill::open(&path).unwrap();
    let file = spill.put(b"page").unwrap();
    assert_eq!(file.read().unwrap(), b"page");
    let linked = Path::new(&path).with_extension("linked");
    file.link(&linked).unwrap();
    assert!(spill.dir().is_dir());
    drop(spill);
    assert!(spills(&path).is_empty(), "removed with its pages");
    assert_eq!(
        std::fs::read(&linked).unwrap(),
        b"page",
        "a written page stays"
    );
}

#[test]
fn a_killed_bake_s_spill_is_removed_by_the_next_and_a_live_one_kept() {
    let path = scene_path("spill_orphans");
    // Past any pid a machine hands out: that process is gone.
    let dead = format!("{path}.spill-4000000000-0{DIR_SUFFIX}");
    // Pid 1 always runs.
    let live = format!("{path}.spill-1-0{DIR_SUFFIX}");
    for dir in [&dead, &live] {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(Path::new(dir).join("0"), b"stale").unwrap();
    }
    let spill = Spill::open(&path).unwrap();
    assert!(
        !Path::new(&dead).exists(),
        "a killed bake's spill is removed"
    );
    assert!(Path::new(&live).exists(), "another running bake's is kept");
    assert_eq!(spills(&path).len(), 2, "the live one and this one");
    drop(spill);
    std::fs::remove_dir_all(&live).unwrap();
}

#[test]
fn a_bake_cancelled_during_its_pages_or_written_leaves_no_spill() {
    let path = scene_path("spill_exits");
    let input = gather_bake_input(&scene().0);
    let maps = rusty_lighting::lightmap::bake(&input, &quick());
    let cancelled = BakeProgress::default();
    cancelled.cancel();
    assert_eq!(
        encode_pages(maps.clone(), 1024, &cancelled, &path),
        Ok(None)
    );
    assert!(spills(&path).is_empty(), "a cancel removes it");
    let (mut scene, ..) = scene();
    assert!(bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap() > 0);
    assert!(spills(&path).is_empty(), "a written bake removes it");
    assert!(scene.lightmaps.pages.iter().all(|p| Path::new(p).is_file()));
}
