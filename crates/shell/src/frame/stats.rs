//! Recording the window's frames into the frame stats (#2205), dev builds only: a
//! build without the `dev` layer records nothing, and these calls do nothing there.
//!
//! `Debug.Stats()` reads what the dev layer's schedule probe records per sim tick;
//! this adds what only the window knows, per drawn frame: its wall-clock frame time
//! and rate, and what the renderer drew for the screen ([`rusty_dev::stats::WindowFrame`]).
//! [`measured`] wraps each frame's draw, once per frame the loop draws.

use rusty_app::GameWorld;
#[cfg(feature = "dev")]
use rusty_dev::{capture::RenderedFrame, stats::WindowFrame};
#[cfg(feature = "dev")]
use {super::FrameClock, rusty_render::Renderer};

use crate::Shell;

/// Run the frontend's `draw` of this frame, then assemble its [`WindowFrame`] and
/// fold it into the stats `Debug.Stats()` reads. A frame the renderer drew carries
/// its counters, its CPU ms and the GPU times resolved since the last one; GPU
/// times wait for a frame that drew. A frame log on disk attaches here, to the
/// same [`WindowFrame`].
pub fn measured(
    shell: &mut Shell,
    game: &mut GameWorld,
    draw: impl FnOnce(&mut Shell, &mut GameWorld),
) {
    let frames_before = shell.renderer.frame_cost().frames;
    draw(shell, game);
    #[cfg(feature = "dev")]
    {
        let frame = window_frame(&mut shell.renderer, &shell.clock, frames_before);
        let stats = game.script_manager().stats_cell();
        rusty_dev::stats::record_window(&mut stats.borrow_mut(), &frame);
    }
    #[cfg(not(feature = "dev"))]
    let _ = frames_before;
}

/// What this frame cost: the clock's reading, and the renderer's frame if it drew
/// since its count read `frames_before`.
#[cfg(feature = "dev")]
pub fn window_frame(
    renderer: &mut Renderer,
    clock: &FrameClock,
    frames_before: u64,
) -> WindowFrame {
    let cost = renderer.frame_cost();
    let render = (cost.frames != frames_before).then(|| RenderedFrame {
        counters: renderer.frame_counters,
        cpu_ms: cost.cpu_ms,
        gpu: renderer.take_gpu_times(),
    });
    WindowFrame {
        wall_ms: f64::from(clock.frame_ms()),
        fps: f64::from(clock.fps()),
        render,
    }
}

#[cfg(all(test, feature = "dev"))]
#[path = "stats_tests.rs"]
mod stats_tests;
