//! The tick hands the sim's game time to the renderer (#398): `Scene::shader_time`
//! follows `Time::time` in Play — scaled, frozen at time scale 0 — and is 0 in edit
//! mode, so edit-mode and preview shots stay pixel-comparable.

use std::cell::RefCell;
use std::rc::Rc;

use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

const DT: f32 = 0.25;

fn world() -> GameWorld {
    GameWorld::new(
        Rc::new(RefCell::new(Scene::new())),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -20.0, 20.0, -20.0, 20.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    )
}

fn shader_time(gw: &GameWorld) -> f32 {
    gw.scene().borrow().shader_time
}

#[test]
fn edit_mode_renders_at_time_zero() {
    let mut gw = world();
    gw.tick(DT);
    gw.tick(DT);
    assert_eq!(shader_time(&gw), 0.0);
}

#[test]
fn play_mode_shader_time_is_scaled_game_time() {
    let mut gw = world();
    gw.set_playing(true);
    gw.tick(DT); // entering Play resets the clock, then advances one frame
    gw.tick(DT);
    assert_eq!(shader_time(&gw), 0.5);

    // Time scale 0 freezes shader animation with the game; the UI clock (#427)
    // keeps running, so a paused menu still animates.
    gw.time().borrow_mut().set_time_scale(0.0);
    gw.tick(DT);
    assert_eq!(shader_time(&gw), 0.5);
    assert_eq!(gw.scene().borrow().ui_time, 0.75);

    gw.time().borrow_mut().set_time_scale(1.0);
    gw.set_playing(false);
    gw.tick(DT);
    assert_eq!(shader_time(&gw), 0.0, "Stop returns to the edit-mode time");
    assert_eq!(gw.scene().borrow().ui_time, 0.0);
}
