//! The lightmap bake's Direct Samples (#2355): `0` follows `samples`, a set count
//! stands on its own, and it round-trips through the scene file and clamps.

use super::BakeSettings;
use crate::lighting::LightingSettings;

#[test]
fn direct_samples_follow_samples_until_set() {
    let follows = BakeSettings {
        samples: 256,
        ..Default::default()
    };
    assert_eq!(follows.direct_samples, 0);
    assert_eq!(follows.direct_sample_count(), 256);
    let set = BakeSettings {
        direct_samples: 32,
        ..follows
    };
    assert_eq!(set.direct_sample_count(), 32);
}

#[test]
fn direct_samples_round_trip_through_the_scene_file_and_clamp() {
    let mut settings = LightingSettings::default();
    settings.lightmaps.direct_samples = 48;
    let text = serde_json::to_string(&settings).unwrap();
    let back: LightingSettings = serde_json::from_str(&text).unwrap();
    assert_eq!(back.lightmaps.direct_samples, 48);
    let old: LightingSettings = serde_json::from_str(r#"{"lightmaps":{"samples":7}}"#).unwrap();
    assert_eq!(
        old.lightmaps.direct_sample_count(),
        7,
        "an older file follows samples"
    );
    let huge = BakeSettings {
        direct_samples: 1 << 20,
        ..Default::default()
    };
    assert_eq!(
        huge.sanitized().direct_samples,
        *BakeSettings::DIRECT_SAMPLES.end()
    );
}
