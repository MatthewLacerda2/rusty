//! Booting a Play session in a test (#939): the `GameWorld` over a scene that a
//! dozen modules built by hand, and the "write this Lua and attach it" step half of
//! them repeated.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::app::GameWorld;
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::{Scene, ScriptComponent};
use rusty::scripting::ConsoleLogs;

/// One fixed 60 Hz step.
pub const DT: f32 = 1.0 / 60.0;

/// A `GameWorld` over `scene`, fresh input, a flat nav graph spanning ±`half`
/// metres and `console`. Not yet playing.
pub fn world(scene: Rc<RefCell<Scene>>, half: f32, console: Rc<RefCell<ConsoleLogs>>) -> GameWorld {
    let nav = NavigationGraph::new(-half, half, -half, half, 1.0);
    GameWorld::new(
        scene,
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        console,
    )
}

/// [`world`] with its own console, set playing.
pub fn playing(scene: Rc<RefCell<Scene>>, half: f32) -> GameWorld {
    let mut game = world(scene, half, Rc::new(RefCell::new(ConsoleLogs::new())));
    game.set_playing(true);
    game
}

/// Step `game` `ticks` times by [`DT`].
pub fn run(game: &mut GameWorld, ticks: usize) {
    for _ in 0..ticks {
        game.tick(DT);
    }
}

/// Write `source` to `file` in the test's temp dir and make it entity `id`'s only
/// script.
pub fn attach_script(scene: &mut Scene, id: u32, file: &str, source: &str) {
    let path = crate::temp::dir().join(file);
    std::fs::write(&path, source).unwrap();
    *scene.world.scripts_mut(id).unwrap() = vec![ScriptComponent {
        path: path.to_string_lossy().replace('\\', "/"),
        ..Default::default()
    }];
}

/// Evaluate `line` in a harness world's gameplay VM, as the console would.
#[cfg(feature = "dev")]
pub fn eval(h: &rusty::dev::harness::Harness, line: &str) -> String {
    let world = h.world.borrow();
    rusty::dev::console::evaluate_line(world.script_manager(), world.console(), line)
        .expect("line evaluates")
}
