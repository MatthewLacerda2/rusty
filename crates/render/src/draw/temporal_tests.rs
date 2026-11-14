//! What a material packs for TAA (#1845), without a GPU: its reactivity and
//! whether it writes its motion, on a `Transparent` surface only.

use super::surface_uniform;
use crate::draw::batch::RecordTraits;
use crate::MaterialData;
use rusty_components::{MaterialAsset, RenderMode, TaaReactive, MAX_REACTIVE};

fn glass(reactive: TaaReactive, writes_motion: bool) -> MaterialAsset {
    MaterialAsset {
        render_mode: RenderMode::Transparent,
        reactive,
        writes_motion,
        ..MaterialAsset::default()
    }
}

#[test]
fn a_transparent_surface_packs_its_reactivity() {
    let r = |m: &MaterialAsset| surface_uniform(true, Some(m)).reactive;
    assert!(
        r(&glass(TaaReactive::Auto, false)) < 0.0,
        "Auto reads below 0"
    );
    assert_eq!(r(&glass(TaaReactive::Off, false)), 0.0);
    assert_eq!(r(&glass(TaaReactive::Full, false)), MAX_REACTIVE);
    let opaque = MaterialAsset {
        reactive: TaaReactive::Full,
        ..MaterialAsset::default()
    };
    assert_eq!(r(&opaque), 0.0, "an opaque surface writes none");
}

#[test]
fn writing_motion_is_a_trait_of_glass_only() {
    let writes = |m: &MaterialAsset| {
        let record = MaterialData {
            surface: surface_uniform(true, Some(m)),
            ..bytemuck::Zeroable::zeroed()
        };
        RecordTraits::of(&record).writes_motion
    };
    assert!(writes(&glass(TaaReactive::Auto, true)));
    assert!(!writes(&glass(TaaReactive::Auto, false)));
    let opaque = MaterialAsset {
        writes_motion: true,
        ..MaterialAsset::default()
    };
    assert!(!writes(&opaque), "an opaque surface writes motion anyway");
}

#[test]
fn reactivity_moves_the_record_alone_and_motion_rekeys() {
    use crate::scene::records::rekeys;
    let auto = glass(TaaReactive::Auto, false);
    assert!(!rekeys(&auto, &glass(TaaReactive::Full, false)));
    assert!(rekeys(&auto, &glass(TaaReactive::Auto, true)));
}
