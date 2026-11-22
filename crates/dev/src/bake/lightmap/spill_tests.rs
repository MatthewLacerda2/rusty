use std::path::{Path, PathBuf};

use super::spill::{Spill, SPILL_DIR};
use super::tests::{quick, scene};
use super::*;

fn project(dir: &str) -> (PathBuf, impl Fn(&str) -> String) {
    let root = rusty_test_support::temp::dir().join(format!("rusty_spill_{dir}"));
    let _ = std::fs::remove_dir_all(&root);
    let scenes = root.join(rusty_core::project::ASSETS_DIR).join("scenes");
    std::fs::create_dir_all(&scenes).unwrap();
    std::fs::write(root.join(rusty_core::project::PROJECT_FILE), "{}").unwrap();
    let scene = move |name: &str| scenes.join(name).to_string_lossy().into_owned();
    (root, scene)
}

fn spills(root: &Path) -> Vec<String> {
    let dir = root.join(rusty_core::project::CACHE_DIR).join(SPILL_DIR);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    found
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        match path.is_dir() {
            true => found.extend(files_under(&path)),
            false => found.push(path),
        }
    }
    found
}

#[test]
fn a_spill_lies_in_the_project_s_cache_and_holds_its_files_until_dropped() {
    let (root, scene_path) = project("dropped");
    let path = scene_path("level.scene");
    let mut spill = Spill::open(&path).unwrap();
    let file = spill.put(b"page").unwrap();
    assert_eq!(file.read().unwrap(), b"page");
    let cache = std::fs::canonicalize(&root).unwrap().join("cache");
    assert!(spill.dir().starts_with(cache), "{:?}", spill.dir());
    let linked = Path::new(&path).with_extension("linked");
    file.link(&linked).unwrap();
    drop(spill);
    assert!(
        spills(&root).is_empty(),
        "removed with its pages, lock and all"
    );
    assert_eq!(
        std::fs::read(&linked).unwrap(),
        b"page",
        "a written page stays"
    );
}

#[test]
fn a_killed_bake_leaves_nothing_in_assets_and_any_scene_s_bake_clears_it() {
    let (root, scene_path) = project("killed");
    let (killed, other) = (scene_path("cafe.scene"), scene_path("level.scene"));
    let live = Spill::open(&killed).unwrap();
    let mut dead = Spill::open(&killed).unwrap();
    dead.put(b"a page").unwrap();
    dead.abandon();
    let assets = root.join(rusty_core::project::ASSETS_DIR);
    assert_eq!(
        files_under(&assets),
        Vec::<PathBuf>::new(),
        "nothing in assets/"
    );
    assert_eq!(spills(&root).len(), 4, "two folders, two locks");
    let (mut scene, ..) = scene();
    assert!(bake_scene_lightmaps(&mut scene, Some(&other), &quick()).unwrap() > 0);
    let left = spills(&root);
    let live_name = live
        .dir()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        left,
        [live_name.clone(), format!("{live_name}.lock")],
        "{left:?}"
    );
    drop(live);
    assert!(spills(&root).is_empty());
}

#[test]
fn a_bake_cancelled_during_its_pages_or_written_leaves_no_spill() {
    let (root, scene_path) = project("exits");
    let path = scene_path("level.scene");
    let input = gather_bake_input(&scene().0);
    let maps = rusty_lighting::lightmap::bake(&input, &quick());
    let cancelled = BakeProgress::default();
    cancelled.cancel();
    assert_eq!(
        encode_pages(maps.clone(), 1024, &cancelled, &path),
        Ok(None)
    );
    assert!(spills(&root).is_empty(), "a cancel removes it");
    let (mut scene, ..) = scene();
    assert!(bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap() > 0);
    assert!(spills(&root).is_empty(), "a written bake removes it");
    assert!(scene.lightmaps.pages.iter().all(|p| Path::new(p).is_file()));
}

#[test]
fn a_scene_in_no_project_spills_in_a_folder_of_spills_not_beside_it() {
    let dir = rusty_test_support::temp::dir().join("rusty_spill_loose");
    std::fs::create_dir_all(&dir).unwrap();
    let spill = Spill::open(&dir.join("loose.scene").to_string_lossy()).unwrap();
    let parent = spill.dir().parent().unwrap();
    assert!(parent.ends_with(SPILL_DIR), "{:?}", spill.dir());
    assert!(!spill.dir().starts_with(&dir), "{:?}", spill.dir());
}
