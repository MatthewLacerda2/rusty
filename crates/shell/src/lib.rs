#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "platform layer, exempt from the sim's clock ban (#757): the frame loop measures real time to produce the sim's dt"
)]

pub mod boot;
mod event_loop;
mod focus;
pub mod frame;
pub mod input;
pub mod pace;
pub mod player;
mod prelude;
pub mod settings;

#[cfg(feature = "editor")]
pub mod editor;

use std::sync::Arc;
use std::time::Instant;

use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};

use rusty_app::{GameWorld, PlayTransition};
use rusty_core::input::keymap::Keymap;
use rusty_core::settings::video::VideoSettings;
use rusty_render::Renderer;

pub use event_loop::{run, Launch};
use frame::{FrameClock, Host, QuitAction, Step};
pub use prelude::{run_after, Prelude};

pub struct Shell {
    pub window: Arc<winit::window::Window>,
    pub renderer: Renderer,
    pub clock: FrameClock,
    pub ticks: frame::FixedTicks,
    pub pacer: pace::Pacer,
    pub keymap: Keymap,
    /// In effect on the surface and window, so the per-frame apply redoes only what changed.
    pub applied_video: VideoSettings,
    pub cursor: input::CursorPolicy,
    pub pad_source: Option<input::pad_source::GilrsSource>,
    pub pads: input::pads::PadPump,
    pub clipboard: Option<input::clipboard_source::OsClipboard>,
    pub modifiers: winit::keyboard::ModifiersState,
    /// Raw mouse motion and pads report without OS focus too; they reach the game only with it.
    pub window_focused: bool,
    #[cfg(feature = "dev")]
    pub cmd_channel: Option<rusty_dev::command_channel::CommandChannel>,
    #[cfg(feature = "dev")]
    pub frame_log: Option<rusty_dev::frame_log::FrameLog>,
}

/// No plugin trait: one compile-time implementation per binary, so the loop is written once.
pub trait Frontend {
    const HOST: Host;

    fn on_window_event(&mut self, _shell: &Shell, _event: &WindowEvent) {}

    fn on_mouse_motion(&mut self, _delta: (f64, f64)) {}

    fn game_has_input(&self, _game: &GameWorld) -> bool {
        true
    }

    fn pace(&self, _game: &GameWorld) -> pace::Pace {
        pace::Pace::Continuous
    }

    fn game_view(&self, shell: &Shell) -> input::GameViewRect {
        let config = &shell.renderer.config;
        input::GameViewRect::full_window(config.width, config.height)
    }

    fn advance(&mut self, game: &mut GameWorld, step: Step<'_>, raw: f32) -> PlayTransition {
        frame::advance_sim(game, step, raw)
    }

    /// Runs before the quit boundary persists the store.
    fn before_exit(&mut self, _game: &mut GameWorld) {}

    fn on_play_transition(&mut self, _game: &mut GameWorld, _transition: PlayTransition) {}

    fn cursor(&self, game: &GameWorld) -> rusty_core::input::CursorState {
        input::CursorPolicy::effective(game.input().borrow().cursor(), self.game_has_input(game))
    }

    fn on_key(&mut self, _game: &mut GameWorld, _key: KeyCode, _pressed: bool) {}

    /// The swapchain refused a frame: do `draw`'s work besides painting, so command-channel
    /// work never waits on a redraw.
    fn undrawn(&mut self, _shell: &mut Shell, _game: &mut GameWorld) {}

    fn draw(
        &mut self,
        shell: &mut Shell,
        game: &mut GameWorld,
        frame: &wgpu::Texture,
        target: &wgpu::TextureView,
    );
}

pub(crate) fn handle_window_event<F: Frontend>(
    event: &WindowEvent,
    elwt: &ActiveEventLoop,
    shell: &mut Shell,
    game: &mut GameWorld,
    frontend: &mut F,
) {
    match event {
        WindowEvent::CloseRequested => elwt.exit(),
        WindowEvent::Resized(size) => shell.renderer.resize(boot::pixel_size(*size)),
        // A DPI change (onto a Retina monitor) can invalidate the swapchain at one size.
        WindowEvent::ScaleFactorChanged { .. } => {
            shell
                .renderer
                .resize(boot::pixel_size(shell.window.inner_size()));
        }
        WindowEvent::Focused(focused) => focus::changed(shell, game, *focused, F::HOST),
        WindowEvent::ModifiersChanged(modifiers) => shell.modifiers = modifiers.state(),
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
                if pressed && input::clipboard::is_shortcut(shell.modifiers) {
                    capture_clipboard(shell, game);
                }
                input::write_key(*key, pressed, game, &shell.keymap);
                if let (true, Some(text)) = (pressed, text) {
                    input::write_text(text, game);
                }
            }
            frontend.on_key(game, *key, pressed);
        }
        // Unguarded by input, so the pointer is right the moment input arrives.
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

fn run_frame<F: Frontend>(
    elwt: &ActiveEventLoop,
    shell: &mut Shell,
    game: &mut GameWorld,
    frontend: &mut F,
) {
    let delta_time = frontend
        .pace(game)
        .frame_step(shell.clock.tick(Instant::now()));
    frame::stats::started(&shell.clock, game);
    let pads_live = shell.window_focused && frontend.game_has_input(game);
    pump_pads(shell, game, pads_live);
    let step = Step::of(&mut shell.ticks, &mut shell.renderer.draw_poses);
    let transition = frontend.advance(game, step, delta_time);
    frame::load::warm::warm_load(&mut shell.renderer, game);
    if let Some(clipboard) = shell.clipboard.as_mut() {
        input::clipboard::apply_request(clipboard, &mut game.input().borrow_mut());
    }
    frontend.on_play_transition(game, transition);
    shell.pacer.settle_if(transition != PlayTransition::None);
    shell.cursor.sync(frontend.cursor(game), &shell.window);

    match frame::quit_action(F::HOST, game.take_quit_request(), game.play_requested()) {
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
    frame::drain_commands(shell, game);
    rusty_logging::forward(game.console());

    settings::apply_pending(shell, game);

    let Some(surface) = frame::acquire_frame(&mut shell.renderer) else {
        frontend.undrawn(shell, game);
        frame::stats::undrawn(shell, game);
        shell.pacer.skipped(Instant::now());
        return;
    };
    let target = surface
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    frame::stats::measured(shell, game, |s, g| {
        frontend.draw(s, g, &surface.texture, &target)
    });
    shell.renderer.queue.present(surface);
    shell.pacer.drew();
}

fn pump_pads(shell: &mut Shell, game: &GameWorld, has_input: bool) {
    if let Some(source) = shell.pad_source.as_mut() {
        let mut input = game.input().borrow_mut();
        shell
            .pads
            .pump(source, &mut input, &shell.keymap, has_input);
    }
}

fn capture_clipboard(shell: &mut Shell, game: &GameWorld) {
    if let Some(clipboard) = shell.clipboard.as_mut() {
        input::clipboard::capture(clipboard, &mut game.input().borrow_mut());
    }
}
