//! src/shell/mod.rs — the runtime shell: one window, one frame loop, two frontends.
//!
//! Everything a windowed rusty run needs **whether or not the editor is present**
//! lives here (#431): the window and wgpu surface, the frame clock, advancing the sim
//! (including the paused-and-stepped mode), audio, the keymap and the input pump, the
//! persisted video/quality settings, the cursor policy, what `Application.Quit()`
//! means, and the dev-only socket command channel. A [`Frontend`] adds what differs:
//!
//! * [`editor::EditorFrontend`] (the `editor` Cargo feature) — the egui dashboard, the
//!   Scene/Game viewports rendered offscreen, and the edit snapshot on Play/Stop. The
//!   `rusty` binary.
//! * [`player::PlayerFrontend`] — no chrome: boots the configured startup scene
//!   straight into Play and renders the camera stack onto the swapchain. The `player`
//!   binary, the thing a game ships as.
//!
//! The shell is platform layer (like `render`), so it may read the wall clock; the
//! sim it drives stays a pure function of `(seed, inputs, fixed dt)`.
//!
//! [`Frontend`] is not a plugin trait: it has exactly two compile-time
//! implementations, one per binary, and exists so the loop is written once.

pub mod boot;
pub mod frame;
pub mod input;
pub mod player;
pub mod settings;

#[cfg(feature = "editor")]
pub mod editor;

use std::sync::Arc;
use std::time::Instant;

use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::app::GameWorld;
use crate::core::keymap::Keymap;
use crate::core::video::VideoSettings;
use crate::render::Renderer;

use frame::{FrameClock, Host, QuitAction};

/// The frontend-independent window state threaded through the event loop.
pub struct Shell {
    pub window: Arc<winit::window::Window>,
    pub renderer: Renderer,
    pub clock: FrameClock,
    /// The physical→logical key remap, loaded once from `Storage` at boot.
    pub keymap: Keymap,
    /// Video settings currently in effect on the surface/window, so the per-frame
    /// apply ([`settings::apply_pending`]) reconfigures only what a script changed.
    pub applied_video: VideoSettings,
    /// Dev-only socket command channel (#282): an external process drives this
    /// running playtest through the same evaluator the console uses. `None` if the
    /// socket failed to bind — the window keeps running without it.
    #[cfg(feature = "dev")]
    pub cmd_channel: Option<crate::dev::command_channel::CommandChannel>,
}

/// What one binary adds on top of the shell. See the module docs.
pub trait Frontend {
    /// Which host this is — decides what `Application.Quit()` means.
    const HOST: Host;

    /// See every window event before the shell routes it (the editor feeds egui).
    fn on_window_event(&mut self, _shell: &Shell, _event: &WindowEvent) {}

    /// Whether keyboard input reaches the game right now. The seam for per-frontend
    /// input routing (#416: the editor will route only while the Game view has focus).
    fn game_has_input(&self, _game: &GameWorld) -> bool {
        true
    }

    /// A physical key changed state, after the shell wrote it into the sim.
    fn on_key(&mut self, _game: &mut GameWorld, _key: KeyCode, _pressed: bool) {}

    /// Draw this frame into the swapchain `target`.
    fn draw(&mut self, shell: &mut Shell, game: &mut GameWorld, target: &wgpu::TextureView);
}

/// Run the event loop until the window closes or the game quits. Persists the
/// settings and flushes `Storage` however the loop exits.
pub fn run<F: Frontend + 'static>(
    event_loop: EventLoop<()>,
    mut shell: Shell,
    mut game: GameWorld,
    mut frontend: F,
) {
    let _ = event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);
        match event {
            Event::WindowEvent {
                window_id,
                ref event,
            } if window_id == shell.window.id() => {
                frontend.on_window_event(&shell, event);
                handle_window_event(event, elwt, &mut shell, &mut game, &mut frontend);
            }
            Event::AboutToWait => shell.window.request_redraw(),
            // Quit boundary: persist the store however the loop is exiting (window
            // close, `Application.Quit`, surface OOM). A no-op when the store is pathless.
            Event::LoopExiting => {
                settings::persist(&shell, &game);
                game.resources.flush_storage();
            }
            _ => {}
        }
    });
}

/// Dispatch one window event for our window.
fn handle_window_event<F: Frontend>(
    event: &WindowEvent,
    elwt: &EventLoopWindowTarget<()>,
    shell: &mut Shell,
    game: &mut GameWorld,
    frontend: &mut F,
) {
    match event {
        WindowEvent::CloseRequested => elwt.exit(),
        WindowEvent::Resized(size) => shell.renderer.resize(*size),
        // A DPI / display change (e.g. dragging between Retina and non-Retina
        // monitors) can invalidate the swapchain without a Resized event.
        WindowEvent::ScaleFactorChanged { .. } => {
            shell.renderer.resize(shell.window.inner_size());
        }
        WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    physical_key: PhysicalKey::Code(key),
                    state,
                    ..
                },
            ..
        } => {
            let pressed = *state == ElementState::Pressed;
            if frontend.game_has_input(game) {
                input::write_key(*key, pressed, game, &shell.keymap);
            }
            frontend.on_key(game, *key, pressed);
        }
        WindowEvent::CursorMoved { position, .. } => {
            game.input().borrow_mut().mouse_position = (position.x, position.y);
        }
        WindowEvent::RedrawRequested => run_frame(elwt, shell, game, frontend),
        _ => {}
    }
}

/// One frame: advance the sim, act on play transitions and a quit request, apply
/// scripted video settings, then let the frontend draw onto the swapchain.
fn run_frame<F: Frontend>(
    elwt: &EventLoopWindowTarget<()>,
    shell: &mut Shell,
    game: &mut GameWorld,
    frontend: &mut F,
) {
    let delta_time = shell.clock.tick(Instant::now());
    let transition = frame::advance_sim(game, delta_time);
    if let Some(policy) = input::CursorPolicy::for_transition(transition) {
        policy.apply(&shell.window);
    }

    match frame::quit_action(F::HOST, game.take_quit_request(), game.is_playing()) {
        QuitAction::Exit => {
            elwt.exit();
            return;
        }
        QuitAction::StopPlay => {
            game.set_playing(false);
            game.console()
                .borrow_mut()
                .info("Application.Quit() stops Play in the editor".to_string());
        }
        QuitAction::None => {}
    }
    drain_commands(shell, game);

    // Apply any video setting (resolution / vsync / fullscreen) a script wrote
    // this tick to the surface + window before drawing.
    settings::apply_pending(shell, game);

    let Some(surface) = frame::acquire_frame(elwt, &mut shell.renderer) else {
        return;
    };
    let target = surface
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    frontend.draw(shell, game, &target);
    surface.present();
}

/// Drain socket-delivered commands (#282) through the one live evaluator, so a
/// windowed playtest — editor or player — answers an external agent exactly like the
/// headless session does. Dev builds only.
#[cfg(feature = "dev")]
fn drain_commands(shell: &Shell, game: &GameWorld) {
    if let Some(channel) = shell.cmd_channel.as_ref() {
        channel.drain(game.script_manager(), &game.resources.console);
    }
}

/// No command channel in ship builds.
#[cfg(not(feature = "dev"))]
fn drain_commands(_shell: &Shell, _game: &GameWorld) {}
