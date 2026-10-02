//! The maestro's mixer verbs against the recording backend (#465): what reaches
//! the device when scripts move groups, snapshots and ducks.

use crate::audio::mix::Shot;
use crate::audio::mixer::{Filter, GroupId, GroupPatch, DEFAULT_GROUPS};
use crate::audio::recording::RecordingBackend;
use crate::audio::AudioMaestro;
use crate::components::AudioSourceComponent;

fn maestro() -> (
    AudioMaestro,
    std::rc::Rc<std::cell::RefCell<crate::audio::recording::Recording>>,
) {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::default();
    m.set_backend(backend);
    (m, rec)
}

fn source(group: &str) -> AudioSourceComponent {
    AudioSourceComponent {
        clip: "a.ogg".to_string(),
        looping: true,
        output_group: group.to_string(),
        ..Default::default()
    }
}

#[test]
fn a_new_backend_gets_the_whole_group_tree() {
    let (_m, rec) = maestro();
    let rec = rec.borrow();
    assert_eq!(rec.groups_added.len(), DEFAULT_GROUPS.len());
    assert_eq!(rec.groups_added[0], (GroupId::MASTER, None));
    assert_eq!(rec.groups_added[3].1, Some(GroupId::MASTER));
}

#[test]
fn a_voice_routes_into_its_output_group_and_unknown_falls_back_to_master() {
    let (mut m, rec) = maestro();
    m.play_source(1, &source("Music"), [0.0; 3], 0);
    m.play_source(2, &source("Nope"), [0.0; 3], 0);
    let shot = Shot {
        group: "World",
        ..Shot::new("b.ogg", [0.0; 3])
    };
    m.play_at(&shot, 0, 0);
    let groups: Vec<_> = rec.borrow().plays.iter().map(|p| p.1.group).collect();
    let find = |n| m.mixer().find(n).unwrap();
    assert_eq!(groups, vec![find("Music"), GroupId::MASTER, find("World")]);
}

#[test]
fn the_recording_backend_sees_the_filter_cutoff() {
    let (mut m, rec) = maestro();
    let patch = GroupPatch {
        low_pass: Some(Filter::new(800.0, 0.3)),
        ..Default::default()
    };
    m.set_group("World", &patch).unwrap();
    let world = m.mixer().find("World").unwrap();
    let mix = rec.borrow().group(world).unwrap();
    assert_eq!(mix.low_pass, Filter::new(800.0, 0.3));
    assert!(m.set_group("Nope", &patch).is_err());
}

#[test]
fn a_snapshot_transition_reaches_the_device_frame_by_frame() {
    let (mut m, rec) = maestro();
    let quiet = GroupPatch {
        volume: Some(0.0),
        ..Default::default()
    };
    m.define_snapshot("Paused", &[("SFX".to_string(), quiet)])
        .unwrap();
    m.transition_to_snapshot("Paused", 1.0, 0.0).unwrap();
    let sfx = m.mixer().find("SFX").unwrap();
    m.advance_mixer(0.5);
    assert_eq!(rec.borrow().group(sfx).unwrap().volume, 0.5);
    m.advance_mixer(1.0);
    assert_eq!(rec.borrow().group(sfx).unwrap().volume, 0.0);
    assert_eq!(m.group_state("SFX").unwrap().effective_volume, 0.0);
}

#[test]
fn a_playing_voice_ducks_music_until_it_stops() {
    let (mut m, rec) = maestro();
    m.add_duck("Voice", "Music", 0.25, 0.0, 0.0).unwrap();
    let music = m.mixer().find("Music").unwrap();
    m.play_source(1, &source("Voice"), [0.0; 3], 0);
    m.advance_mixer(0.1);
    assert_eq!(rec.borrow().group(music).unwrap().volume, 0.25);
    m.stop_source(1, [0.0; 3], 0, true);
    m.advance_mixer(0.2);
    assert_eq!(rec.borrow().group(music).unwrap().volume, 1.0);
    assert!(m.add_duck("Voice", "Nope", 0.5, 0.0, 0.0).is_err());
}

#[test]
fn reset_restores_the_default_mixer() {
    let (mut m, _rec) = maestro();
    m.create_group("Guns", Some("SFX")).unwrap();
    m.set_group(
        "Music",
        &GroupPatch {
            mute: Some(true),
            ..Default::default()
        },
    )
    .unwrap();
    m.reset_mixer();
    assert!(m.group_state("Guns").is_none());
    assert!(!m.group_state("Music").unwrap().settings.mute);
}
