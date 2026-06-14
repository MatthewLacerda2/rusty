mod app;
mod core;
mod editor;
mod navigation;
mod physics;
mod render;
mod scripting;

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

use glam::Vec3;

use crate::app::{GameWorld, PlayTransition};
use crate::core::input::InputState;
use crate::core::scene::{
    self, AnimatorComponent, ColliderComponent, ColliderShape, HealthComponent, RigidBodyComponent,
    Scene, ScriptComponent,
};
use crate::editor::EditorUi;
use crate::navigation::NavigationGraph;
use crate::render::mesh as primitives;
use crate::render::Renderer;
use crate::scripting::ConsoleLogs;

fn main() {
    env_logger::init();
    println!("[Engine] Starting Antigravity Engine 3D...");

    // 1. Setup local asset structure inside git-ignored project folder
    std::fs::create_dir_all("project/assets/scripts").ok();
    std::fs::create_dir_all("project/assets/textures").ok();
    std::fs::create_dir_all("project/assets/models").ok();
    std::fs::create_dir_all("project/assets/audio").ok();
    std::fs::create_dir_all("project/scenes").ok();

    // Ensure default demo bot.lua exists in project/assets/scripts/
    if !std::path::Path::new("project/assets/scripts/bot.lua").exists() {
        if let Ok(default_code) = std::fs::read_to_string("assets/scripts/bot.lua") {
            std::fs::write("project/assets/scripts/bot.lua", default_code).ok();
        } else {
            let fallback_lua = r#"
local BotAI = {}
BotAI.health = 100.0
function BotAI.Start(entity_id)
    Transform.SetPosition(entity_id, 8.0, 1.0, 8.0)
    Animator.Play(entity_id, "Walk")
end
function BotAI.Update(entity_id, delta_time)
    local player_id = Scene.FindEntityByName("Player")
    if player_id then
        local pos_x, pos_y, pos_z = Transform.GetPosition(entity_id)
        local target_x, target_y, target_z = Transform.GetPosition(player_id)
        local next_x, next_y, next_z = Navigation.GetNextPathStep(pos_x, pos_y, pos_z, target_x, target_y, target_z)
        Transform.MoveTowards(entity_id, next_x, next_y, next_z, 3.0 * delta_time)
        local let_dx = target_x - pos_x
        local let_dz = target_z - pos_z
        local angle = math.atan2(let_dx, let_dz) * (180.0 / math.pi)
        Transform.SetRotation(entity_id, 0.0, angle, 0.0)
    end
end
function BotAI.Damage(entity_id, amount)
    BotAI.health = BotAI.health - amount
    if BotAI.health <= 0.0 then
        Animator.Play(entity_id, "Death")
    else
        Animator.Play(entity_id, "Hit")
    end
end
return BotAI
"#;
            std::fs::write("project/assets/scripts/bot.lua", fallback_lua).ok();
        }
    }

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
    let mut egui_renderer =
        egui_wgpu::Renderer::new(&renderer.device, renderer.config.format, None, 1);

    // 4. Initialize Core Engine Systems (shared simulation state)
    let scene = Rc::new(RefCell::new(Scene::new()));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));

    // 5. Populate Beautiful Demo 3D Scene
    {
        let mut s = scene.borrow_mut();
        console
            .borrow_mut()
            .info("Loading default demo scene assets...".to_string());

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
        floor.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box {
                size: Vec3::new(15.0, 0.1, 15.0),
            },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });

        // B. Add Player Camera Anchor
        let player_id = s.add_entity("Player".to_string());
        let player = s.get_entity_mut(player_id).unwrap();
        player.transform.position = Vec3::new(0.0, 1.5, -6.0);
        let (v_player, idx_player) = primitives::generate_cylinder(
            Vec3::new(0.0, -0.8, 0.0),
            Vec3::new(0.0, 0.8, 0.0),
            0.5,
            12,
        );
        player.mesh = Some(scene::MeshComponent {
            primitive_type: "Cylinder".to_string(),
            vertices: v_player,
            indices: idx_player,
            is_dirty: std::cell::Cell::new(true),
        });
        player.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Cylinder {
                radius: 0.5,
                height: 1.6,
            },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });
        player.rigidbody = Some(RigidBodyComponent {
            active: true,
            is_kinematic: true,
            mass: 80.0,
            velocity: Vec3::ZERO,
            use_gravity: false,
        });

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
        wall1.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box { size: Vec3::ONE },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });

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
        wall2.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box { size: Vec3::ONE },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });

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
        enemy.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box {
                size: Vec3::new(1.3, 2.0, 1.3),
            },
            is_trigger: false,
            aabb_min: Vec3::ZERO,
            aabb_max: Vec3::ZERO,
        });
        enemy.rigidbody = Some(RigidBodyComponent {
            active: true,
            is_kinematic: true,
            mass: 80.0,
            velocity: Vec3::ZERO,
            use_gravity: false,
        });
        enemy.health = Some(HealthComponent {
            current_health: 100.0,
            max_health: 100.0,
            is_dead: false,
        });
        enemy.animator = Some(AnimatorComponent {
            current_clip: "Walk".to_string(),
            time: 0.0,
            speed: 3.0,
            is_playing: true,
            freeze: false,
        });
        enemy.script = Some(ScriptComponent {
            path: "project/assets/scripts/bot.lua".to_string(),
            is_loaded: false,
        });

        s.update_all_colliders();
        nav.borrow_mut().bake(&s);
    }

    // 6. Wrap the shared sim state in the window/GPU-agnostic GameWorld.
    let mut game = GameWorld::new(scene, input, nav, console);

    // 7. Editor + timing state (front-end only)
    let mut editor_ui = EditorUi::new();
    let mut last_frame_time = Instant::now();
    let mut frame_count = 0;
    let mut fps = 60.0;
    let mut last_fps_update = Instant::now();
    let mut current_frame_duration = 0.0;

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
                            let mut inp = game.input.borrow_mut();
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
                                _ => {}
                            }
                        }
                        // Hit ESC in PlayMode to unlock cursor (platform-side transition)
                        if *key == KeyCode::Escape && pressed && game.is_playing {
                            game.is_playing = false;
                            game.console
                                .borrow_mut()
                                .info("Unlocked cursor, entering EditorMode".to_string());
                        }
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        if *button == winit::event::MouseButton::Left {
                            game.input.borrow_mut().mouse_left_clicked =
                                *state == ElementState::Pressed;
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        game.input.borrow_mut().mouse_position = (position.x, position.y);
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

                        // Advance the simulation (decoupled from window + GPU).
                        let transition = game.tick(delta_time);
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

                        // --- GPU RENDER TICK ---
                        let frame = match renderer.surface.get_current_texture() {
                            Ok(f) => f,
                            Err(e) => {
                                eprintln!("[WGPU] Swapchain error: {}", e);
                                return;
                            }
                        };
                        let view = frame
                            .texture
                            .create_view(&wgpu::TextureViewDescriptor::default());

                        // Render the 3D scene (Forward unlit/lit + gizmos line drawers)
                        {
                            let s = game.scene.borrow();
                            renderer.render(
                                &s,
                                &game.camera,
                                &view,
                                !game.is_playing,
                                game.pathfinding_points(),
                            );
                        }

                        // Render Egui Overlay (header always visible, side panels only in editor mode)
                        {
                            let raw_input = egui_winit.take_egui_input(&window);
                            egui_ctx.begin_frame(raw_input);

                            // Draw Editor dashboard UI
                            {
                                let mut s = game.scene.borrow_mut();
                                let mut c = game.console.borrow_mut();
                                let mut n = game.nav.borrow_mut();
                                editor_ui.draw(
                                    &egui_ctx,
                                    &mut s,
                                    &mut c,
                                    &mut n,
                                    &mut game.is_playing,
                                    fps,
                                    current_frame_duration,
                                );
                                if editor_ui.is_dirty {
                                    renderer.shadow_renderer.is_static_cached = false;
                                    editor_ui.is_dirty = false;
                                }
                            }

                            let full_output = egui_ctx.end_frame();
                            let paint_jobs = egui_ctx
                                .tessellate(full_output.shapes, full_output.pixels_per_point);

                            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                size_in_pixels: [renderer.config.width, renderer.config.height],
                                pixels_per_point: window.scale_factor() as f32,
                            };

                            for (id, image_delta) in &full_output.textures_delta.set {
                                egui_renderer.update_texture(
                                    &renderer.device,
                                    &renderer.queue,
                                    *id,
                                    image_delta,
                                );
                            }

                            let mut encoder = renderer.device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("Egui Render Encoder"),
                                },
                            );

                            egui_renderer.update_buffers(
                                &renderer.device,
                                &renderer.queue,
                                &mut encoder,
                                &paint_jobs,
                                &screen_descriptor,
                            );

                            {
                                let mut render_pass =
                                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: Some("Egui Render Pass"),
                                        color_attachments: &[Some(
                                            wgpu::RenderPassColorAttachment {
                                                view: &view,
                                                resolve_target: None,
                                                ops: wgpu::Operations {
                                                    load: wgpu::LoadOp::Load,
                                                    store: wgpu::StoreOp::Store,
                                                },
                                            },
                                        )],
                                        depth_stencil_attachment: None,
                                        timestamp_writes: None,
                                        occlusion_query_set: None,
                                    });

                                egui_renderer.render(
                                    &mut render_pass,
                                    &paint_jobs,
                                    &screen_descriptor,
                                );
                            }

                            renderer.queue.submit(std::iter::once(encoder.finish()));

                            for id in &full_output.textures_delta.free {
                                egui_renderer.free_texture(id);
                            }
                        }

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
