//! Particle emitter modules (#439): shapes, start ranges, over-life curves and
//! sub-emitters, driven through the real play loop (`GameWorld::tick`).

mod api;
mod replay;
mod sub_emitters;

use std::cell::RefCell;
use std::rc::Rc;

use rusty::app::GameWorld;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

/// Enter Play on `scene` and step it `frames` fixed 60 Hz ticks.
fn play(scene: Scene, frames: u32) -> Rc<RefCell<GameWorld>> {
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let world = crate::play::world(Rc::new(RefCell::new(scene)), 20.0, console);
    let world = Rc::new(RefCell::new(world));
    step(&world, frames);
    world
}

/// Step an already-playing world `frames` more ticks.
fn step(world: &Rc<RefCell<GameWorld>>, frames: u32) {
    let mut w = world.borrow_mut();
    w.set_playing(true);
    for _ in 0..frames {
        w.tick(1.0 / 60.0);
    }
}

/// The live particle count of `id`'s emitter.
fn count(world: &Rc<RefCell<GameWorld>>, id: u32) -> usize {
    let w = world.borrow();
    let scene = w.scene().borrow();
    scene.world.particles(id).map_or(0, |p| p.live_count())
}
