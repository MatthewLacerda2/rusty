//! The default humanoid bone guesser (#1747): a canonical bone from a joint's
//! name, for the naming schemes rigs commonly ship with — Mixamo's
//! (`mixamorig:LeftUpLeg`), Blender's (`thigh.L`, `upper_arm.R`), and the
//! `_l`/`_r` and `Left`/`Right` spellings. The first joint (in slot order) a
//! canonical bone matches takes it.

use super::CANONICAL;

/// Names a rig's tooling puts before the bone's own, dropped as whole tokens.
const PREFIXES: &[&str] = &["mixamorig", "def", "bip01", "bip001", "bone"];

/// A bone's side-free core name → its canonical bone, without the side.
const CORES: &[(&[&str], &str)] = &[
    (&["hips", "hip", "pelvis"], "Hips"),
    (&["spine", "spine0", "spine00"], "Spine"),
    (&["chest", "spine1", "spine01", "spine001"], "Chest"),
    (
        &["upperchest", "spine2", "spine02", "spine002"],
        "UpperChest",
    ),
    (&["neck"], "Neck"),
    (&["head"], "Head"),
    (
        &["shoulder", "clavicle", "collar", "collarbone"],
        "Shoulder",
    ),
    (&["upperarm", "arm", "uparm"], "UpperArm"),
    (&["lowerarm", "forearm", "loarm"], "LowerArm"),
    (&["hand", "wrist"], "Hand"),
    (&["upperleg", "upleg", "thigh"], "UpperLeg"),
    (&["lowerleg", "leg", "calf", "shin", "knee"], "LowerLeg"),
    (&["foot", "ankle"], "Foot"),
    (&["toes", "toe", "toebase", "ball"], "Toes"),
];

/// Canonical index → the first of `names` guessed as that bone.
pub(super) fn guess(names: &[String]) -> Vec<Option<usize>> {
    let mut slots = vec![None; CANONICAL.len()];
    for (slot, name) in names.iter().enumerate() {
        let Some(canonical) = canonical(name) else {
            continue;
        };
        let Some(i) = CANONICAL.iter().position(|c| *c == canonical) else {
            continue;
        };
        slots[i].get_or_insert(slot);
    }
    slots
}

/// The canonical bone `name` reads as, if any.
pub(super) fn canonical(name: &str) -> Option<String> {
    let lower = name.rsplit(':').next().unwrap_or(name).to_lowercase();
    let mut side = None;
    let mut core = String::new();
    let tokens = lower.split(|c: char| !c.is_ascii_alphanumeric());
    for (i, token) in tokens.filter(|t| !t.is_empty()).enumerate() {
        match token {
            "l" | "left" => side = Some("Left"),
            "r" | "right" => side = Some("Right"),
            t if i == 0 && PREFIXES.contains(&t) => {}
            t => core.push_str(t),
        }
    }
    for (word, named) in [("left", "Left"), ("right", "Right")] {
        if let Some(rest) = core.strip_prefix(word).filter(|r| !r.is_empty()) {
            core = rest.to_string();
            side = Some(named);
        }
    }
    let base = CORES
        .iter()
        .find(|(words, _)| words.contains(&core.as_str()))?
        .1;
    let sided = !matches!(
        base,
        "Hips" | "Spine" | "Chest" | "UpperChest" | "Neck" | "Head"
    );
    match (sided, side) {
        (false, None) => Some(base.to_string()),
        (true, Some(side)) => Some(format!("{side}{base}")),
        _ => None,
    }
}
