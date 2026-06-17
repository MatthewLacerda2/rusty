use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use rusty::app::{GameWorld, PlayTransition};
use rusty::core::input::InputState;
use rusty::core::keymap::{Keymap, KEYBINDINGS_KEY, KEYBINDINGS_NAMESPACE};
use rusty::editor::EditorUi;
use rusty::navigation::NavigationGraph;
use rusty::render::Renderer;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

/// The stable physical-key name (uppercase) the engine speaks for a winit
/// `KeyCode`, or `None` for keys the sim has no logical name for. This is the
/// hardware-side identity name; the [`Keymap`] then remaps it to a logical key
/// before the simulation ever sees it.
fn physical_key_name(key: KeyCode) -> Option<&'static str> {
    Some(match key {
        KeyCode::KeyW => "W",
        KeyCode::KeyA => "A",
        KeyCode::KeyS => "S",
        KeyCode::KeyD => "D",
        KeyCode::ArrowUp => "UP",
        KeyCode::ArrowDown => "DOWN",
        KeyCode::ArrowLeft => "LEFT",
        KeyCode::ArrowRight => "RIGHT",
        // The shoot button is just the SPACE key; the player controller script
        // edge-detects it. No engine-side "shoot" field — gameplay reads the key
        // like any other.
        KeyCode::Space => "SPACE",
        _ => return None,
    })
}

/// Front-end (window/GPU) state owned by `main` and threaded through the event
/// loop. Kept distinct from the window-agnostic [`GameWorld`] simulation.
struct Frontend {
    renderer: Renderer,
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
    editor_ui: EditorUi,
    last_frame_time: Instant,
    frame_count: u32,
    fps: f32,
    last_fps_update: Instant,
}

fn main() {
    env_logger::init();
    println!("[Engine] Starting rusty 3D engine...");

    seed_project_workspace();

    let event_loop = EventLoop::new().unwrap();
    let window = create_window(&event_loop);
    let (mut frontend, boot_scene_path) = init_frontend(&window);
    let mut game = init_game(boot_scene_path.clone());
    frontend.editor_ui.current_scene_path = Some(boot_scene_path);
    let keymap = load_keymap(&game);

    let _ = event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);
        match event {
            Event::WindowEvent {
                window_id,
                ref event,
            } if window_id == window.id() => {
                // Always feed events to egui so the header panel with Play/Stop buttons works
                let _ = frontend.egui_winit.on_window_event(&window, event);
                handle_window_event(event, elwt, &window, &mut frontend, &mut game, &keymap);
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            // Quit boundary: persist the store however the loop is exiting (window
            // close, menu quit, surface OOM). A no-op when the store is pathless.
            Event::LoopExiting => {
                game.resources.flush_storage();
            }
            _ => {}
        }
    });
}

/// Set up the git-ignored project folder structure and seed bundled scripts.
fn seed_project_workspace() {
    // Setup local asset structure inside git-ignored project folder
    std::fs::create_dir_all("project/assets/textures").ok();
    std::fs::create_dir_all("project/assets/models").ok();
    std::fs::create_dir_all("project/assets/audio").ok();
    std::fs::create_dir_all("project/scenes").ok();

    // Seed the bundled default scripts (player_controller.lua, bot.lua) into the
    // gitignored project workspace, the same way the default scene is seeded. These
    // are GAME scripts that ship with the engine; the play loop runs no gameplay.
    rusty::scene::seed_default_scripts();
}

/// Build the editor window.
fn create_window(event_loop: &EventLoop<()>) -> Arc<winit::window::Window> {
    Arc::new(
        WindowBuilder::new()
            .with_title("Rusty 3D Game Engine & Editor")
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720))
            .build(event_loop)
            .unwrap(),
    )
}

