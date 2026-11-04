//! Borrowed clips through the importer (#1747): the sidecar names the source,
//! the import carries the retargeted clip, and the model's own clips are the
//! same bytes as without it.

use std::path::{Path, PathBuf};

use super::HumanoidSettings;
use crate::asset::fixtures_anim::{animated_buffer, ANIMATED_GLTF};
use crate::asset::{import_file, imported, sidecar, ImportedAsset};

/// `name`, a copy of the animated fixture whose one clip is `clip`, in `dir`,
/// its sidecar mapping `Joint0` to the hips with `clips_from`.
pub(crate) fn model(dir: &Path, name: &str, clip: &str, clips_from: &[&Path]) -> PathBuf {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("anim.bin"), animated_buffer()).unwrap();
    let path = dir.join(name);
    let text = ANIMATED_GLTF.replace("\"name\": \"Wave\"", &format!("\"name\": \"{clip}\""));
    std::fs::write(&path, text).unwrap();
    let mut humanoid = HumanoidSettings::default();
    humanoid.bones.insert("Hips".into(), "Joint0".into());
    humanoid.bones.insert("Spine".into(), "Joint1".into());
    humanoid.clips_from = clips_from
        .iter()
        .map(|p| p.to_string_lossy().into())
        .collect();
    let settings = sidecar::ImportSettings {
        humanoid,
        ..Default::default()
    };
    sidecar::save(&path, &settings).unwrap();
    path
}

fn clips(asset: &ImportedAsset) -> Vec<String> {
    asset.sub_meshes[0]
        .clips
        .iter()
        .map(|c| c.name.clone())
        .collect()
}

#[test]
fn a_model_borrows_a_clip_and_keeps_its_own_bytes() {
    let dir = crate::test_temp::dir().join("rusty_humanoid_borrow");
    let a = model(&dir, "a.gltf", "Wave", &[]);
    let alone = import_file(&model(&dir, "alone.gltf", "Idle", &[])).unwrap();
    let b = import_file(&model(&dir, "b.gltf", "Idle", &[&a])).unwrap();
    assert_eq!(clips(&b), ["Idle", "Wave"]);
    let own = &b.sub_meshes[0].clips[0];
    assert_eq!(
        format!("{own:?}"),
        format!("{:?}", alone.sub_meshes[0].clips[0])
    );
    let wave = &b.sub_meshes[0].clips[1];
    let source = &import_file(&a).unwrap().sub_meshes[0].clips[0];
    assert_eq!(wave.tracks[0].translation, source.tracks[0].translation);
}

#[test]
fn a_clip_of_a_name_the_model_has_and_a_missing_source_are_left_out() {
    let dir = crate::test_temp::dir().join("rusty_humanoid_clash");
    let a = model(&dir, "a.gltf", "Wave", &[]);
    let gone = dir.join("gone.gltf");
    let b = import_file(&model(&dir, "b.gltf", "Wave", &[&gone, &a])).unwrap();
    assert_eq!(clips(&b), ["Wave"]);
}

#[test]
fn two_models_borrowing_from_each_other_take_only_the_others_own_clips() {
    let dir = crate::test_temp::dir().join("rusty_humanoid_cycle");
    let (a, b) = (dir.join("a.gltf"), dir.join("b.gltf"));
    model(&dir, "a.gltf", "Wave", &[&b]);
    model(&dir, "b.gltf", "Idle", &[&a]);
    assert_eq!(clips(&import_file(&a).unwrap()), ["Wave", "Idle"]);
    assert_eq!(clips(&import_file(&b).unwrap()), ["Idle", "Wave"]);
}

#[test]
fn the_cached_import_follows_its_sources_map() {
    let dir = crate::test_temp::dir().join("rusty_humanoid_cache");
    let a = model(&dir, "a.gltf", "Wave", &[]);
    let b = model(&dir, "b.gltf", "Idle", &[&a]);
    let wave = |b: &Path| imported(b).unwrap().sub_meshes[0].clips[1].clone();
    assert!(!wave(&b).tracks[0].is_empty());
    let mut settings = sidecar::load(&a).unwrap();
    settings.humanoid.bones.insert("Hips".into(), String::new());
    sidecar::save(&a, &settings).unwrap();
    assert!(
        wave(&b).tracks[0].is_empty(),
        "a source's unmapped hips drive nothing"
    );
}

#[test]
fn a_source_sharing_no_bone_lends_no_clip() {
    let dir = crate::test_temp::dir().join("rusty_humanoid_disjoint");
    let a = model(&dir, "a.gltf", "Wave", &[]);
    let mut settings = sidecar::load(&a).unwrap();
    settings.humanoid.bones.clear();
    sidecar::save(&a, &settings).unwrap();
    let b = import_file(&model(&dir, "b.gltf", "Idle", &[&a])).unwrap();
    assert_eq!(clips(&b), ["Idle"]);
}

#[test]
fn travel_on_a_root_above_the_hips_is_said() {
    use super::borrow::root_travel_lost;
    use super::rig_fixture::{rig, walk, Shape, HIPS};
    use super::BoneMap;
    let mut skin = rig(Shape::default());
    let mut clip = walk(&skin);
    let none = HumanoidSettings::default();
    assert_eq!(
        root_travel_lost(&skin, &BoneMap::resolve(&skin, &none), &[clip.clone()]),
        None
    );
    // The hips under a `root` (the spine, renamed), which carries the travel.
    skin.names[1] = "root".into();
    skin.parents[HIPS] = Some(1);
    clip.tracks[1].translation = clip.tracks[HIPS].translation.clone();
    let map = BoneMap::resolve(&skin, &none);
    let said = root_travel_lost(&skin, &map, &[clip]).unwrap();
    assert!(
        said.contains("'root' above the hips moves in clip 'Walk'"),
        "{said}"
    );
}
