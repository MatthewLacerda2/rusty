//! src/shell/event_loop.rs — the winit application handler both frontends run under.
//!
//! winit 0.30 hands events to an [`ApplicationHandler`] and only lets a window be
//! created once the loop is running (`resumed`), so the window, the renderer, the
//! [`Shell`] and the frontend are built there, from a [`Launch`] the frontend's
//! `launch()` prepared. Every event after that is routed exactly as before: window
//! events through the frontend and [`super::handle_window_event`], raw mouse motion
//! to the frontend (the editor's Scene-view look, #745) and to the game while it has
//! input, a redraw request each time the loop goes idle.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use super::{boot, input, settings, Frontend, Shell};
use crate::app::GameWorld;
use crate::core::video::VideoSettings;
use crate::render::Renderer;

/// Everything a frontend decides before the window exists.
pub struct Launch<M> {
    /// The window title.
    pub title: String,
    /// What a first launch (no saved video settings) gets.
    pub video_defaults: VideoSettings,
    /// Builds the frontend once the shell is up.
    pub frontend: M,
}

/// The loop's state: the launch until the shell starts, then the running shell.
pub(super) struct App<F, M> {
    game: GameWorld,
    launch: Option<Launch<M>>,
    running: Option<(Shell, F)>,
}

impl<F, M> App<F, M>
where
    F: Frontend,
    M: FnOnce(&Shell, &GameWorld) -> F,
{
    pub(super) fn new(game: GameWorld, launch: Launch<M>) -> Self {
        Self {
            game,
            launch: Some(launch),
            running: None,
        }
    }

    /// Boot the shell and the frontend on an open window: the one `resumed` made, or
    /// the one a [`super::prelude`] stage ran in (it gets the launch's title).
    pub(super) fn start(&mut self, window: Arc<Window>, renderer: Renderer) {
        let Some(launch) = self.launch.take() else {
            return;
        };
        window.set_title(&launch.title);
        let shell = Shell::new(window, renderer, &self.game, launch.video_defaults);
        let frontend = (launch.frontend)(&shell, &self.game);
        self.running = Some((shell, frontend));
    }
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
    let mut app = App::new(game, launch);
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
        let Some(launch) = self.launch.as_ref() else {
            return;
        };
        let size = launch.video_defaults.resolution();
        let window = boot::create_window(event_loop, &launch.title, size);
        let renderer = boot::init_renderer(&window);
        self.start(window, renderer);
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
        let Some((shell, frontend)) = self.running.as_mut() else {
            return;
        };
        if let DeviceEvent::MouseMotion { delta } = event {
            if !shell.window_focused {
                return;
            }
            frontend.on_mouse_motion(delta);
            if frontend.game_has_input(&self.game) {
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