/// Initialize the renderer + egui integration + front-end timing state. Also
/// seeds the boot scene file and returns its path (the scene is loaded by
/// [`init_game`]). Exits the process if no compatible GPU is available.
fn init_frontend(window: &Arc<winit::window::Window>) -> (Frontend, String) {
    let renderer = match pollster::block_on(Renderer::new(Arc::clone(window))) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("[Engine] Could not initialize the renderer: {err}");
            eprintln!("[Engine] No compatible GPU is available — exiting.");
            std::process::exit(1);
        }
    };
    let egui_ctx = egui::Context::default();
    let egui_winit = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        window,
        Some(window.scale_factor() as f32),
        None,
    );
    let egui_renderer = egui_wgpu::Renderer::new(&renderer.device, renderer.config.format, None, 1);

    let frontend = Frontend {
        renderer,
        egui_ctx,
        egui_winit,
        egui_renderer,
        editor_ui: EditorUi::new(),
        last_frame_time: Instant::now(),
        frame_count: 0,
        fps: 60.0,
        last_fps_update: Instant::now(),
    };
    (frontend, rusty::scene::seed_default_scene())
}

/// Build the simulation: shared engine state, the loaded boot scene, baked nav,
/// and the persistent store bound to its file.
fn init_game(boot_scene_path: String) -> GameWorld {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));

    // The procedural bot-chase demo lives in the tracked `default.scene`, seeded
    // into the gitignored `project/scenes/` on boot and loaded as the boot scene.
    {
        let mut s = scene.borrow_mut();
        console
            .borrow_mut()
            .info(format!("Loading scene from {}...", boot_scene_path));
        if let Err(err) = s.load_from_file(&boot_scene_path) {
            console
                .borrow_mut()
                .error(format!("Failed to load default scene: {}", err));
        }
        nav.borrow_mut().bake(&s);
    }

    let game = GameWorld::new(scene, input, nav, console);

    // Bind the persistent store to its file and load it once — this boundary read
    // is a sim input. Writes flush at Stop and on loop exit. The harness leaves
    // the store pathless so headless runs never read this file.
    if let Err(err) = game
        .resources
        .storage
        .borrow_mut()
        .open(rusty::core::storage::DEFAULT_STORAGE_PATH)
    {
        game.console()
            .borrow_mut()
            .error(format!("Failed to load storage: {}", err));
    }
    game
}

/// Load the physical→logical key remap (issue #88) from the persistent store.
/// A missing/empty binding blob means the identity (default) mapping. The
/// harness and bot-players inject logical keys directly and bypass this.
fn load_keymap(game: &GameWorld) -> Keymap {
    match game
        .resources
        .storage
        .borrow()
        .get(KEYBINDINGS_NAMESPACE, KEYBINDINGS_KEY)
    {
        Some(blob) => Keymap::from_json(&blob),
        None => Keymap::new(),
    }
}

/// Dispatch a single window event for our window.
fn handle_window_event(
    event: &WindowEvent,
    elwt: &winit::event_loop::EventLoopWindowTarget<()>,
    window: &Arc<winit::window::Window>,
    frontend: &mut Frontend,
    game: &mut GameWorld,
    keymap: &Keymap,
) {
    match event {
        WindowEvent::CloseRequested => elwt.exit(),
        WindowEvent::Resized(physical_size) => {
            frontend.renderer.resize(*physical_size);
        }
        WindowEvent::ScaleFactorChanged { .. } => {
            // A DPI / display change (e.g. dragging between Retina and
            // non-Retina monitors) can invalidate the swapchain without a
            // Resized event. Reconfigure to the window's current inner size
            // so the surface stays valid.
            frontend.renderer.resize(window.inner_size());
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
            handle_keyboard_input(*key, pressed, game, keymap);
        }
        WindowEvent::CursorMoved { position, .. } => {
            game.input().borrow_mut().mouse_position = (position.x, position.y);
        }
        WindowEvent::RedrawRequested => {
            render_frame(elwt, window, frontend, game);
        }
        _ => {}
    }
}

