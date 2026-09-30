//! Unit tests for the shell's window-free frame logic: the frame clock, the sim
//! advance (live and paused-and-stepped), and the per-host quit semantics.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use super::*;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

fn playing_world() -> GameWorld {
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -20.0, 20.0, -20.0, 20.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    game.boot_standalone();
    game
}

#[test]
fn clock_reports_the_delta_and_counts_fps_per_second() {
    let start = Instant::now();
    let mut clock = FrameClock::new(start);
    let dt = clock.tick(start + Duration::from_millis(16));
    assert!((dt - 0.016).abs() < 1e-6);
    assert!((clock.frame_ms() - 16.0).abs() < 1e-3);
    // 30 more frames inside the first second, then the frame at 1 s: 32 counted.
    for i in 2..=31 {
        clock.tick(start + Duration::from_millis(16 * i));
    }
    clock.tick(start + Duration::from_millis(1000));
    assert_eq!(clock.fps(), 32.0);
}

#[test]
fn clock_never_goes_backwards() {
    let start = Instant::now() + Duration::from_secs(1);
    let mut clock = FrameClock::new(start);
    assert_eq!(clock.tick(start - Duration::from_millis(5)), 0.0);
}

#[test]
fn live_advance_ticks_with_the_wall_clock_delta() {
    let mut game = playing_world();
    assert!(advance_sim(&mut game, 0.02) == PlayTransition::Entered);
    advance_sim(&mut game, 0.02);
    assert_eq!(game.play_frame(), 2);
}

#[test]
fn paused_advance_runs_only_the_queued_fixed_steps() {
    let mut game = playing_world();
    advance_sim(&mut game, 0.02); // enter Play
    let before = game.play_frame();
    game.time().borrow_mut().pause();
    advance_sim(&mut game, 0.5);
    assert_eq!(
        game.play_frame(),
        before,
        "a paused frame with no steps is frozen"
    );
    game.time().borrow_mut().request_steps(3);
    advance_sim(&mut game, 0.5);
    assert_eq!(game.play_frame(), before + 3);
}

#[test]
fn paused_advance_still_processes_a_stop() {
    let mut game = playing_world();
    advance_sim(&mut game, 0.02);
    game.time().borrow_mut().pause();
    game.set_playing(false);
    assert!(advance_sim(&mut game, 0.02) == PlayTransition::Exited);
}

#[test]
fn quit_closes_the_player_and_stops_play_in_the_editor() {
    use QuitAction::*;
    assert_eq!(quit_action(Host::Player, true, true), Exit);
    assert_eq!(quit_action(Host::Editor, true, true), StopPlay);
    // Outside Play the editor has no game to end.
    assert_eq!(quit_action(Host::Editor, true, false), None);
    for host in [Host::Editor, Host::Player] {
        assert_eq!(quit_action(host, false, true), None);
    }
}
