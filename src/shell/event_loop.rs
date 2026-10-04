//! src/shell/event_loop.rs — the winit application handler both frontends run under.
//!
//! winit 0.30 hands events to an [`ApplicationHandler`] and only lets a window be
//! created once the loop is running (`resumed`), so the window, the renderer, the
//! [`Shell`] and the frontend are built there, from a [`Launch`] the frontend's
//! `launch()` prepared. Every event after that is routed exactly as before: window
//! events through the frontend and [`super::handle_window_event`], raw mouse motion
//! to the game while it has input, a redraw request each time the loop goes idle.

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

use super::{boot, input, settings, Frontend, Shell};
use crate::app::GameWorld;
use crate::core::video::VideoSettings;

/// Everything a frontend decides before the window exists.
pub struct Launch<M> {
    /// The window title.
    pub title: String,
    /// What a first launch (no saved video settings) gets.
    pub video_defaults: VideoSettings,
    /// Builds the frontend once the shell is up.
    pub frontend: M,
}

/// The loop's state: the launch until the first `resumed`, then the running shell.
struct App<F, M> {
    game: GameWorld,
    launch: Option<Launch<M>>,
    running: Option<(Shell, F)>,
}

/// Run the event loop until the window closes or the game quits. Persists the
/// settings and flushes `Storage` however the loop exits.
pub fn run<F, M>(game: GameWorld, launch: Launch<M>)
where
    F: Frontend,
    M: FnOnce(&Shell, &GameWorld) -> F,
{
    let event_loop = EventLoop::new().expect("the OS refused an event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        game,
        launch: Some(launch),
        running: None,
    };
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("[Engine] The event loop failed: {err}");
    }
}

impl<F, M> ApplicationHandler for App<F, M>
where
    F: Frontend,
    M: FnOnce(&Shell, &GameWorld) -> F,
{
    /// The first resume opens the window and boots the shell; desktop platforms
    /// resume once, so later ones (mobile) keep the shell they have.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(launch) = self.launch.take() else {
            return;
        };
        let size = launch.video_defaults.resolution();
        let window = boot::create_window(event_loop, &launch.title, size);
        let renderer = boot::init_renderer(&window);
        let shell = Shell::new(window, renderer, &self.game, launch.video_defaults);
        let frontend = (launch.frontend)(&shell, &self.game);
        self.running = Some((shell, frontend));
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some((shell, frontend)) = self.running.as_mut() else {
            return;
        };
        if id != shell.window.id() {
            return;
        }
        frontend.on_window_event(shell, &event);
        super::handle_window_event(&event, event_loop, shell, &mut self.game, frontend);
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        let Some((shell, frontend)) = self.running.as_ref() else {
            return;
        };
        if let DeviceEvent::MouseMotion { delta } = event {
            if shell.window_focused && frontend.game_has_input(&self.game) {
                input::write_mouse_motion(delta, &self.game);
            }
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some((shell, _)) = self.running.as_ref() {
            shell.window.request_redraw();
        }
    }

    /// Quit boundary: persist the store however the loop is exiting (window close,
    /// `Application.Quit`). A no-op when the store is pathless.
    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some((shell, _)) = self.running.as_ref() {
            settings::persist(shell, &self.game);
        }
        self.game.resources.flush_storage();
    }
}
