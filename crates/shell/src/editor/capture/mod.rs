//! crates/shell/src/editor/capture/ — the whole editor, offscreen, to a PNG.

pub mod bench;
mod demo;
mod paint;
mod preview;
mod viewport;

use std::{cell::RefCell, path::Path, rc::Rc};

use preview::PreviewTarget;
use viewport::render_viewport;

pub use demo::capture_demo;
pub(super) use paint::paint;
pub use preview::{parse_mesh, refuse_with_picker, PreviewRect};
use rusty_app::{GameWorld, ProjectSource};
use rusty_core::application::{GameViewSetting, SceneGizmos};
use rusty_core::input::InputState;
use rusty_dev::capture::CaptureHost;
use rusty_editor::{viewport::scene_nav::selection_bounds, EditorUi, PreviewMesh, ViewportTab};
use rusty_navigation::NavigationGraph;
use rusty_render::{readback, OFFSCREEN_FORMAT};
use rusty_scene::Scene;
use rusty_script::scripting::ConsoleLogs;

pub const DEFAULT_WIDTH: u32 = 1600;
pub const DEFAULT_HEIGHT: u32 = 900;
pub const DEFAULT_FRAMES: u32 = 4;
const FRAME_SECONDS: f64 = 1.0;

#[derive(Clone, Debug)]
pub struct EditorCaptureOptions {
    pub width: u32,
    pub height: u32,
    pub frames: u32,
    pub select: Option<String>,
    pub select_asset: Option<String>,
    pub frame_selected: bool,
    /// Inspect this reflection probe first, as `Editor.InspectReflectionProbe` does.
    pub inspect_reflection_probe: Option<usize>,
    pub playing: bool,
    /// Which viewport tab is showing.
    pub tab: ViewportTab,
    pub pointer: Option<(f32, f32)>,
    /// The Scene view's gizmos, as its toggles or `Editor.SetGizmo` set them.
    pub gizmos: SceneGizmos,
    /// The Game view's size, as its dropdown or `Editor.SetGameViewSize` sets it.
    pub game_view: GameViewSetting,
    /// Open the selection's Inspector Preview tab on this mesh: `--preview`.
    pub preview: Option<PreviewMesh>,
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
            inspect_reflection_probe: None,
            playing: false,
            tab: ViewportTab::Scene,
            pointer: None,
            gizmos: SceneGizmos::default(),
            game_view: GameViewSetting::default(),
            preview: None,
        }
    }
}

pub fn default_world(project: ProjectSource) -> GameWorld {
    rusty_edit::seed_default_scripts();
    rusty_edit::default_scene::seed_default_assets();
    let mut scene = Scene::new();
    rusty_edit::default_scene::build(&mut scene, rusty_edit::default_scene::BOT_SCRIPT);
    let nav = NavigationGraph::from_scene(&scene);
    let game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    game.open_project(project);
    game
}

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
    Ok(capture_shot(host, game, path, opts)?.is_some())
}

#[derive(Clone, Copy, Debug)]
pub struct Shot {
    /// Under `--preview`, the Preview tab's image rect in the PNG.
    pub preview: Option<PreviewRect>,
}

/// [`capture_into`], saying what it wrote: `Ok(None)` with no adapter.
pub fn capture_shot(
    host: &mut CaptureHost,
    game: &GameWorld,
    path: impl AsRef<Path>,
    opts: &EditorCaptureOptions,
) -> Result<Option<Shot>, String> {
    let ui = &mut initial_ui(game, opts)?;
    capture_with(host, game, ui, path, opts)
}

fn capture_with(
    host: &mut CaptureHost,
    game: &GameWorld,
    ui: &mut EditorUi,
    path: impl AsRef<Path>,
    opts: &EditorCaptureOptions,
) -> Result<Option<Shot>, String> {
    let (width, height) = (opts.width.max(1), opts.height.max(1));
    let Some(renderer) = host.renderer(width, height) else {
        rusty_dev::capture::no_adapter("capture the editor")?;
        return Ok(None);
    };
    let mut egui_renderer = egui_wgpu::Renderer::new(
        &renderer.device,
        OFFSCREEN_FORMAT,
        super::paint::EGUI_RENDERER,
    );
    let ctx = egui::Context::default();
    let mut texture_id = None;
    let (mut output, mut preview) = (None, PreviewTarget::default());
    for frame in 0..opts.frames.max(1) {
        let mut input = raw_input(width, height, frame);
        if let Some((x, y)) = opts.pointer {
            input
                .events
                .push(egui::Event::PointerMoved(egui::pos2(x, y)));
        }
        ctx.begin_pass(input);
        preview.bind(ui);
        let drawn = draw_ui(ui, &ctx, game, opts.playing, texture_id);
        render_viewport(host, &mut egui_renderer, game, ui, drawn, &mut texture_id);
        let renderer = host.renderer(width, height).expect("probed above");
        preview.render((renderer, &mut egui_renderer), ui);
        output = Some(ctx.end_pass());
        let renderer = host.renderer(width, height).expect("probed above");
        let out: &mut egui::FullOutput = output.as_mut().expect("just set");
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                egui_renderer.update_texture(&renderer.device, &renderer.queue, *id, delta);
            }
        }
        out.textures_delta.clear();
    }
    let shown = opts
        .preview
        .map(|_| preview.shown(width, height))
        .transpose()?;
    let output = output.expect("at least one frame");
    let renderer = host.renderer(width, height).expect("probed above");
    let target = paint(renderer, &mut egui_renderer, &ctx, output, width, height);
    let pixels =
        readback::read_texture_rgba8(&renderer.device, &renderer.queue, &target, width, height);
    readback::write_png(path, width, height, &pixels)?;
    Ok(Some(Shot { preview: shown }))
}

/// A fresh editor with `opts`' tab, selection and framing applied.
fn initial_ui(game: &GameWorld, opts: &EditorCaptureOptions) -> Result<EditorUi, String> {
    let mut ui = EditorUi::new(game.script_manager().scene_path_cell());
    game.refresh_ui_textures();
    super::sync_ui_textures(&mut ui, game);
    ui.viewport_tab = opts.tab;
    (ui.gizmos, ui.game_view) = (opts.gizmos, opts.game_view);
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
    if let Some(index) = opts.inspect_reflection_probe {
        game.scene().borrow_mut().inspect_reflection_probe(index)?;
    }
    preview::open(&mut ui, &game.scene().borrow(), opts.preview)?;
    Ok(ui)
}

pub(super) fn raw_input(width: u32, height: u32, frame: u32) -> egui::RawInput {
    let size = egui::vec2(width as f32, height as f32);
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        time: Some(f64::from(frame) * FRAME_SECONDS),
        predicted_dt: FRAME_SECONDS as f32,
        ..Default::default()
    }
}

fn draw_ui(
    ui: &mut EditorUi,
    ctx: &egui::Context,
    game: &GameWorld,
    playing: bool,
    texture_id: Option<egui::TextureId>,
) -> rusty_editor::ViewportInteraction {
    let mut is_playing = playing;
    let clock = rusty_editor::viewport::stats::Clock::at(60.0);
    super::sync::sync_stats(ui, game, clock, false);
    ui.draw(
        ctx,
        &mut game.world.scene.borrow_mut(),
        &mut game.resources.console.borrow_mut(),
        &mut game.resources.nav.borrow_mut(),
        &mut is_playing,
        texture_id,
    )
}
