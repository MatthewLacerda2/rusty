//! `make bench-scenes`' editor round (#2314): the editor's own costs on a
//! project scene, which the game rounds never draw.
//!
//! The scene is opened as the editor opens it, in edit mode and not standalone
//! ([`editor_world`]), so Play takes its edit snapshot and Stop restores it. Each
//! run draws [`Round::warmup`] frames, then [`Round::frames`] measured ones, of the
//! editor at the bench's size: the Scene tab, its default gizmos, the scene's first
//! camera selected, the Scene view posed at the scene's first game viewpoint
//! (`dev::bench::scenes::views::pick`: its first camera, or the fallbacks after
//! it). A frame is the capture's ([`super::capture`]): `EditorUi::draw` and egui's
//! tessellation (timed as `ui_ms`), the viewport's render and egui's paint (the
//! renderer's stages and GPU passes), waited on. Then one Play entry, one Stop
//! (to the first edit frame drawn) and one save each run, into the meter
//! (`dev::bench::scenes::editor::Meter`), which returns plain rows.
//!
//! **The project is read-only.** The save goes to `Round::save_dir` and is
//! deleted after; the world's store is pathless, so Stop's flush writes nothing;
//! the process is marked standalone for the asset gate (as the bench's harness
//! is), so Play's refresh compresses no texture into the project.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use rusty_app::{GameWorld, ProjectSource};
use rusty_core::input::InputState;
use rusty_core::project::{BootPlan, ProjectSettings};
use rusty_dev::bench::scenes::editor::{gpu_speed, EditorRound, Meter, UiTimer};
use rusty_dev::capture::{CaptureHost, RenderedFrame};
use rusty_editor::viewport::scene_camera::SceneCamera;
use rusty_editor::{EditorUi, ViewportTab};
use rusty_navigation::NavigationGraph;
use rusty_render::{ClockProbe, OFFSCREEN_FORMAT};
use rusty_scene::{Camera, Scene};
use rusty_script::scripting::ConsoleLogs;

use super::paint::{paint_jobs, target, Painted};

/// What the round measures, and where its save goes.
#[derive(Clone, Debug)]
pub struct Round {
    pub size: (u32, u32),
    pub warmup: u32,
    pub frames: u32,
    pub runs: u32,
    /// Build every lazy pipeline (the editor's overlays too) first, off the
    /// clock, as the editor's window does.
    pub warm: bool,
    /// The folder the save writes into, outside the project; removed after.
    pub save_dir: PathBuf,
}

/// The scene at `scene_path` (under the working directory, the project's root)
/// in an edit-mode editor world on `settings`, unbound: what the editor opens,
/// minus the window, the project's store and the demo recording. `Err` with the
/// console's lines when the scene does not load.
pub fn editor_world(scene_path: &str, settings: ProjectSettings) -> Result<GameWorld, String> {
    rusty_asset::convert::mark_standalone();
    let (mut scene, mut console) = (Scene::new(), ConsoleLogs::new());
    let plan = BootPlan::of(scene_path);
    let Some(loaded) = rusty_app::load_boot_scene(&mut scene, &mut console, &plan, false) else {
        let said: Vec<&str> = console.since(0).map(|(m, _)| m.as_str()).collect();
        return Err(format!("{scene_path} did not load: {}", said.join("; ")));
    };
    let nav = NavigationGraph::from_scene(&scene);
    let mut game = GameWorld::new(
        Rc::new(RefCell::new(scene)),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(console)),
    );
    rusty_dev::stats::install(&mut game);
    rusty_dev::lua_surface::install_api();
    *game.script_manager().scene_path_cell().borrow_mut() = Some(loaded);
    let init = game.init_edit_runtime();
    game.report_lua_init(init);
    game.open_project(ProjectSource::ReadOnly(settings));
    Ok(game)
}

/// Run the round over `game` (an [`editor_world`]): `Ok(None)` with no adapter
/// (no rows are made up), `Err` when a save fails.
pub fn round(game: &mut GameWorld, opts: &Round) -> Result<Option<EditorRound>, String> {
    let Some(mut editor) = Editor::new(game, opts) else {
        rusty_dev::capture::no_adapter("bench the editor")?;
        return Ok(None);
    };
    let mut meter = Meter::new();
    let _ = std::fs::create_dir_all(&opts.save_dir);
    let saved = (1..=opts.runs.max(1)).try_for_each(|_| {
        for _ in 0..opts.warmup {
            editor.frame(game);
        }
        for _ in 0..opts.frames {
            let (ui, rendered, gpu) = editor.frame(game);
            meter.frame(ui, rendered.as_ref(), gpu);
        }
        play_stop_save(&mut editor, game, &mut meter, opts)?;
        meter.end_run();
        Ok::<_, String>(())
    });
    let _ = std::fs::remove_dir_all(&opts.save_dir);
    saved.map(|()| Some(meter.finish()))
}

