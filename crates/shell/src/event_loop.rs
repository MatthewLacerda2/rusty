//! crates/shell/src/event_loop.rs — the winit application handler both frontends run under.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use super::{boot, input, pace, settings, Frontend, Shell};
use rusty_app::GameWorld;
use rusty_core::settings::video::VideoSettings;
use rusty_render::Renderer;

/// Everything a frontend decides before the window exists.
pub struct Launch<M> {
    /// The window title.
    pub title: String,
    pub app_id: Option<&'static str>,
    /// Whether the window opens filling the screen (the editor) or at its size.
    pub maximized: bool,
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

    pub(super) fn start(&mut self, window: Arc<Window>, renderer: Renderer) {
        let Some(launch) = self.launch.take() else {
            return;
        };
        window.set_title(&launch.title);
        let shell = Shell::new(window, renderer, &self.game, launch.video_defaults);
        #[cfg(feature = "dev")]
        let shell = Shell {
            frame_log: super::frame::stats::start_log(F::HOST, &self.game),
            ..shell
        };
        let frontend = (launch.frontend)(&shell, &self.game);
        self.running = Some((shell, frontend));
    }
}

pub fn run<F, M>(game: GameWorld, launch: Launch<M>)
where
    F: Frontend,
    M: FnOnce(&Shell, &GameWorld) -> F,
{
    let event_loop = EventLoop::new().expect("the OS refused an event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    pace::install_wake(&event_loop);
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
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(launch) = self.launch.as_ref() else {
            return;
        };
        let size = launch.video_defaults.resolution();
        let window = boot::create_window(
            event_loop,
            &launch.title,
            launch.app_id,
            size,
            launch.maximized,
        );
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
        if !matches!(event, WindowEvent::RedrawRequested) {
            shell.pacer.activity();
            shell.window.request_redraw();
        }
    }

    /// A [`rusty_core::wake`] ring: another thread left work for a frame.
    fn user_event(&mut self, _: &ActiveEventLoop, _: ()) {
        if let Some((shell, _)) = self.running.as_mut() {
            shell.pacer.activity();
        }
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Some((shell, frontend)) = self.running.as_ref() {
            let pace = frontend.pace(&self.game);
            pace::schedule(&shell.pacer, pace, &shell.window, event_loop);
        }
    }

    /// Quit boundary: persist the store however the loop is exiting (window close,
    /// `Application.Quit`). A no-op when the store is pathless.
    fn exiting(&mut self, _: &ActiveEventLoop) {
        if let Some((_, frontend)) = self.running.as_mut() {
            frontend.before_exit(&mut self.game);
        }
        if self.running.is_some() {
            settings::persist(&self.game);
        }
        self.game.resources.flush_storage();
    }
}
