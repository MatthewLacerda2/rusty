//! The humanoid bone guesser (#1747) on the naming schemes it claims.

use super::guess::{canonical, guess};
use super::CANONICAL;

fn reads(name: &str) -> Option<String> {
    canonical(name)
}

#[test]
fn mixamo_names_read_as_their_bones() {
    let cases = [
        ("mixamorig:Hips", "Hips"),
        ("mixamorig:Spine1", "Chest"),
        ("mixamorig:Spine2", "UpperChest"),
        ("mixamorig:LeftUpLeg", "LeftUpperLeg"),
        ("mixamorig:LeftLeg", "LeftLowerLeg"),
        ("mixamorig:RightArm", "RightUpperArm"),
        ("mixamorig:RightForeArm", "RightLowerArm"),
        ("mixamorig:LeftToeBase", "LeftToes"),
        ("mixamorig_Head", "Head"),
    ];
    for (name, want) in cases {
        assert_eq!(reads(name).as_deref(), Some(want), "{name}");
    }
}

#[test]
fn blender_and_underscore_sides_read() {
    let cases = [
        ("thigh.L", "LeftUpperLeg"),
        ("shin.R", "RightLowerLeg"),
        ("upper_arm.L", "LeftUpperArm"),
        ("forearm_r", "RightLowerArm"),
        ("hand_l", "LeftHand"),
        ("DEF-foot.R", "RightFoot"),
        ("L_Clavicle", "LeftShoulder"),
        ("pelvis", "Hips"),
    ];
    for (name, want) in cases {
        assert_eq!(reads(name).as_deref(), Some(want), "{name}");
    }
}

#[test]
fn a_sideless_limb_or_a_sided_spine_reads_as_nothing() {
    for name in [
        "hand", "foot", "Head_end", "spine.L", "Armature", "weapon_r",
    ] {
        assert_eq!(reads(name), None, "{name}");
    }
}

#[test]
fn the_first_joint_a_bone_matches_takes_it() {
    let names = ["root", "Hips", "pelvis", "LeftFoot"].map(String::from);
    let slots = guess(&names, |_| false);
    let at = |c: &str| slots[CANONICAL.iter().position(|n| *n == c).unwrap()];
    assert_eq!(at("Hips"), Some(1));
    assert_eq!(at("LeftFoot"), Some(3));
    assert_eq!(at("Head"), None);
}

#[test]
fn an_override_takes_its_joint_from_the_guess_holding_it() {
    use super::rig_fixture::{rig, Shape};
    use super::{BoneMap, HumanoidSettings};
    let mut skin = rig(Shape::default());
    skin.names[3] = "leg.L".into();
    skin.names[4] = "shin.L".into();
    let mut settings = HumanoidSettings::default();
    let guessed = BoneMap::resolve(&skin, &settings);
    assert_eq!(
        guessed.slot("LeftLowerLeg"),
        Some(3),
        "'leg' reads as a shin"
    );
    settings.bones.insert("LeftUpperLeg".into(), "leg.L".into());
    let map = BoneMap::resolve(&skin, &settings);
    assert_eq!(map.slot("LeftUpperLeg"), Some(3));
    assert_eq!(
        map.slot("LeftLowerLeg"),
        Some(4),
        "guessed again without it"
    );
    let said = map.problems.join("; ");
    assert!(said.contains("LeftLowerLeg: 'leg.L'"), "{said}");
    settings
        .bones
        .insert("RightUpperLeg".into(), "leg.L".into());
    let twice = BoneMap::resolve(&skin, &settings).problems.join("; ");
    assert!(twice.contains("both name 'leg.L'"), "{twice}");
}
