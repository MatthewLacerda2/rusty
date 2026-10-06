//! src/shell/prelude.rs — a stage on the window before any project is open (#854).
//!
//! The editor started without `--project` shows a project picker first (Unity Hub's
//! model). There is no game yet (opening the project is what lets one boot), but
//! there must be a window, and winit allows one event loop per process. So the loop
//! starts in a [`Prelude`]: it owns the window and the renderer and draws on them
//! until it yields its output; `boot` turns that into the game and its [`Launch`],
//! and the same window and renderer go on to run the frontend as [`super::run`]
//! would have. The shell stays egui-free: the prelude brings its own UI.

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, DeviceId, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use super::event_loop::App;
use super::{boot, frame, Frontend, Launch, Shell};
use crate::app::GameWorld;
use crate::render::Renderer;

/// What runs on the window before the game exists (the editor's project picker).
pub trait Prelude {
    /// What the stage yields when it is done (the opened project).
    type Output;

    /// See every window event (the picker feeds egui).
    fn on_window_event(&mut self, window: &Window, event: &WindowEvent);

    /// Draw one frame onto the swapchain `target`; `Some` once the stage is done.
    fn draw(
        &mut self,
        window: &Window,
        renderer: &Renderer,
        target: &wgpu::TextureView,
    ) -> Option<Self::Output>;
}

/// Run a prelude on a window titled `title` and `size` big, then the game `boot`
/// builds from its output, until the window closes.
pub fn run_after<P, K, B, F, M>(title: &str, size: (u32, u32), make: K, boot: B)
where
    P: Prelude,
    K: FnOnce(&Arc<Window>, &Renderer) -> P,
    B: FnOnce(P::Output) -> (GameWorld, Launch<M>),
    F: Frontend,
    M: FnOnce(&Shell, &GameWorld) -> F,
{
    let event_loop = EventLoop::new().expect("the OS refused an event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = Staged {
        stage: Stage::Waiting {
            title: title.to_string(),
            size,
            make,
            boot,
        },
    };
    if let Err(err) = event_loop.run_app(&mut app) {
        eprintln!("[Engine] The event loop failed: {err}");
    }
}

enum Stage<P, K, B, F, M> {
    /// Before the first `resumed`: nothing open yet.
    Waiting {
        title: String,
        size: (u32, u32),
        make: K,
        boot: B,
    },
    /// The prelude drawing on the window.
    Prelude(Box<Open<P, B>>),
    /// The game, exactly as [`super::run`] runs it.
    Running(Box<App<F, M>>),
    /// Only while moving from one stage to the next.
    Moving,
}

/// The window and renderer a prelude draws on, and what boots the game after it.
struct Open<P, B> {
    window: Arc<Window>,
    renderer: Renderer,
    prelude: P,
    boot: B,
}

struct Staged<P, K, B, F, M> {
    stage: Stage<P, K, B, F, M>,
}

impl<P, K, B, F, M> Staged<P, K, B, F, M>
where
    P: Prelude,
    K: FnOnce(&Arc<Window>, &Renderer) -> P,
    B: FnOnce(P::Output) -> (GameWorld, Launch<M>),
    F: Frontend,
    M: FnOnce(&Shell, &GameWorld) -> F,
{
    /// Draw a prelude frame; when it yields, boot the game on the same window.
    fn prelude_frame(&mut self) {
        let Stage::Prelude(open) = &mut self.stage else {
            return;
        };
        let Open {
            window,
            renderer,
            prelude,
            ..
        } = &mut **open;
        let Some(surface) = frame::acquire_frame(renderer) else {
            return;
        };
        let target = surface.texture.create_view(&Default::default());
        let output = prelude.draw(window, renderer, &target);
        renderer.queue.present(surface);
        let Some(output) = output else {
            return;
        };
        let Stage::Prelude(open) = std::mem::replace(&mut self.stage, Stage::Moving) else {
            return;
        };
        let Open {
            window,
            renderer,
            prelude,
            boot,
        } = *open;
        drop(prelude);
        let (game, launch) = boot(output);
        let mut app = App::new(game, launch);
        app.start(window, renderer);
        self.stage = Stage::Running(Box::new(app));
    }
}

impl<P, K, B, F, M> ApplicationHandler for Staged<P, K, B, F, M>
where
    P: Prelude,
    K: FnOnce(&Arc<Window>, &Renderer) -> P,
    B: FnOnce(P::Output) -> (GameWorld, Launch<M>),
    F: Frontend,
    M: FnOnce(&Shell, &GameWorld) -> F,
{
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        match std::mem::replace(&mut self.stage, Stage::Moving) {
            Stage::Waiting {
                title,
                size,
                make,
                boot,
            } => {
                let window = boot::create_window(event_loop, &title, size);
                let renderer = boot::init_renderer(&window);
                let prelude = make(&window, &renderer);
                self.stage = Stage::Prelude(Box::new(Open {
                    window,
                    renderer,
                    prelude,
                    boot,
                }));
            }
            Stage::Running(mut app) => {
                app.resumed(event_loop);
                self.stage = Stage::Running(app);
            }
            other => self.stage = other,
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let (window, renderer, prelude) = match &mut self.stage {
            Stage::Running(app) => return app.window_event(event_loop, id, event),
            Stage::Prelude(open) if id == open.window.id() => {
                let Open {
                    window,
                    renderer,
                    prelude,
                    ..
                } = &mut **open;
                (window, renderer, prelude)
            }
            _ => return,
        };
        prelude.on_window_event(window, &event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => renderer.resize(size),
            WindowEvent::ScaleFactorChanged { .. } => renderer.resize(window.inner_size()),
            WindowEvent::RedrawRequested => self.prelude_frame(),
            _ => {}
        }
    }

    fn device_event(&mut self, event_loop: &ActiveEventLoop, id: DeviceId, event: DeviceEvent) {
        if let Stage::Running(app) = &mut self.stage {
            app.device_event(event_loop, id, event);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match &mut self.stage {
            Stage::Running(app) => app.about_to_wait(event_loop),
            Stage::Prelude(open) => open.window.request_redraw(),
            _ => {}
        }
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        if let Stage::Running(app) = &mut self.stage {
            app.exiting(event_loop);
        }
    }
}
