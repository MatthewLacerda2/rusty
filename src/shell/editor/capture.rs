//! src/shell/editor/capture.rs — the whole editor, offscreen, to a PNG (#731).
//!
//! [`screenshot`](crate::dev::screenshot) shows what the *game* looks like; this shows what
//! the *editor* looks like — the egui panels around the scene viewport — with no window,
//! on lavapipe. It exists so an agent that changes the editor can attach before/after
//! captures to its PR and the operator can review the change from a phone (#725 did
//! this with a scratch example; this is that path, supported).
//!
//! **The same frame the editor draws, minus the window.** Each frame runs
//! [`EditorUi::draw`] exactly as the [`EditorFrontend`](super::EditorFrontend) does,
//! renders the scene into the viewport rect the layout asked for (through a [`CaptureHost`], so it is the
//! screenshot's renderer), rebinds that target as the viewport's egui texture, and on
//! the last frame paints egui into an offscreen target and reads it back.
//!
//! **Deterministic by construction:** a fixed size, a fixed pixel scale (1×), a fixed
//! frame count and a synthetic egui clock. Several frames are needed because egui loads
//! fonts registered in one frame on the *next*, and sizes some panels from the frame
//! before; [`DEFAULT_FRAMES`] covers both. The clock steps a whole second a frame,
//! longer than any egui animation (a window's fade-in, #333), so the painted frame
//! shows the settled UI a user sees. The sim is never stepped.
//!
//! No adapter → `Ok(false)`, the same skip contract as every headless capture.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::dev::capture::CaptureHost;
use crate::editor::viewport::scene_nav::selection_bounds;
use crate::editor::{EditorUi, ViewportTab};
use crate::navigation::NavigationGraph;
use crate::render::{readback, OFFSCREEN_FORMAT};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Default capture size: the 16:9 layout #725's captures used.
pub const DEFAULT_WIDTH: u32 = 1600;
pub const DEFAULT_HEIGHT: u32 = 900;
/// Frames drawn before the one painted: fonts land on frame 2, panel sizes settle by 3.
pub const DEFAULT_FRAMES: u32 = 4;
/// The synthetic clock's step: every animation has finished by the next frame.
const FRAME_SECONDS: f64 = 1.0;

/// What to capture: the size, the frame count and the UI state to apply first.
#[derive(Clone, Debug)]
pub struct EditorCaptureOptions {
    pub width: u32,
    pub height: u32,
    /// Frames drawn in total; the last is the one written. At least 1.
    pub frames: u32,
    /// Select the entity with this name first (the Inspector shows its cards).
    pub select: Option<String>,
    /// Select the asset at this path first (the Inspector shows its card: a scene's
    /// bake buttons, an image or audio preview, a prefab). Exclusive with `select`.
    pub select_asset: Option<String>,
    /// Frame the selection in the Scene view first, as the F key does (#745).
    pub frame_selected: bool,
    /// Draw the chrome as in Play mode (accent tint, floating console). The sim is
    /// not entered or stepped: this is the look, not a playtest.
    pub playing: bool,
    /// Which viewport tab is showing.
    pub tab: ViewportTab,
}

impl Default for EditorCaptureOptions {
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            frames: DEFAULT_FRAMES,
            select: None,
            select_asset: None,
            frame_selected: false,
            playing: false,
            tab: ViewportTab::Scene,
        }
    }
}

/// The editor's default scene in a fresh edit-mode world — what `cargo run` opens on a
/// new project, built in memory like the harness does (its textures and shader seeded
/// into the open project). Neither the sim nor any script runs.
pub fn default_world() -> GameWorld {
    crate::scene::seed_default_scripts();
    crate::scene::default_scene::seed_default_assets();
    let mut scene = Scene::new();
    crate::scene::default_scene::build(&mut scene, crate::scene::default_scene::BOT_SCRIPT);
    let nav = NavigationGraph::from_scene(&scene);
    GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    )
}

/// Capture the editor over `game`'s scene to a PNG at `path`.
///
/// `Ok(true)` when written, `Ok(false)` when the box has no GPU/software adapter, `Err`
/// for a `select` name the scene doesn't hold, a `select_asset` path that doesn't
/// exist, both selections at once, or an I/O / encode failure.
pub fn capture(
    game: &GameWorld,
    path: impl AsRef<Path>,
    opts: &EditorCaptureOptions,
) -> Result<bool, String> {
    capture_into(&mut CaptureHost::new(), game, path, opts)
}

