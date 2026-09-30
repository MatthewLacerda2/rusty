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

pub mod audio;
pub mod boot;
pub mod frame;
pub mod input;
pub mod player;
pub mod settings;

#[cfg(feature = "editor")]
pub mod editor;

use std::sync::Arc;
use std::time::Instant;

use winit::event::{DeviceEvent, ElementState, Event, KeyEvent, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::app::{GameWorld, PlayTransition};
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
    /// What the OS cursor was last set to (#416).
    pub cursor: input::CursorPolicy,
    /// The OS gamepad backend (#471); `None` where the platform has none.
    pub pad_source: Option<input::pad_source::GilrsSource>,
    /// Pad slots and the pad state last written into the sim.
    pub pads: input::pads::PadPump,
    /// Whether the window has OS focus: raw mouse motion arrives even when it does
    /// not, and must not reach the game then.
    pub window_focused: bool,
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

    /// Whether input (keys, mouse, text, the cursor request) reaches the game right
    /// now. The player: always. The editor: only in Play with the Game view focused.
    fn game_has_input(&self, _game: &GameWorld) -> bool {
        true
    }

    /// Where the game view sits in the window, for mapping the pointer into
    /// game-view pixels. Default: the whole window.
    fn game_view(&self, shell: &Shell) -> input::GameViewRect {
        let config = &shell.renderer.config;
        input::GameViewRect::full_window(config.width, config.height)
    }

    /// The sim entered or left Play this frame (the editor focuses the Game view).
    fn on_play_transition(&mut self, _game: &mut GameWorld, _transition: PlayTransition) {}

    /// A physical key changed state, after the shell wrote it into the sim.
    fn on_key(&mut self, _game: &mut GameWorld, _key: KeyCode, _pressed: bool) {}

    /// Draw this frame into the swapchain `target` — the default view of `frame`,
    /// the swapchain texture itself (for a frontend needing another view of it, like
    /// the player's UI pass, #418).
    fn draw(
        &mut self,
        shell: &mut Shell,
        game: &mut GameWorld,
        frame: &wgpu::Texture,
        target: &wgpu::TextureView,
    );
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
            Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { delta },
                ..
            } if shell.window_focused && frontend.game_has_input(&game) => {
                input::write_mouse_motion(delta, &game);
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
        WindowEvent::Focused(focused) => {
            shell.window_focused = *focused;
            if !focused {
                // Key-ups never arrive for keys released while unfocused.
                game.input().borrow_mut().release_all();
            }
        }
        WindowEvent::KeyboardInput {
            event:
                KeyEvent {
                    physical_key: PhysicalKey::Code(key),
                    state,
                    text,
                    ..
                },
            ..
        } => {
            let pressed = *state == ElementState::Pressed;
            if frontend.game_has_input(game) {
                input::write_key(*key, pressed, game, &shell.keymap);
                if let (true, Some(text)) = (pressed, text) {
                    input::write_text(text, game);
                }
            }
            frontend.on_key(game, *key, pressed);
        }
        // Tracked even without input, so the pointer is right the moment it arrives.
        WindowEvent::CursorMoved { position, .. } => {
            let view = frontend.game_view(shell);
            input::write_cursor_moved((position.x, position.y), &view, game);
        }
        WindowEvent::MouseInput { state, button, .. } if frontend.game_has_input(game) => {
            input::write_mouse_button(*button, *state, game, &shell.keymap);
        }
        WindowEvent::MouseWheel { delta, .. } if frontend.game_has_input(game) => {
            input::write_wheel(*delta, game);
        }
        WindowEvent::RedrawRequested => run_frame(elwt, shell, game, frontend),
        _ => {}
    }
}

/// One frame: advance the sim, apply the audio mix, act on play transitions and a
/// quit request, apply scripted video settings, then let the frontend draw onto the
/// swapchain.
fn run_frame<F: Frontend>(
    elwt: &EventLoopWindowTarget<()>,
    shell: &mut Shell,
    game: &mut GameWorld,
    frontend: &mut F,
) {
    let delta_time = shell.clock.tick(Instant::now());
    // Like raw mouse motion, pads keep reporting while the window is unfocused.
    let pads_live = shell.window_focused && frontend.game_has_input(game);
    pump_pads(shell, game, pads_live);
    let transition = frame::advance_sim(game, delta_time);
    audio::apply_mix(game);
    frontend.on_play_transition(game, transition);
    // The game's cursor request (Play defaults to locked + hidden), while it has input.
    let requested = game.input().borrow().cursor();
    let cursor = input::CursorPolicy::effective(requested, frontend.game_has_input(game));
    shell.cursor.sync(cursor, &shell.window);

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
    frontend.draw(shell, game, &surface.texture, &target);
    surface.present();
}

/// Write this frame's gamepad snapshot into the sim, before it ticks.
fn pump_pads(shell: &mut Shell, game: &GameWorld, has_input: bool) {
    if let Some(source) = shell.pad_source.as_mut() {
        let mut input = game.input().borrow_mut();
        shell
            .pads
            .pump(source, &mut input, &shell.keymap, has_input);
    }
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
