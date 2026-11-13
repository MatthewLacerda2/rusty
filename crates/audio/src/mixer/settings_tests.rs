use super::*;

#[test]
fn a_patch_changes_only_the_fields_it_names_and_clamps_them() {
    let base = GroupSettings {
        volume: 0.4,
        ..Default::default()
    };
    let patch = GroupPatch {
        low_pass: Some(Filter::new(500.0, 3.0)),
        reverb_send: Some(2.0),
        ..Default::default()
    };
    let out = patch.apply(&base);
    assert_eq!(out.volume, 0.4, "volume untouched");
    assert_eq!(out.low_pass, Filter::new(500.0, 1.0));
    assert_eq!(out.reverb_send, 1.0);
    assert_eq!(
        Filter::new(1.0, 0.0).cutoff,
        HIGH_PASS_OFF,
        "cutoff clamped"
    );
}

#[test]
fn lerp_hits_both_ends_and_sweeps_cutoff_in_octaves() {
    let from = GroupSettings::default();
    let to = GroupSettings {
        volume: 0.0,
        low_pass: Filter::new(LOW_PASS_OFF / 4.0, 0.5),
        ..Default::default()
    };
    assert_eq!(from.lerp(&to, 0.0).volume, 1.0);
    let half = from.lerp(&to, 0.5);
    assert_eq!(half.volume, 0.5);
    // Two octaves down at the end, so one octave down halfway.
    assert!((half.low_pass.cutoff - LOW_PASS_OFF / 2.0).abs() < 1.0);
    assert_eq!(half.low_pass.resonance, 0.25);
    let end = from.lerp(&to, 1.0);
    assert!((end.low_pass.cutoff - to.low_pass.cutoff).abs() < 0.01);
}

#[test]
fn mute_and_duck_fold_into_the_device_volume() {
    let s = GroupSettings {
        volume: 0.8,
        ..Default::default()
    };
    assert_eq!(GroupMix::new(&s, 0.5).volume, 0.4);
    let muted = GroupSettings { mute: true, ..s };
    assert_eq!(GroupMix::new(&muted, 1.0).volume, 0.0);
}