/// Translate a physical key to its logical name and write it into the sim; ESC
/// in PlayMode unlocks the cursor (a platform-side transition).
fn handle_keyboard_input(key: KeyCode, pressed: bool, game: &mut GameWorld, keymap: &Keymap) {
    // Translate the physical key to its hardware name, remap it to a logical
    // key via the keymap, then write logical state.
    if let Some(physical) = physical_key_name(key) {
        let logical = keymap.resolve(physical);
        game.input().borrow_mut().set_key_state(&logical, pressed);
    }
    // Hit ESC in PlayMode to unlock cursor (platform-side transition)
    if key == KeyCode::Escape && pressed && game.is_playing() {
        game.set_playing(false);
        game.console()
            .borrow_mut()
            .info("Unlocked cursor, entering EditorMode".to_string());
    }
}

/// Advance one frame: timing, sim tick, cursor transitions, and the GPU render.
fn render_frame(
    elwt: &winit::event_loop::EventLoopWindowTarget<()>,
    window: &Arc<winit::window::Window>,
    frontend: &mut Frontend,
    game: &mut GameWorld,
) {
    let (delta_time, current_frame_duration) = tick_timing(frontend);

    // Advance the simulation (decoupled from window + GPU).
    let transition = game.tick(delta_time);
    apply_play_transition(transition, window);

    // --- GPU RENDER TICK ---
    let frame = match acquire_frame(elwt, &mut frontend.renderer) {
        Some(f) => f,
        None => return,
    };
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    // Render the 3D scene (Forward unlit/lit + gizmos line drawers)
    {
        let s = game.scene().borrow();
        let cam = game.camera().borrow();
        frontend.renderer.render(
            &s,
            &cam,
            &view,
            !game.is_playing(),
            game.pathfinding_points(),
        );
    }

    render_egui_overlay(window, frontend, game, &view, current_frame_duration);
    frame.present();
}

/// Update front-end timing/FPS counters, returning (delta_time, frame_ms).
fn tick_timing(frontend: &mut Frontend) -> (f32, f32) {
    let now = Instant::now();
    let delta_time = now.duration_since(frontend.last_frame_time).as_secs_f32();
    frontend.last_frame_time = now;

    frontend.frame_count += 1;
    if now.duration_since(frontend.last_fps_update) >= Duration::from_secs(1) {
        frontend.fps = frontend.frame_count as f32;
        frontend.frame_count = 0;
        frontend.last_fps_update = now;
    }
    (delta_time, delta_time * 1000.0)
}

/// React to a play-mode transition by grabbing/releasing the cursor.
fn apply_play_transition(transition: PlayTransition, window: &Arc<winit::window::Window>) {
    match transition {
        PlayTransition::Entered => {
            window
                .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                .ok();
            window.set_cursor_visible(false);
        }
        PlayTransition::Exited => {
            window
                .set_cursor_grab(winit::window::CursorGrabMode::None)
                .ok();
            window.set_cursor_visible(true);
        }
        PlayTransition::None => {}
    }
}

/// Acquire the next swapchain frame, recovering from surface loss. Returns
/// `None` when the frame should be skipped this redraw (recovered or exiting).
fn acquire_frame(
    elwt: &winit::event_loop::EventLoopWindowTarget<()>,
    renderer: &mut Renderer,
) -> Option<wgpu::SurfaceTexture> {
    let surface_frame = {
        let window_surface = renderer
            .surface
            .as_ref()
            .expect("windowed renderer must have a surface");
        window_surface.get_current_texture()
    };
    // Variant-specific surface recovery. `Lost`/`Outdated` only recover by
    // reconfiguring the surface — which never happens on the per-frame path
    // otherwise (only on Resized), so a surface lost without a resize event
    // would log forever. Reuse `resize` (reconfigures, guards zero-size).
    match surface_frame {
        Ok(f) => Some(f),
        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
            renderer.resize(renderer.size);
            None
        }
        Err(wgpu::SurfaceError::OutOfMemory) => {
            eprintln!("[WGPU] Surface out of memory — exiting");
            elwt.exit();
            None
        }
        Err(wgpu::SurfaceError::Timeout) => None,
    }
}