/// One Play entry, one Stop and one save, each a single sample of the run.
fn play_stop_save(
    editor: &mut Editor,
    game: &mut GameWorld,
    meter: &mut Meter,
    opts: &Round,
) -> Result<(), String> {
    let dt = rusty_time::FIXED_DELTA_TIME;
    let rss_before = rusty_dev::memory::rss_now_bytes();
    let start = Instant::now();
    game.set_playing(true);
    game.tick(dt); // the transition: the snapshot, `enter_play`, then the first FixedUpdate
    meter.single("play_enter_ms", ms_since(start));
    let rss_after = rusty_dev::memory::rss_now_bytes();
    if let (Some(before), Some(after)) = (rss_before, rss_after) {
        let grew = after.saturating_sub(before) as f64 / (1024.0 * 1024.0);
        meter.single("play_rss_mb", (grew * 10.0).round() / 10.0);
    }
    let start = Instant::now();
    game.set_playing(false);
    game.tick(dt); // `exit_play`: the restore and the rehydrate
    editor.frame(game);
    meter.single("play_stop_ms", ms_since(start));
    let path = opts.save_dir.join("editor-save.scene");
    let path = path.to_string_lossy();
    let start = Instant::now();
    rusty_edit::save_to_file(&game.scene().borrow(), &path)?;
    meter.single("save_ms", ms_since(start));
    Ok(())
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

/// The editor the round draws: its UI, egui, and the renderer they share.
struct Editor {
    host: CaptureHost,
    egui: egui_wgpu::Renderer,
    ctx: egui::Context,
    ui: EditorUi,
    texture: Option<egui::TextureId>,
    target: wgpu::Texture,
    probe: Option<ClockProbe>,
    size: (u32, u32),
    drawn: u32,
}

impl Editor {
    /// The editor over `game`, posed and selected (see the module docs); `None`
    /// with no adapter.
    fn new(game: &GameWorld, opts: &Round) -> Option<Self> {
        let size = (opts.size.0.max(1), opts.size.1.max(1));
        let mut host = CaptureHost::new();
        let renderer = host.renderer(size.0, size.1)?;
        if opts.warm {
            renderer.warm_pipelines(true);
        }
        let egui = egui_wgpu::Renderer::new(
            &renderer.device,
            OFFSCREEN_FORMAT,
            super::super::paint::EGUI_RENDERER,
        );
        let target = target(&renderer.device, size.0, size.1);
        let probe = ClockProbe::new(renderer);
        let mut ui = EditorUi::new(game.script_manager().scene_path_cell());
        game.refresh_ui_textures();
        super::super::sync_ui_textures(&mut ui, game);
        ui.viewport_tab = ViewportTab::Scene;
        pose(&mut ui, game);
        let ctx = egui::Context::default();
        Some(Self {
            host,
            egui,
            ctx,
            ui,
            texture: None,
            target,
            probe,
            size,
            drawn: 0,
        })
    }

    /// Draw one editor frame and wait for it: its UI time, the viewport frame
    /// when one rendered, and the GPU's speed over the frame.
    fn frame(
        &mut self,
        game: &GameWorld,
    ) -> (
        rusty_dev::bench::scenes::editor::UiTime,
        Option<RenderedFrame>,
        Option<f64>,
    ) {
        let (w, h) = self.size;
        let timer = UiTimer::start();
        self.ctx.begin_pass(super::raw_input(w, h, self.drawn));
        self.drawn += 1;
        let drawn = super::draw_ui(&mut self.ui, &self.ctx, game, false, self.texture);
        let mut output = self.ctx.end_pass();
        let shapes = std::mem::take(&mut output.shapes);
        let jobs = self.ctx.tessellate(shapes, output.pixels_per_point);
        let ui = timer.stop();
        let before = self.probed();
        let host = &mut self.host;
        let rendered = super::viewport::render_viewport(
            host,
            &mut self.egui,
            game,
            &self.ui,
            drawn,
            &mut self.texture,
        );
        let renderer = host.renderer(w, h).expect("built in new");
        let cost = rendered.then(|| {
            (
                renderer.frame_cost().cpu_ms,
                renderer.frame_stages().to_vec(),
                renderer.frame_counters,
            )
        });
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                self.egui
                    .update_texture(&renderer.device, &renderer.queue, *id, delta);
            }
        }
        let painted = Painted {
            jobs: &jobs,
            pixels_per_point: output.pixels_per_point,
        };
        paint_jobs(renderer, &mut self.egui, &painted, &self.target, self.size);
        for id in &output.textures_delta.free {
            self.egui.free_texture(id);
        }
        renderer.wait_idle();
        let gpu = renderer.take_gpu_times();
        let frame = cost.map(|(cpu_ms, stages, counters)| RenderedFrame {
            counters,
            cpu_ms,
            stages,
            gpu,
        });
        let after = self.probed();
        (ui, frame, gpu_speed(before, after))
    }

    /// The GPU clock probe's time, `None` without one.
    fn probed(&mut self) -> Option<f64> {
        let (w, h) = self.size;
        let probe = self.probe.as_ref()?;
        probe.time(self.host.renderer(w, h)?)
    }
}

/// Select the scene's first screen camera, and pose the Scene view at the
/// scene's first game viewpoint (its first camera, or the fallbacks after it).
fn pose(ui: &mut EditorUi, game: &GameWorld) {
    let scene = game.scene().borrow();
    let views = rusty_dev::bench::scenes::views::pick(&scene, &game.resources.nav.borrow());
    let mut cameras: Vec<u32> = rusty_scene::camera::active_cameras(&scene)
        .filter(|(_, target)| target.is_none())
        .map(|(id, _)| id)
        .collect();
    cameras.sort_unstable();
    ui.selected_entity_id = cameras.first().copied();
    if let Some(view) = views.views.first() {
        ui.scene_view.camera = SceneCamera::from_camera(&Camera::new(view.at, view.yaw, 0.0));
    }
}
