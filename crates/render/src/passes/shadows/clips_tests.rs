use super::*;

fn cutting(pipeline: usize) -> CasterClip {
    let clip = Clip::of(pipeline, 3);
    CasterClip { clip, cutout: None }
}

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
    assert!(!depth_only(4, Some(&cutting(4))));
    assert!(!depth_only(2, None));
}

#[test]
fn a_slot_with_no_material_is_depth_only_as_it_casts_unclipped() {
    assert!(depth_only(STANDARD, None));
}
