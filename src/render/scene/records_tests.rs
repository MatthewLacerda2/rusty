//! The material table's slots and the edit sort (#1706), adapter-free.

use super::*;
use crate::components::{RenderFace, RenderMode};

fn tint(r: f32) -> MaterialData {
    MaterialData::flat([r, 0.0, 0.0, 1.0])
}

#[test]
fn a_record_per_scene_name_and_variant_freed_with_its_name_or_scene() {
    let (a, b) = (SceneId::next(), SceneId::next());
    let mut m = MaterialRecords::default();
    let lit = m.slot((a, Some("red"), Variant::Lit), tint(0.1));
    assert_eq!(
        lit,
        m.slot((a, Some("red"), Variant::Lit), tint(0.2)),
        "kept"
    );
    assert_eq!(m.table.get(lit).color_tint[0], 0.2, "and rewritten");
    let unlit = m.slot((a, Some("red"), Variant::Unlit), tint(0.3));
    let other = m.slot((b, Some("red"), Variant::Lit), tint(0.4));
    let none = m.slot((a, None, Variant::Lit), tint(0.5));
    assert_eq!(
        [lit, unlit, other, none].map(|s| s != 0),
        [true; 4],
        "slot 0 is the flat record's"
    );
    let mut all = vec![lit, unlit, other, none];
    all.dedup();
    assert_eq!(
        all.len(),
        4,
        "a scene, a variant and no material each a slot"
    );
    assert_eq!(
        m.variants_of(a, "red"),
        vec![(Variant::Lit, lit), (Variant::Unlit, unlit)]
    );
    m.release_name(a, "red");
    assert!(m.variants_of(a, "red").is_empty());
    assert_eq!(m.variants_of(b, "red"), vec![(Variant::Lit, other)]);
    m.release_scene(b);
    assert_eq!(m.kept(), vec![none]);
    assert_eq!(m.table.live(), 2, "the flat record and `none`");
}

#[test]
fn a_value_edit_rewrites_and_a_key_edit_rekeys() {
    let base = MaterialAsset::default();
    let with = |f: fn(&mut MaterialAsset)| {
        let mut m = base.clone();
        f(&mut m);
        m
    };
    let values: [fn(&mut MaterialAsset); 5] = [
        |m| m.base_color = [0.2, 0.3, 0.4],
        |m| m.metallic = 0.7,
        |m| m.emissive = [4.0; 3],
        |m| m.tiling = [2.0, 2.0],
        |m| drop(m.shader_params.insert("tint".into(), vec![1.0])),
    ];
    for f in values {
        assert!(!rekeys(&base, &with(f)), "{:?}", with(f));
    }
    let keys: [fn(&mut MaterialAsset); 4] = [
        |m| m.render_mode = RenderMode::Transparent,
        |m| m.render_face = RenderFace::Both,
        |m| m.shader = Some("dissolve".into()),
        |m| m.base_color_map = Some("a.png".into()),
    ];
    for f in keys {
        assert!(rekeys(&base, &with(f)), "{:?}", with(f));
    }
    assert!(params_moved(&base, &with(values[4])));
    // A Cutout's test is a static bake's term: its cutoff and mapping re-key.
    let cutout = with(|m| m.render_mode = RenderMode::Cutout);
    let mut cut = cutout.clone();
    cut.alpha_cutoff = 0.9;
    assert!(rekeys(&cutout, &cut), "a cutoff edit re-bakes");
    cut = cutout.clone();
    cut.base_color = [0.0; 3];
    assert!(!rekeys(&cutout, &cut), "a cutout's colour does not");
    // A glass's alpha is its record's; its blend mode picks a pipeline.
    let glass = with(|m| m.render_mode = RenderMode::Transparent);
    let mut faded = glass.clone();
    faded.alpha = 0.2;
    assert!(!rekeys(&glass, &faded));
    faded.blend_mode = crate::components::BlendMode::Additive;
    assert!(rekeys(&glass, &faded));
}
