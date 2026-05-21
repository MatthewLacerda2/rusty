mod primitives;
mod scene;
mod input;
mod navigation;
mod physics;
mod scripting;
mod renderer;
mod editor;

use std::rc::Rc;
use std::sync::Arc;
use std::cell::RefCell;
use std::collections::HashSet;
use std::time::{Instant, Duration};
use winit::{
    event::{Event, WindowEvent, DeviceEvent, ElementState, KeyEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    keyboard::{KeyCode, PhysicalKey},
};
use glam::Vec3;

use crate::scene::{Scene, LightType, LightComponent, ColliderComponent, HealthComponent, AnimatorComponent, ScriptComponent};
use crate::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scripting::{ScriptManager, ConsoleLogs};
use crate::renderer::{Renderer, Camera};
use crate::editor::EditorUi;
use crate::physics::{Ray, cast_ray_in_scene};

fn main() {
    env_logger::init();
    println!("[Engine] Starting Antigravity Engine 3D...");

    // 1. Setup local asset structure
    std::fs::create_dir_all("assets/scripts").ok();
    std::fs::create_dir_all("assets/textures").ok();
    std::fs::create_dir_all("assets/models").ok();

    // 2. Initialize Window Event Loop
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Rusty 3D Game Engine & Editor")
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720))
            .build(&event_loop)
            .unwrap()
    );

    // 3. Initialize Core Render Engine & egui Context
    let mut renderer = pollster::block_on(Renderer::new(Arc::clone(&window)));
    let egui_ctx = egui::Context::default();
    
    // Egui state integration
    let mut egui_winit = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        &window,
        Some(window.scale_factor() as f32),
        None,
    );
    let mut egui_renderer = egui_wgpu::Renderer::new(
        &renderer.device,
        renderer.config.format,
        None,
        1,
    );

    // 4. Initialize Core Engine Systems
    let scene = Rc::new(RefCell::new(Scene::new()));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0)));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    
    let mut script_manager = ScriptManager::new(
        Rc::clone(&scene),
        Rc::clone(&input),
        Rc::clone(&nav),
        Rc::clone(&console),
    );

    // 5. Populate Beautiful Demo 3D Scene
    {
        let mut s = scene.borrow_mut();
        console.borrow_mut().info("Loading default demo scene assets...".to_string());

        // A. Add Floor Plane (Procedural XZ grid)
        let floor_id = s.add_entity("Floor_Plane".to_string());
        let floor = s.get_entity_mut(floor_id).unwrap();
        floor.transform.scale = Vec3::new(2.5, 1.0, 2.5); // Large floor area
        floor.is_static = true;
        let (v_floor, idx_floor) = primitives::generate_plane(15.0, 15.0);
        floor.mesh = Some(scene::MeshComponent {
            primitive_type: "Plane".to_string(),
            vertices: v_floor,
            indices: idx_floor,
            is_dirty: std::cell::Cell::new(true),
        });
        floor.collider = Some(ColliderComponent { active: true, aabb_min: Vec3::ZERO, aabb_max: Vec3::ZERO });

        // B. Add Player Camera Anchor (so we can inspect / track player in Hierarchy)
        let player_id = s.add_entity("Player".to_string());
        let player = s.get_entity_mut(player_id).unwrap();
        player.transform.position = Vec3::new(0.0, 1.5, -6.0);
        let (v_player, idx_player) = primitives::generate_cylinder(Vec3::new(0.0, -0.8, 0.0), Vec3::new(0.0, 0.8, 0.0), 0.5, 12);
        player.mesh = Some(scene::MeshComponent {
            primitive_type: "Cylinder".to_string(),
            vertices: v_player,
            indices: idx_player,
            is_dirty: std::cell::Cell::new(true),
        });
        player.collider = Some(ColliderComponent { active: true, aabb_min: Vec3::ZERO, aabb_max: Vec3::ZERO });

        // C. Add Static Obstacle Walls (to test dynamic A* path avoidance!)
        let wall1_id = s.add_entity("Obstacle_Wall_Left".to_string());
        let wall1 = s.get_entity_mut(wall1_id).unwrap();
        wall1.transform.position = Vec3::new(3.0, 1.0, 2.0);
        wall1.transform.scale = Vec3::new(1.0, 2.0, 4.0);
        wall1.is_static = true;
        let (v_w1, idx_w1) = primitives::generate_box(1.0, 1.0, 1.0);
        wall1.mesh = Some(scene::MeshComponent {
            primitive_type: "Box".to_string(),
            vertices: v_w1,
            indices: idx_w1,
            is_dirty: std::cell::Cell::new(true),
        });
        wall1.collider = Some(ColliderComponent { active: true, aabb_min: Vec3::ZERO, aabb_max: Vec3::ZERO });

        let wall2_id = s.add_entity("Obstacle_Wall_Right".to_string());
        let wall2 = s.get_entity_mut(wall2_id).unwrap();
        wall2.transform.position = Vec3::new(-3.0, 1.0, 4.0);
        wall2.transform.scale = Vec3::new(4.0, 2.0, 1.0);
        wall2.is_static = true;
        let (v_w2, idx_w2) = primitives::generate_box(1.0, 1.0, 1.0);
        wall2.mesh = Some(scene::MeshComponent {
            primitive_type: "Box".to_string(),
            vertices: v_w2,
            indices: idx_w2,
            is_dirty: std::cell::Cell::new(true),
        });
        wall2.collider = Some(ColliderComponent { active: true, aabb_min: Vec3::ZERO, aabb_max: Vec3::ZERO });

        // E. Add Dynamic Enemy Bot entity
        let enemy_id = s.add_entity("Enemy_1".to_string());
        let enemy = s.get_entity_mut(enemy_id).unwrap();
        enemy.transform.position = Vec3::new(8.0, 1.0, 8.0);
        let (v_enemy, idx_enemy) = primitives::generate_box(1.3, 2.0, 1.3);
        enemy.mesh = Some(scene::MeshComponent {
            primitive_type: "Box".to_string(),
            vertices: v_enemy,
            indices: idx_enemy,
            is_dirty: std::cell::Cell::new(true),
        });
        enemy.collider = Some(ColliderComponent { active: true, aabb_min: Vec3::ZERO, aabb_max: Vec3::ZERO });
        enemy.health = Some(HealthComponent { current_health: 100.0, max_health: 100.0, is_dead: false });
        enemy.animator = Some(AnimatorComponent {
            current_clip: "Walk".to_string(),
            time: 0.0,
            speed: 3.0,
            is_playing: true,
            freeze: false,
        });
        // Auto-assign bot.lua script
        enemy.script = Some(ScriptComponent {
            path: "assets/scripts/bot.lua".to_string(),
            is_loaded: false,
        });

        s.update_all_colliders();
    }

    // 6. Editor state values
    let mut editor_ui = EditorUi::new();
    let mut camera = Camera::new(Vec3::new(0.0, 5.0, -10.0), 90.0, -20.0);
    
    // Core game state
    let mut is_playing = false;
    let mut was_playing_last_frame = false;
    
    // Timing counters
    let mut last_frame_time = Instant::now();
    let mut frame_count = 0;
    let mut fps = 60.0;
    let mut last_fps_update = Instant::now();
    let mut current_frame_duration = 0.0;

    let mut pathfinding_points = Vec::new();
    let mut last_path_bake = Instant::now();

    // 7. Execute Window Event Loop
    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent { window_id, ref event } if window_id == window.id() => {
                // Always feed events to egui so the header panel with Play/Stop buttons works
                let _ = egui_winit.on_window_event(&window, event);
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(physical_size) => {
                        renderer.resize(*physical_size);
                    }
                    WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(key), state, .. }, .. } => {
                        let pressed = *state == ElementState::Pressed;
                        let mut inp = input.borrow_mut();

                        match key {
                            KeyCode::KeyW => inp.set_key_state("W", pressed),
                            KeyCode::KeyA => inp.set_key_state("A", pressed),
                            KeyCode::KeyS => inp.set_key_state("S", pressed),
                            KeyCode::KeyD => inp.set_key_state("D", pressed),
                            KeyCode::ArrowUp => inp.set_key_state("UP", pressed),
                            KeyCode::ArrowDown => inp.set_key_state("DOWN", pressed),
                            KeyCode::ArrowLeft => inp.set_key_state("LEFT", pressed),
                            KeyCode::ArrowRight => inp.set_key_state("RIGHT", pressed),
                            KeyCode::Space => {
                                inp.space_pressed = pressed;
                                inp.set_key_state("SPACE", pressed);
                            }
                            KeyCode::Escape => {
                                if pressed && is_playing {
                                    // Hit ESC in PlayMode to unlock cursor
                                    is_playing = false;
                                    console.borrow_mut().info("Unlocked cursor, entering EditorMode".to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        if *button == winit::event::MouseButton::Left {
                            input.borrow_mut().mouse_left_clicked = *state == ElementState::Pressed;
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        input.borrow_mut().mouse_position = (position.x, position.y);
                    }
                    WindowEvent::RedrawRequested => {
                        // Compute timing metrics
                        let now = Instant::now();
                        let delta_time = now.duration_since(last_frame_time).as_secs_f32();
                        last_frame_time = now;
                        current_frame_duration = delta_time * 1000.0;

                        frame_count += 1;
                        if now.duration_since(last_fps_update) >= Duration::from_secs(1) {
                            fps = frame_count as f32;
                            frame_count = 0;
                            last_fps_update = now;
                        }

                        // State Machine Transitions
                        if is_playing && !was_playing_last_frame {
                            // ENTERING PLAYMODE: Lock cursor, reset & initialize mlua scripting context
                            window.set_cursor_grab(winit::window::CursorGrabMode::Confined).ok();
                            window.set_cursor_visible(false);
                            console.borrow_mut().info("Capturing cursor, entering PlayMode!".to_string());

                            // Place camera behind Player cylinder
                            let s = scene.borrow();
                            if let Some(player) = s.get_entity(2) { // Player is ID 2
                                camera.position = player.transform.position + Vec3::new(0.0, 1.5, -4.5);
                                camera.yaw = 90.0;
                                camera.pitch = -10.0;
                            }
                            drop(s);

                            // Trigger scripting reload
                            if let Err(err) = script_manager.init_runtime() {
                                console.borrow_mut().error(format!("Lua init error: {}", err));
                            } else {
                                // Load script files assigned in the scene
                                let s = scene.borrow();
                                let mut to_load = Vec::new();
                                for entity in &s.entities {
                                    if let Some(script) = &entity.script {
                                        to_load.push((entity.id, script.path.clone()));
                                    }
                                }
                                drop(s);

                                for (id, path) in to_load {
                                    if let Err(e) = script_manager.load_entity_script(id, &path) {
                                        console.borrow_mut().error(format!("Lua compile error (Entity {}): {}", id, e));
                                    }
                                }
                                script_manager.start_scripts();
                            }
                        } else if !is_playing && was_playing_last_frame {
                            // ENTERING EDITORMODE: Unlock cursor, shut down script runtime
                            window.set_cursor_grab(winit::window::CursorGrabMode::None).ok();
                            window.set_cursor_visible(true);
                            script_manager.shutdown();
                            pathfinding_points.clear();
                        }
                        was_playing_last_frame = is_playing;

                        // Ticking game mechanics
                        if is_playing {
                            // Dynamic A* Pathfinding grid rebaking (Runs once per second to prevent GPU lag)
                            if now.duration_since(last_path_bake) >= Duration::from_secs(1) {
                                let s = scene.borrow();
                                nav.borrow_mut().bake(&s);
                                
                                // Visualize current pathfinding from Enemy 1 to Player
                                if let Some(enemy) = s.get_entity(5) { // Enemy 1 is ID 5
                                    if let Some(player) = s.get_entity(2) {
                                        let grid = nav.borrow();
                                        let (es_x, es_z) = grid.world_to_grid(enemy.transform.position);
                                        let (pl_x, pl_z) = grid.world_to_grid(player.transform.position);
                                        
                                        // Run internal A* path solver
                                        let mut pts = Vec::new();
                                        pts.push(enemy.transform.position);
                                        if let Some(grid_pts) = grid.find_path(es_x, es_z, pl_x, pl_z) {
                                            for &(gx, gz) in &grid_pts {
                                                pts.push(grid.grid_to_world(gx, gz));
                                            }
                                        }
                                        pts.push(player.transform.position);
                                        pathfinding_points = pts;
                                    }
                                }
                                last_path_bake = now;
                            }

                            // 1. Update active Lua scripting systems
                            script_manager.update_scripts(delta_time);

                            // 2. PlayMode Input Controls (Drive the Player entity movement & look)
                            {
                                let mut s = scene.borrow_mut();
                                let inp = input.borrow();

                                let mut move_dir = Vec3::ZERO;
                                if inp.is_key_down("W") { move_dir += camera.forward(); }
                                if inp.is_key_down("S") { move_dir -= camera.forward(); }
                                if inp.is_key_down("A") { move_dir -= camera.right(); }
                                if inp.is_key_down("D") { move_dir += camera.right(); }
                                // Flatten Y axis so player doesn't fly upwards when looking up
                                move_dir.y = 0.0;

                                if move_dir.length_squared() > 0.001 {
                                    let speed = 5.0 * delta_time;
                                    if let Some(player) = s.get_entity_mut(2) {
                                        player.transform.position += move_dir.normalize() * speed;
                                        player.update_collider();
                                    }
                                }

                                // Look rotations using Arrow Keys
                                let look_speed = 90.0 * delta_time;
                                if inp.is_key_down("LEFT") { camera.yaw -= look_speed; }
                                if inp.is_key_down("RIGHT") { camera.yaw += look_speed; }
                                if inp.is_key_down("UP") { camera.pitch += look_speed; }
                                if inp.is_key_down("DOWN") { camera.pitch -= look_speed; }
                                camera.pitch = camera.pitch.clamp(-80.0, 80.0);

                                // Snap camera target position slightly behind player
                                if let Some(player) = s.get_entity(2) {
                                    let offset = -camera.forward() * 4.5 + Vec3::new(0.0, 1.5, 0.0);
                                    camera.position = player.transform.position + offset;
                                }
                            } // s and inp dropped here

                            // 3. Shooting & Hitscan calculations (Triggered on Space key or Left Click)
                            {
                                let should_shoot = {
                                    let inp = input.borrow();
                                    inp.mouse_left_clicked || inp.space_pressed
                                };

                                if should_shoot {
                                    {
                                        let mut inp_mut = input.borrow_mut();
                                        inp_mut.mouse_left_clicked = false;
                                        inp_mut.space_pressed = false;
                                        inp_mut.set_key_state("SPACE", false);
                                    }

                                    console.borrow_mut().info("💥 Fired hitscan laser beam!".to_string());
                                    let shoot_ray = Ray::new(camera.position, camera.forward());

                                    let hit_result = {
                                        let s = scene.borrow();
                                        cast_ray_in_scene(&shoot_ray, &s)
                                    };

                                    if let Some((hit_id, hit_t)) = hit_result {
                                        let name = {
                                            let s = scene.borrow();
                                            s.get_entity(hit_id).map(|e| e.name.clone())
                                        };
                                        if let Some(name) = name {
                                            console.borrow_mut().info(format!(
                                                "  🎯 Direct hit! Struck {} at distance t={:.2} units", name, hit_t
                                            ));
                                        }
                                        script_manager.trigger_damage(hit_id, 25.0);
                                    } else {
                                        console.borrow_mut().info("  💨 Shot missed into empty space.".to_string());
                                    }
                                }
                            }

                            // 4. Animator update ticks
                            {
                                let mut s = scene.borrow_mut();
                                for entity in &mut s.entities {
                                    if entity.active {
                                        if let Some(anim) = &mut entity.animator {
                                            if anim.is_playing && !anim.freeze {
                                                anim.time += delta_time;
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            // Editor Mode: Standard free camera fly controls
                            let inp = input.borrow();
                            let mut move_dir = Vec3::ZERO;
                            if inp.is_key_down("W") { move_dir += camera.forward(); }
                            if inp.is_key_down("S") { move_dir -= camera.forward(); }
                            if inp.is_key_down("A") { move_dir -= camera.right(); }
                            if inp.is_key_down("D") { move_dir += camera.right(); }

                            if move_dir.length_squared() > 0.001 {
                                let speed = 10.0 * delta_time;
                                camera.position += move_dir.normalize() * speed;
                            }

                            let look_speed = 90.0 * delta_time;
                            if inp.is_key_down("LEFT") { camera.yaw -= look_speed; }
                            if inp.is_key_down("RIGHT") { camera.yaw += look_speed; }
                            if inp.is_key_down("UP") { camera.pitch += look_speed; }
                            if inp.is_key_down("DOWN") { camera.pitch -= look_speed; }
                            camera.pitch = camera.pitch.clamp(-80.0, 80.0);
                        }

                        // --- GPU RENDER TICK ---
                        let frame = match renderer.surface.get_current_texture() {
                            Ok(f) => f,
                            Err(e) => {
                                eprintln!("[WGPU] Swapchain error: {}", e);
                                return;
                            }
                        };
                        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

                        // Render the 3D scene (Forward unlit/lit + gizmos line drawers)
                        {
                            let s = scene.borrow();
                            renderer.render(&s, &camera, &view, !is_playing, &pathfinding_points);
                        }

                        // Render Egui Overlay (header always visible, side panels only in editor mode)
                        {
                            let raw_input = egui_winit.take_egui_input(&window);
                            egui_ctx.begin_frame(raw_input);

                            // Draw Editor dashboard UI (header always, side panels only in EditorMode)
                            {
                                let mut s = scene.borrow_mut();
                                let mut c = console.borrow_mut();
                                editor_ui.draw(&egui_ctx, &mut s, &mut c, &mut is_playing, fps, current_frame_duration);
                            }

                            let full_output = egui_ctx.end_frame();
                            let paint_jobs = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

                            // Upload egui textures
                            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                size_in_pixels: [renderer.config.width, renderer.config.height],
                                pixels_per_point: window.scale_factor() as f32,
                            };

                            for (id, image_delta) in &full_output.textures_delta.set {
                                egui_renderer.update_texture(&renderer.device, &renderer.queue, *id, image_delta);
                            }

                            // Write Egui vertex/index buffers
                            let mut encoder = renderer.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Egui Render Encoder"),
                            });

                            egui_renderer.update_buffers(
                                &renderer.device,
                                &renderer.queue,
                                &mut encoder,
                                &paint_jobs,
                                &screen_descriptor,
                            );

                            {
                                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: Some("Egui Render Pass"),
                                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                        view: &view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Load, // Don't clear! Draw egui ON TOP of 3D scene texture
                                            store: wgpu::StoreOp::Store,
                                        },
                                    })],
                                    depth_stencil_attachment: None,
                                    timestamp_writes: None,
                                    occlusion_query_set: None,
                                });

                                egui_renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
                            }

                            // Submit egui commands
                            renderer.queue.submit(std::iter::once(encoder.finish()));

                            // Free egui textures
                            for id in &full_output.textures_delta.free {
                                egui_renderer.free_texture(id);
                            }
                        }

                        // Present texture swapchain
                        frame.present();
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