/// [`capture`] through a caller-owned host, so several captures share one renderer.
pub fn capture_into(
    host: &mut CaptureHost,
    game: &GameWorld,
    path: impl AsRef<Path>,
    opts: &EditorCaptureOptions,
) -> Result<bool, String> {
    let (width, height) = (opts.width.max(1), opts.height.max(1));
    let ui = &mut initial_ui(game, opts)?;
    let Some(renderer) = host.renderer(width, height) else {
        log::warn!("[EditorCapture] no GPU/software adapter available — skipping capture");
        return Ok(false);
    };
    let mut egui_renderer = egui_wgpu::Renderer::new(
        &renderer.device,
        OFFSCREEN_FORMAT,
        super::paint::EGUI_RENDERER,
    );
    let ctx = egui::Context::default();
    let mut texture_id = None;
    let mut output = None;
    for frame in 0..opts.frames.max(1) {
        ctx.begin_pass(raw_input(width, height, frame));
        let size = draw_ui(ui, &ctx, game, opts.playing, texture_id);
        render_viewport(host, &mut egui_renderer, game, ui, size, &mut texture_id);
        output = Some(ctx.end_pass());
        // Upload font atlas / image deltas every frame: egui sends each only once.
        let renderer = host.renderer(width, height).expect("probed above");
        let out: &mut egui::FullOutput = output.as_mut().expect("just set");
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                egui_renderer.update_texture(&renderer.device, &renderer.queue, *id, delta);
            }
        }
        // Handled: egui asserts every delta is consumed. Frees are moot for one shot.
        out.textures_delta.clear();
    }
    let output = output.expect("at least one frame");
    let renderer = host.renderer(width, height).expect("probed above");
    let target = paint(renderer, &mut egui_renderer, &ctx, output, width, height);
    let pixels =
        readback::read_texture_rgba8(&renderer.device, &renderer.queue, &target, width, height);
    readback::write_png(path, width, height, &pixels)?;
    Ok(true)
}

/// A fresh editor with `opts`' tab, selection and framing applied.
fn initial_ui(game: &GameWorld, opts: &EditorCaptureOptions) -> Result<EditorUi, String> {
    let mut ui = EditorUi::new();
    ui.viewport_tab = opts.tab;
    if let Some(name) = &opts.select {
        let id = game.scene().borrow().find_entity_by_name(name);
        ui.selected_entity_id = Some(id.ok_or_else(|| format!("no entity named {name:?}"))?);
    }
    if let Some(path) = &opts.select_asset {
        if opts.select.is_some() {
            return Err("select an entity or an asset, not both".to_string());
        }
        if !Path::new(path).exists() {
            return Err(format!("no asset at {path:?}"));
        }
        ui.selected_asset_path = Some(path.clone());
    }
    let framed = ui.selected_entity_id.filter(|_| opts.frame_selected);
    let bounds = framed.and_then(|id| selection_bounds(&game.scene().borrow(), id));
    if let Some((center, radius)) = bounds {
        ui.scene_view.camera.frame(center, radius);
    }
    Ok(ui)
}

/// A frame's input: the fixed screen rect and a synthetic 60 Hz clock, nothing else.
fn raw_input(width: u32, height: u32, frame: u32) -> egui::RawInput {
    let size = egui::vec2(width as f32, height as f32);
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        time: Some(f64::from(frame) * FRAME_SECONDS),
        predicted_dt: FRAME_SECONDS as f32,
        ..Default::default()
    }
}

/// Run the editor's own draw over the live cells; returns the viewport rect's size.
fn draw_ui(
    ui: &mut EditorUi,
    ctx: &egui::Context,
    game: &GameWorld,
    playing: bool,
    texture_id: Option<egui::TextureId>,
) -> egui::Vec2 {
    let mut is_playing = playing;
    let interaction = ui.draw(
        ctx,
        &mut game.world.scene.borrow_mut(),
        &mut game.resources.console.borrow_mut(),
        &mut game.resources.nav.borrow_mut(),
        &mut is_playing,
        60.0,
        1000.0 / 60.0,
        texture_id,
    );
    interaction.size
}

/// Render the scene into a target the viewport's size and bind it as its egui texture —
/// `shell::editor`'s `render_viewport_scene`, against the capture host.
fn render_viewport(
    host: &mut CaptureHost,
    egui_renderer: &mut egui_wgpu::Renderer,
    game: &GameWorld,
    ui: &EditorUi,
    size: egui::Vec2,
    texture_id: &mut Option<egui::TextureId>,
) {
    let (px, py) = (size.x.round() as u32, size.y.round() as u32);
    if px == 0 || py == 0 {
        return;
    }
    game.resources.screen.borrow_mut().set_game_view(px, py);
    let Some((renderer, view)) = host.frame(px, py) else {
        return;
    };
    let Some(target) = view.color_target_view() else {
        return;
    };
    let scene = game.scene().borrow();
    let scene_tab = ui.viewport_tab == ViewportTab::Scene;
    view.ui.screen_in_editor = scene_tab && ui.ui_overlay;
    let camera = super::viewport::viewport_camera(ui, game, &scene, ui.viewport_tab);
    renderer.render(view, &scene, &camera, &target, scene_tab);
    super::viewport::rebind_egui_texture(egui_renderer, &renderer.device, texture_id, view);
}

/// Tessellate `output` and paint it over a cleared `width × height` offscreen target.
fn paint(
    renderer: &crate::render::Renderer,
    egui_renderer: &mut egui_wgpu::Renderer,
    ctx: &egui::Context,
    output: egui::FullOutput,
    width: u32,
    height: u32,
) -> wgpu::Texture {
    let (device, queue) = (&renderer.device, &renderer.queue);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Editor Capture Target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: OFFSCREEN_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&Default::default());
    let jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [width, height],
        pixels_per_point: output.pixels_per_point,
    };
    let mut encoder = device.create_command_encoder(&Default::default());
    egui_renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
    {
        let mut pass = encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("Editor Capture Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            })
            .forget_lifetime();
        egui_renderer.render(&mut pass, &jobs, &screen);
    }
    queue.submit(std::iter::once(encoder.finish()));
    target
}
