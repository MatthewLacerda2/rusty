mod render_loop;

use std::sync::Arc;
use std::time::Instant;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use render_loop::{run_frame, FrameTiming};
use rusty::app::GameWorld;
use rusty::core::input::InputState;
use rusty::editor::EditorUi;
use rusty::navigation::NavigationGraph;
use rusty::render::Renderer;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

fn main() {
    env_logger::init();
    println!("[Engine] Starting rusty 3D engine...");

    // 1. Setup local asset structure inside git-ignored project folder
    std::fs::create_dir_all("project/assets/textures").ok();
    std::fs::create_dir_all("project/assets/models").ok();
    std::fs::create_dir_all("project/assets/audio").ok();
    std::fs::create_dir_all("project/scenes").ok();

    // Seed the bundled default scripts (player_controller.lua, bot.lua) into the
    // gitignored project workspace, the same way the default scene is seeded. These
    // are GAME scripts that ship with the engine; the play loop runs no gameplay.
    rusty::scene::seed_default_scripts();

    // 2. Initialize Window Event Loop
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Rusty 3D Game Engine & Editor")
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720))
            .build(&event_loop)
            .unwrap(),
    );

    // 3. Initialize Core Render Engine & egui Context
    let mut renderer = match pollster::block_on(Renderer::new(Arc::clone(&window))) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("[Engine] Could not initialize the renderer: {err}");
            eprintln!("[Engine] No compatible GPU is available — exiting.");
            std::process::exit(1);
        }
    };
    let egui_ctx = egui::Context::default();

    // Egui state integration
    let mut egui_winit = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        &window,
        Some(window.scale_factor() as f32),
        None,
    );
    let mut egui_renderer =
        egui_wgpu::Renderer::new(&renderer.device, renderer.config.format, None, 1);

    // 4. Initialize Core Engine Systems (owned simulation state)
    let mut scene = Scene::new();
    let input = InputState::new();
    let mut nav = NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0);
    let mut console = ConsoleLogs::new();

    // 5. Seed and load the default scene.
    //
    // The procedural bot-chase demo used to be hand-built here (~130 lines). It now
    // lives in the tracked `assets/scenes/default.scene`, seeded into the gitignored
    // `project/scenes/` on boot (same pattern as `bot.lua`) and loaded as the boot
    // scene. The harness can boot any scene file the same way.
    let boot_scene_path = rusty::scene::seed_default_scene();
    {
        console.info(format!("Loading scene from {}...", boot_scene_path));
        if let Err(err) = scene.load_from_file(&boot_scene_path) {
            console.error(format!("Failed to load default scene: {}", err));
        }
        nav.bake(&scene);
    }

    // 6. Wrap the owned sim state in the window/GPU-agnostic GameWorld.
    let mut game = GameWorld::new(scene, input, nav, console);

    // 7. Editor + timing state (front-end only)
    let mut editor_ui = EditorUi::new();
    // Adopt the boot scene as the current scene so Save writes back to it.
    editor_ui.current_scene_path = Some(boot_scene_path);
    let mut timing = FrameTiming {
        last_frame_time: Instant::now(),
        frame_count: 0,
        fps: 60.0,
        last_fps_update: Instant::now(),
    };

    // 8. Execute Window Event Loop
    let _ = event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent {
                window_id,
                ref event,
            } if window_id == window.id() => {
                // Always feed events to egui so the header panel with Play/Stop buttons works
                let _ = egui_winit.on_window_event(&window, event);
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(physical_size) => {
                        renderer.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { .. } => {
                        // A DPI / display change (e.g. dragging between Retina and
                        // non-Retina monitors) can invalidate the swapchain without a
                        // Resized event. Reconfigure to the window's current inner size
                        // so the surface stays valid.
                        renderer.resize(window.inner_size());
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
                        {
                            let inp = game.input_mut();
                            match key {
                                KeyCode::KeyW => inp.set_key_state("W", pressed),
                                KeyCode::KeyA => inp.set_key_state("A", pressed),
                                KeyCode::KeyS => inp.set_key_state("S", pressed),
                                KeyCode::KeyD => inp.set_key_state("D", pressed),
                                KeyCode::ArrowUp => inp.set_key_state("UP", pressed),
                                KeyCode::ArrowDown => inp.set_key_state("DOWN", pressed),
                                KeyCode::ArrowLeft => inp.set_key_state("LEFT", pressed),
                                KeyCode::ArrowRight => inp.set_key_state("RIGHT", pressed),
                                // The shoot button is just the SPACE key; the player
                                // controller script edge-detects it. No engine-side
                                // "shoot" field — gameplay reads the key like any other.
                                KeyCode::Space => inp.set_key_state("SPACE", pressed),
                                _ => {}
                            }
                        }
                        // Hit ESC in PlayMode to unlock cursor (platform-side transition)
                        if *key == KeyCode::Escape && pressed && game.is_playing() {
                            game.set_playing(false);
                            game.console_mut()
                                .info("Unlocked cursor, entering EditorMode".to_string());
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        game.input_mut().mouse_position = (position.x, position.y);
                    }
                    WindowEvent::RedrawRequested => {
                        run_frame(
                            elwt,
                            &window,
                            &mut renderer,
                            &mut game,
                            &mut editor_ui,
                            &egui_ctx,
                            &mut egui_winit,
                            &mut egui_renderer,
                            &mut timing,
                        );
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    });
}
