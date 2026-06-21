//! Tests for the audio play-mode system (#212): `play_on_start` auto-start.

use std::cell::RefCell;
use std::rc::Rc;

use crate::components::AudioSourceComponent;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

use crate::app::GameWorld;

/// Build a minimal play-ready world holding a single entity with an `AudioSource`.
fn world_with_audio(play_on_start: bool) -> GameWorld {
    let mut scene = Scene::new();
    let id = scene.add_entity("Speaker".to_string());
    if let Some(mut e) = scene.get_entity_mut(id) {
        e.audio = Some(AudioSourceComponent {
            clip: "music/theme.ogg".to_string(),
            play_on_start,
            looping: true,
            ..Default::default()
        });
    }
    let scene = Rc::new(RefCell::new(scene));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -5.0, 5.0, -5.0, 5.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    GameWorld::new(scene, input, nav, console)
}

#[test]
fn play_on_start_sources_auto_start_on_entering_play() {
    let mut world = world_with_audio(true);
    world.set_playing(true);
    world.tick(1.0 / 60.0); // enter_play runs the Startup stage

    let audio = world.resources.audio.borrow();
    // Entity 1 is the only entity; its source should be live and logged.
    assert!(
        audio.is_source_playing(1),
        "play_on_start should auto-start"
    );
    let ev = audio.events().last().expect("a Play event was logged");
    assert_eq!(ev.clip, "music/theme.ogg");
    assert_eq!(ev.entity, 1);
}

#[test]
fn sources_without_play_on_start_stay_silent() {
    let mut world = world_with_audio(false);
    world.set_playing(true);
    world.tick(1.0 / 60.0);

    let audio = world.resources.audio.borrow();
    assert!(!audio.is_source_playing(1));
    assert!(audio.events().is_empty(), "nothing should have auto-played");
}

#[test]
fn leaving_play_silences_voices() {
    let mut world = world_with_audio(true);
    world.set_playing(true);
    world.tick(1.0 / 60.0);
    assert!(world.resources.audio.borrow().is_source_playing(1));

    world.set_playing(false);
    world.tick(1.0 / 60.0); // exit_play stops all voices
    assert!(!world.resources.audio.borrow().is_source_playing(1));
}