/// Draw the editor dashboard + console overlay and paint it over `view`.
/// Header always visible; side panels only in editor mode.
fn render_egui_overlay(
    window: &Arc<winit::window::Window>,
    frontend: &mut Frontend,
    game: &mut GameWorld,
    view: &wgpu::TextureView,
    current_frame_duration: f32,
) {
    let raw_input = frontend.egui_winit.take_egui_input(window);
    frontend.egui_ctx.begin_frame(raw_input);

    draw_editor_dashboard(frontend, game, current_frame_duration);
    drain_repl(frontend, game);

    let full_output = frontend.egui_ctx.end_frame();
    let paint_jobs = frontend
        .egui_ctx
        .tessellate(full_output.shapes, full_output.pixels_per_point);

    let screen_descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [
            frontend.renderer.config.width,
            frontend.renderer.config.height,
        ],
        pixels_per_point: window.scale_factor() as f32,
    };

    for (id, image_delta) in &full_output.textures_delta.set {
        frontend.egui_renderer.update_texture(
            &frontend.renderer.device,
            &frontend.renderer.queue,
            *id,
            image_delta,
        );
    }

    paint_egui(frontend, view, &paint_jobs, &screen_descriptor);

    for id in &full_output.textures_delta.free {
        frontend.egui_renderer.free_texture(id);
    }
}

/// Run the editor UI for one frame and apply its post-FX quality selection.
fn draw_editor_dashboard(
    frontend: &mut Frontend,
    game: &mut GameWorld,
    current_frame_duration: f32,
) {
    let mut s = game.world.scene.borrow_mut();
    let mut c = game.resources.console.borrow_mut();
    let mut n = game.resources.nav.borrow_mut();
    frontend.editor_ui.draw(
        &frontend.egui_ctx,
        &mut s,
        &mut c,
        &mut n,
        &mut game.resources.is_playing,
        frontend.fps,
        current_frame_duration,
    );
    if frontend.editor_ui.is_dirty {
        frontend.renderer.shadow_renderer.is_static_cached = false;
        frontend.editor_ui.is_dirty = false;
    }
    // Apply the selected post-FX scalability tier. The editor dropdown and
    // `Graphics.SetQuality` (script) both feed one shared cell: reflect a
    // script-driven change back into the dropdown, push the editor's choice
    // into the cell, then hand the result to the renderer (`set_quality`
    // guards the bloom realloc).
    let cell = game.script_manager().quality_cell();
    let scripted = *cell.borrow();
    if scripted != frontend.editor_ui.quality_preset
        && frontend.editor_ui.quality_preset == frontend.renderer.quality
    {
        frontend.editor_ui.quality_preset = scripted;
    }
    *cell.borrow_mut() = frontend.editor_ui.quality_preset;
    frontend
        .renderer
        .set_quality(frontend.editor_ui.quality_preset);
}

/// Drain a submitted REPL line through the single live evaluator. Dev builds
/// only — the console input line and the harness share `evaluate_line`.
#[cfg(feature = "dev")]
fn drain_repl(frontend: &mut Frontend, game: &mut GameWorld) {
    if let Some(line) = frontend.editor_ui.pending_repl.take() {
        let mut c = game.resources.console.borrow_mut();
        let _ = rusty::dev::console::evaluate_line(game.script_manager(), &mut c, &line);
    }
}

/// No-op REPL drain in ship builds (the console/REPL is dev-only).
#[cfg(not(feature = "dev"))]
fn drain_repl(_frontend: &mut Frontend, _game: &mut GameWorld) {}

/// Encode + submit the egui paint jobs as a load-pass over `view`.
fn paint_egui(
    frontend: &mut Frontend,
    view: &wgpu::TextureView,
    paint_jobs: &[egui::ClippedPrimitive],
    screen_descriptor: &egui_wgpu::ScreenDescriptor,
) {
    let mut encoder =
        frontend
            .renderer
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Egui Render Encoder"),
            });

    frontend.egui_renderer.update_buffers(
        &frontend.renderer.device,
        &frontend.renderer.queue,
        &mut encoder,
        paint_jobs,
        screen_descriptor,
    );

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Egui Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        frontend
            .egui_renderer
            .render(&mut render_pass, paint_jobs, screen_descriptor);
    }

    frontend
        .renderer
        .queue
        .submit(std::iter::once(encoder.finish()));
}
