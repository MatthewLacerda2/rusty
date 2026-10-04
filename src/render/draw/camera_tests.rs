//! The per-camera pass list's order (#636): the frame's dependencies, pinned so a
//! new row cannot silently land before the stage whose output it reads.

use super::CAMERA_PASSES;

fn position(name: &str) -> usize {
    CAMERA_PASSES
        .iter()
        .position(|(stage, _)| *stage == name)
        .unwrap_or_else(|| panic!("no `{name}` stage in CAMERA_PASSES"))
}

#[test]
fn stages_run_in_the_frames_order() {
    let names: Vec<&str> = CAMERA_PASSES.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        names,
        [
            "camera_uniform",
            "clusters",
            "solids",
            "scene",
            "transparent",
            "world_ui",
            "effects",
            "count",
            "post_fx",
        ]
    );
}

#[test]
fn every_stage_reads_only_what_an_earlier_stage_wrote() {
    // The uniform is on the GPU before anything draws with it.
    assert_eq!(position("camera_uniform"), 0);
    // Every lit pass reads this camera's light and decal clusters (#434, #638).
    assert!(position("clusters") < position("scene"));
    // The scene and transparent passes draw the solids batch.
    assert!(position("solids") < position("scene"));
    assert!(position("solids") < position("transparent"));
    assert!(position("world_ui") < position("effects"));
    // The counters read the solids and the effects' draw counts.
    assert!(position("effects") < position("count"));
    // Post-FX composites everything this camera drew.
    assert_eq!(position("post_fx"), CAMERA_PASSES.len() - 1);
}

#[test]
fn stage_names_are_unique() {
    let mut names: Vec<&str> = CAMERA_PASSES.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), CAMERA_PASSES.len());
}
