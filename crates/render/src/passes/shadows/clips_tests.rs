//! Which slots the depth prepass draws with no fragment stage (#2347): those of the
//! standard pipeline whose material nothing clips, by the shadow pass's own clip.

use super::*;

/// The clip a cutting variant's slot carries: its own pipeline's cut.
fn cutting(pipeline: usize) -> CasterClip {
    let clip = Clip::of(pipeline, 3);
    CasterClip { clip, cutout: None }
}

/// The clip a Cutout material carries, as `slot_clip` builds it on the standard
/// pipeline.
fn cutout(asset: &MaterialAsset) -> CasterClip {
    let cutout = Cutout::of(asset);
    assert!(cutout.is_some(), "a Cutout material has an alpha test");
    CasterClip {
        clip: Clip::of(STANDARD, 3),
        cutout,
    }
}

#[test]
fn a_plain_opaque_on_the_standard_pipeline_is_depth_only() {
    let opaque = MaterialAsset::default();
    assert_eq!(
        Cutout::of(&opaque),
        None,
        "an opaque material clips nothing"
    );
    assert!(depth_only(STANDARD, None));
}

#[test]
fn a_cutout_material_keeps_the_clipping_prepass() {
    let asset = MaterialAsset {
        render_mode: RenderMode::Cutout,
        ..MaterialAsset::default()
    };
    assert!(!depth_only(STANDARD, Some(&cutout(&asset))));
}

#[test]
fn a_cutting_or_vertex_moving_variant_keeps_its_own_prepass() {
    // Both carry their own pipeline's cut, Cutout or not (`slot_clip`).
    assert!(!depth_only(4, Some(&cutting(4))));
    // A variant nothing clips still draws its own prepass: only the standard
    // pipeline has a depth-only one.
    assert!(!depth_only(2, None));
}

#[test]
fn a_slot_with_no_material_is_depth_only_as_it_casts_unclipped() {
    // `push_slot` gives a slot with no material no clip, so it casts every
    // triangle; the prepass writes every one too.
    assert!(depth_only(STANDARD, None));
}
