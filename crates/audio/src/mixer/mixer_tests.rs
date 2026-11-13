//! Mixer tests (#465): the default tree, snapshot blends, ducking.

use super::*;

fn world_muffled() -> Vec<(GroupId, GroupPatch)> {
    let world = Mixer::default().find("World").unwrap();
    vec![(
        world,
        GroupPatch {
            volume: Some(0.5),
            low_pass: Some(Filter::new(LOW_PASS / 4.0, 0.0)),
            ..Default::default()
        },
    )]
}
use settings::LOW_PASS_OFF as LOW_PASS;

#[test]
fn the_default_tree_is_master_and_five_buses() {
    let mixer = Mixer::default();
    let names: Vec<_> = mixer.groups().iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, DEFAULT_GROUPS);
    assert!(mixer.groups()[1..]
        .iter()
        .all(|g| g.parent == Some(GroupId::MASTER)));
    assert!(mixer.require("Nope").unwrap_err().contains("Music"));
}

#[test]
fn snapshot_blends_at_zero_half_and_one() {
    let mut m = Mixer::default();
    let world = m.find("World").unwrap();
    m.define_snapshot("BulletTime", world_muffled());
    m.transition_to("BulletTime", 2.0, 10.0).unwrap();
    assert_eq!(m.state(world).settings.volume, 1.0, "t = 0");
    m.advance(11.0, |_| false);
    let half = m.state(world).settings;
    assert_eq!(half.volume, 0.75, "t = ½");
    assert!(
        (half.low_pass.cutoff - LOW_PASS / 2.0).abs() < 1.0,
        "an octave down"
    );
    assert_eq!(m.snapshot(), Some(("BulletTime", 0.5)));
    m.advance(12.0, |_| false);
    assert_eq!(m.state(world).settings.volume, 0.5, "t = 1");
    m.advance(30.0, |_| false);
    assert_eq!(m.state(world).settings.volume, 0.5, "holds after the end");
    let music = m.find("Music").unwrap();
    assert_eq!(
        m.state(music).settings,
        GroupSettings::default(),
        "untouched"
    );
}

#[test]
fn a_zero_second_transition_lands_at_once() {
    let mut m = Mixer::default();
    m.define_snapshot("Flashbanged", world_muffled());
    m.transition_to("Flashbanged", 0.0, 0.0).unwrap();
    let world = m.find("World").unwrap();
    assert_eq!(m.state(world).settings.volume, 0.5);
    assert!(m.transition_to("Missing", 1.0, 0.0).is_err());
}

#[test]
fn a_direct_set_survives_a_transition_in_flight() {
    let mut m = Mixer::default();
    let world = m.find("World").unwrap();
    m.define_snapshot("BulletTime", world_muffled());
    m.transition_to("BulletTime", 2.0, 0.0).unwrap();
    m.set(
        world,
        &GroupPatch {
            volume: Some(0.2),
            ..Default::default()
        },
    );
    m.advance(1.0, |_| false);
    assert_eq!(m.state(world).settings.volume, 0.2);
}

#[test]
fn a_child_group_follows_its_parent_volume() {
    let mut m = Mixer::default();
    let sfx = m.find("SFX").unwrap();
    let guns = m.create("Guns", sfx).unwrap();
    m.set(
        sfx,
        &GroupPatch {
            volume: Some(0.5),
            ..Default::default()
        },
    );
    assert_eq!(m.state(guns).effective_volume, 0.5);
    assert_eq!(m.state(guns).parent.as_deref(), Some("SFX"));
    assert!(m.create("Guns", sfx).is_err(), "names are unique");
}

#[test]
fn ducking_engages_with_attack_and_releases() {
    let mut m = Mixer::default();
    let (voice, music) = (m.find("Voice").unwrap(), m.find("Music").unwrap());
    m.add_duck(Duck::new(voice, music, 0.2, 0.5, 1.0));
    m.advance(0.0, |_| false);
    assert_eq!(m.state(music).duck, 1.0, "idle");
    m.advance(0.25, |g| g == voice);
    assert!((m.state(music).duck - 0.6).abs() < 1e-6, "half attack");
    m.advance(1.0, |g| g == voice);
    assert!((m.mix(music).volume - 0.2).abs() < 1e-6, "fully ducked");
    m.advance(1.5, |_| false);
    assert!((m.state(music).duck - 0.6).abs() < 1e-6, "half release");
    m.advance(3.0, |_| false);
    assert_eq!(m.state(music).duck, 1.0, "released");
}

#[test]
fn a_voice_in_a_child_group_triggers_its_parents_duck() {
    let mut m = Mixer::default();
    let (voice, music) = (m.find("Voice").unwrap(), m.find("Music").unwrap());
    let radio = m.create("Radio", voice).unwrap();
    m.add_duck(Duck::new(voice, music, 0.0, 0.0, 0.0));
    m.advance(0.1, |g| g == radio);
    assert_eq!(m.mix(music).volume, 0.0);
    m.clear_ducks();
    assert_eq!(m.mix(music).volume, 1.0);
}
