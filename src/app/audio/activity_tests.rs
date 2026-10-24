//! Tests for an `AudioSource` following its entity's activity (#1363): a pivot
//! with two sounding children, the issue's holstered-weapon shape.

use std::cell::RefCell;
use std::rc::Rc;

use crate::app::GameWorld;
use crate::audio::AudioEventKind;
use crate::components::AudioSourceComponent;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

const DT: f32 = 1.0 / 60.0;

/// A pivot (id 1) with two looping children (ids 2, 3), both `play_on_start`.
fn weapon() -> GameWorld {
    let mut scene = Scene::new();
    let pivot = scene.add_entity("Pivot".to_string());
    for name in ["Shotgun", "Arms"] {
        let child = scene.add_entity(name.to_string());
        scene.set_parent(child, Some(pivot)).unwrap();
        let hum = AudioSourceComponent {
            clip: format!("{name}.ogg"),
            play_on_start: true,
            looping: true,
            ..Default::default()
        };
        scene.world.set_audio(child, Some(hum));
    }
    let scene = Rc::new(RefCell::new(scene));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -5.0, 5.0, -5.0, 5.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut world = GameWorld::new(scene, input, nav, console);
    world.set_playing(true);
    world.tick(DT);
    world
}

fn set_pivot_active(world: &GameWorld, active: bool) {
    world.scene().borrow_mut().world.set_active(1, active);
}

fn playing(world: &GameWorld) -> [bool; 2] {
    let audio = world.resources.audio.borrow();
    [audio.is_source_playing(2), audio.is_source_playing(3)]
}

#[test]
fn hiding_the_pivot_stops_its_childrens_voices() {
    let mut world = weapon();
    assert_eq!(playing(&world), [true, true]);
    set_pivot_active(&world, false);
    world.tick(DT);
    assert_eq!(
        playing(&world),
        [false, false],
        "OnDisable silences the subtree"
    );
    let audio = world.resources.audio.borrow();
    let stops: Vec<u32> = audio
        .events()
        .iter()
        .filter(|e| e.kind == AudioEventKind::Stop)
        .map(|e| e.entity)
        .collect();
    assert_eq!(stops, [2, 3], "one Stop per child, in id order");
    let scene = world.scene().borrow();
    assert!(
        scene.world.is_active_self(2),
        "the children's own flags stay set"
    );
}

#[test]
fn showing_the_pivot_again_replays_play_on_start_once() {
    let mut world = weapon();
    set_pivot_active(&world, false);
    world.tick(DT);
    set_pivot_active(&world, true);
    world.tick(DT);
    world.tick(DT);
    assert_eq!(
        playing(&world),
        [true, true],
        "play-on-enable restarts both"
    );
    let plays = world
        .resources
        .audio
        .borrow()
        .events()
        .iter()
        .filter(|e| e.kind == AudioEventKind::Play)
        .count();
    assert_eq!(plays, 4, "two at Play, two on enable, no repeats");
}

#[test]
fn a_source_inactive_at_play_starts_when_its_pivot_is_shown() {
    let mut world = weapon();
    world.set_playing(false);
    world.tick(DT);
    set_pivot_active(&world, false);
    world.set_playing(true);
    world.tick(DT);
    assert_eq!(
        playing(&world),
        [false, false],
        "a holstered weapon stays silent"
    );
    set_pivot_active(&world, true);
    world.tick(DT);
    assert_eq!(playing(&world), [true, true]);
}
