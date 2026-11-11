use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = crate::test_temp::dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[test]
fn clear_refuses_a_folder_it_did_not_make() {
    let dir = scratch("foreign");
    write(&dir.join("keep.txt"), "mine");
    assert!(clear(&dir).is_err());
    assert!(dir.join("keep.txt").is_file(), "nothing was deleted");
}

#[test]
fn clear_empties_a_generated_folder_and_makes_a_missing_one() {
    let dir = scratch("generated");
    write(&dir.join(MARKER), "smoke\n");
    write(&dir.join("assets").join("old.scene"), "stale");
    clear(&dir).unwrap();
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    let missing = dir.join("new");
    clear(&missing).unwrap();
    assert!(missing.is_dir());
}

#[test]
fn copy_dir_keeps_the_relative_paths() {
    let dir = scratch("copy");
    write(&dir.join("from").join("a.lua"), "a");
    write(&dir.join("from").join("sub").join("b.lua"), "b");
    copy_dir(&dir.join("from"), &dir.join("to")).unwrap();
    copy_dir(&dir.join("absent"), &dir.join("to")).unwrap();
    assert!(differences(&dir.join("from"), &dir.join("to"))
        .unwrap()
        .is_empty());
}

#[test]
fn differences_names_changed_missing_and_extra_files() {
    let dir = scratch("diff");
    for (file, text) in [("same", "1"), ("changed", "2"), ("only_a", "3")] {
        write(&dir.join("a").join(file), text);
    }
    for (file, text) in [("same", "1"), ("changed", "X"), ("only_b", "4")] {
        write(&dir.join("b").join(file), text);
    }
    let differ = differences(&dir.join("a"), &dir.join("b")).unwrap();
    let names: Vec<_> = differ
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["changed", "only_a", "only_b"]);
}
