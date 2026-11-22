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
            "volumetric_fog",
            "solids",
            "occlusion",
            "scene",
            "ssil_source",
            "ssr",
            "opaque_copy",
            "transparent",
            "depth_postpass",
            "world_ui",
            "effects",
            "gizmo_depth",
            "count",
            "post_fx",
            "outline",
            "handles",
        ]
    );
}

#[test]
fn every_stage_reads_only_what_an_earlier_stage_wrote() {
    assert_eq!(position("camera_uniform"), 0);
    assert!(position("clusters") < position("scene"));
    assert!(position("clusters") < position("volumetric_fog"));
    assert!(position("volumetric_fog") < position("scene"));
    assert!(position("solids") < position("scene"));
    assert!(position("solids") < position("transparent"));
    assert_eq!(position("occlusion"), position("solids") + 1);
    assert!(position("world_ui") < position("effects"));
    assert_eq!(position("ssil_source"), position("scene") + 1);
    assert_eq!(position("ssr"), position("ssil_source") + 1);
    assert_eq!(position("opaque_copy"), position("ssr") + 1);
    assert_eq!(position("transparent"), position("opaque_copy") + 1);
    assert_eq!(position("depth_postpass"), position("transparent") + 1);
    assert!(position("effects") < position("count"));
    assert!(position("effects") < position("post_fx"));
    assert_eq!(position("outline"), position("post_fx") + 1);
    assert_eq!(position("handles"), CAMERA_PASSES.len() - 1);
    assert!(position("depth_postpass") < position("gizmo_depth"));
    assert!(position("gizmo_depth") < position("post_fx"));
}

#[test]
fn stage_names_are_unique() {
    let mut names: Vec<&str> = CAMERA_PASSES.iter().map(|(name, _)| *name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), CAMERA_PASSES.len());
}

#[test]
fn only_a_stacked_depth_only_camera_is_an_overlay() {
    use super::is_overlay;
    use rusty_scene::ClearFlags;
    assert!(is_overlay(false, ClearFlags::DepthOnly));
    // A base camera is never one, whatever it clears; nor is a stacked one that clears colour.
    assert!(!is_overlay(true, ClearFlags::DepthOnly));
    assert!(!is_overlay(false, ClearFlags::Skybox));
    assert!(!is_overlay(false, ClearFlags::SolidColor));
}

#[test]
fn only_the_base_camera_traces_transparent_ssr() {
    use super::ssr_march;
    let march = [32.0, 0.3, 0.7, 0.2];
    assert_eq!(ssr_march(true, march), (march, true));
    assert_eq!(ssr_march(false, march), ([0.0; 4], false));
    assert_eq!(ssr_march(true, [0.0; 4]), ([0.0; 4], false));
}

#[test]
fn early_flushes_follow_their_stages_in_pass_order_and_each_shape_uses_a_prefix() {
    use super::{flushes_after, EARLY_FLUSHES};
    use crate::EarlyFlushes;
    let at: Vec<usize> = EARLY_FLUSHES.iter().map(|n| position(n)).collect();
    assert!(at.windows(2).all(|w| w[0] < w[1]), "in pass order: {at:?}");
    // The depth work goes first, then the opaque pass (`EARLY_FLUSHES`' why).
    assert_eq!(EARLY_FLUSHES, ["occlusion", "scene"]);
    let after = |f| -> Vec<&str> {
        let names = CAMERA_PASSES.iter().map(|(name, _)| *name);
        names.filter(|n| flushes_after(f, n)).collect()
    };
    assert!(after(EarlyFlushes::None).is_empty());
    assert_eq!(after(EarlyFlushes::Occlusion), ["occlusion"]);
    assert_eq!(after(EarlyFlushes::OcclusionScene), ["occlusion", "scene"]);
    assert_eq!(EarlyFlushes::default(), EarlyFlushes::OcclusionScene);
}
