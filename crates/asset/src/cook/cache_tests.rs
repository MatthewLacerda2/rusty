//! The cooked file: a current one is read, bit for bit the fresh cook, and
//! every change of what it was made from cooks again.

use super::fixture;
use crate::import_file;

/// Import `path` with cooked files written, returning it and how many cooks it ran.
fn cooked(path: &std::path::Path) -> (crate::ImportedAsset, usize) {
    let before = super::cooks();
    let asset = super::with_writes(true, || import_file(path).unwrap());
    (asset, super::cooks() - before)
}

#[test]
fn a_cooked_read_is_a_fresh_cook_bit_for_bit_and_cooks_nothing() {
    let path = fixture::model("hit");
    let (fresh, cooks) = cooked(&path);
    assert_eq!(cooks, 1, "nothing cooked yet");
    assert!(super::fixture::cooked_file(&path).is_file());
    let (read, cooks) = cooked(&path);
    assert_eq!(cooks, 0, "a current cooked file is read, not cooked again");
    assert_eq!(read, fresh);
    assert_eq!(format!("{read:?}"), format!("{fresh:?}"));
    // The lightmap UVs a bake fitted are the ones a cooked load gives.
    let uvs = |a: &crate::ImportedAsset| -> Vec<[f32; 2]> {
        a.sub_meshes
            .iter()
            .flat_map(|s| s.vertices.iter().map(|v| v.lightmap_uv))
            .collect()
    };
    assert_eq!(uvs(&read), uvs(&fresh));
}

#[test]
fn a_player_reads_a_cooked_file_and_never_writes_one() {
    let path = fixture::model("player");
    let file = super::fixture::cooked_file(&path);
    super::with_writes(false, || import_file(&path).unwrap());
    assert!(!file.exists(), "a shipped player writes nothing");
    let (authored, _) = cooked(&path);
    let before = super::cooks();
    let played = super::with_writes(false, || import_file(&path).unwrap());
    assert_eq!(
        super::cooks() - before,
        0,
        "it reads what an authoring build cooked"
    );
    assert_eq!(played, authored);
}

#[test]
fn each_geometry_setting_and_a_source_edit_cook_again() {
    let path = fixture::model("settings");
    let (first, _) = cooked(&path);
    crate::sidecar::set_scale(&path, 2.0).unwrap();
    let (scaled, cooks) = cooked(&path);
    assert_eq!(cooks, 1, "a new Scale Factor cooks again");
    assert_ne!(scaled, first);
    assert_eq!(cooked(&path).1, 0);
    let mut lm = crate::sidecar::load(&path).unwrap().lightmap_uvs;
    lm.pack_margin = 9.0;
    crate::sidecar::set_lightmap_uvs(&path, lm).unwrap();
    assert_eq!(cooked(&path).1, 1, "a new pack margin cooks again");
    lm.generate = false;
    crate::sidecar::set_lightmap_uvs(&path, lm).unwrap();
    let (off, cooks) = cooked(&path);
    assert_eq!(cooks, 1, "unticking Generate Lightmap UVs cooks again");
    assert!(off.sub_meshes[0]
        .vertices
        .iter()
        .all(|v| v.lightmap_uv == [0.0; 2]));
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replacen("v 0 0 0", "v 0 0.5 0", 1)).unwrap();
    let (edited, cooks) = cooked(&path);
    assert_eq!(cooks, 1, "an edited source cooks again");
    let fresh = super::with_writes(false, || import_file(&path).unwrap());
    assert_eq!(edited, fresh);
}

#[test]
fn a_malformed_cooked_file_is_cooked_again() {
    let path = fixture::model("malformed");
    let (fresh, _) = cooked(&path);
    let file = super::fixture::cooked_file(&path);
    let bytes = std::fs::read(&file).unwrap();
    std::fs::write(&file, &bytes[..bytes.len() / 2]).unwrap();
    let (again, cooks) = cooked(&path);
    assert_eq!(cooks, 1);
    assert_eq!(again, fresh);
    assert_eq!(std::fs::read(&file).unwrap(), bytes, "rewritten whole");
}

#[test]
fn a_cooked_file_of_another_model_is_not_laid_on_this_one() {
    let path = fixture::model("swapped");
    let (fresh, _) = cooked(&path);
    let file = super::fixture::cooked_file(&path);
    // Another model's values under this one's stamp: its sub-mesh ids disagree.
    let read = crate::read_target(&path).unwrap();
    let settings = crate::settings_of(&path);
    let stamp = super::stamp::stamp(&read, &settings).unwrap();
    let other = super::sub::Cooked::of(&fresh.sub_meshes[2], 0, None);
    std::fs::write(&file, super::file::encode(stamp, &[other])).unwrap();
    let (again, cooks) = cooked(&path);
    assert_eq!(cooks, 1);
    assert_eq!(again, fresh);
}

#[test]
fn a_re_converted_source_cooks_again() {
    let dir = rusty_test_support::temp::dir().join("rusty_cook_reconverted");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.fbx");
    let fixture = |name: &str| {
        rusty_core::project::checkout()
            .join("tests/fixtures/models")
            .join(name)
    };
    std::fs::copy(fixture("skinned_cube.fbx"), &path).unwrap();
    let _ = std::fs::remove_file(super::fixture::cooked_file(&path));
    let (first, cooks) = cooked(&path);
    assert_eq!(cooks, 1);
    assert_eq!(cooked(&path).1, 0);
    std::fs::copy(fixture("physical_material.fbx"), &path).unwrap();
    let (second, cooks) = cooked(&path);
    assert_eq!(
        cooks, 1,
        "the new conversion is cooked, not the old one's file read"
    );
    assert_ne!(first, second);
    assert_eq!(cooked(&path), (second, 0));
}

#[test]
fn a_standalone_game_writes_no_cooked_file() {
    let path = fixture::model("standalone");
    crate::convert::mark_standalone();
    let (_, cooks) = cooked(&path);
    assert_eq!(cooks, 1, "it cooks in memory");
    assert!(
        !super::fixture::cooked_file(&path).exists(),
        "and writes nothing"
    );
}
