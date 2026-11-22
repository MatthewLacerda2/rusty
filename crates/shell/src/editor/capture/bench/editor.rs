//! The editor the round draws (`super`): its UI, egui and the renderer they
//! share, one frame at a time, probed or not.

use rusty_app::GameWorld;
use rusty_dev::bench::scenes::editor::{gpu_speed, Drawn, UiTime, UiTimer};
use rusty_dev::bench::scenes::views::View;
use rusty_dev::capture::{CaptureHost, RenderedFrame};
use rusty_editor::viewport::scene_camera::SceneCamera;
use rusty_editor::{EditorUi, ViewportInteraction, ViewportTab};
use rusty_render::{ClockProbe, OFFSCREEN_FORMAT};
use rusty_scene::camera::camera_for_entity;
use rusty_scene::Camera;

use super::super::paint::{paint_jobs, target, Painted};
use super::Round;

/// The editor the round draws: its UI, egui, and the renderer they share.
pub(super) struct Editor {
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
    pub(super) fn new(game: &GameWorld, opts: &Round) -> Option<Self> {
        let size = (opts.size.0.max(1), opts.size.1.max(1));
        let mut host = CaptureHost::new();
        let renderer = host.renderer(size.0, size.1)?;
        renderer.set_switches(opts.switches);
        if opts.warm {
            renderer.warm_pipelines(true);
        }
        let egui = egui_wgpu::Renderer::new(
            &renderer.device,
            OFFSCREEN_FORMAT,
            super::super::super::paint::EGUI_RENDERER,
        );
        let target = target(&renderer.device, size.0, size.1);
        let probe = ClockProbe::new(renderer);
        let mut ui = EditorUi::new(game.script_manager().scene_path_cell());
        game.refresh_ui_textures();
        super::super::super::sync_ui_textures(&mut ui, game);
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

    pub(super) fn frame(
        &mut self,
        game: &mut GameWorld,
    ) -> (UiTime, Option<Drawn>, Option<f64>, f64) {
        let mut timer = UiTimer::start();
        game.edit_frame();
        timer.edit_done();
        let (output, jobs, drawn) = self.ui(game);
        let ui = timer.stop();
        let before = self.probed();
        let drawing = rusty_dev::stats::now();
        let cost = self.render(game, output, &jobs, drawn);
        let (w, h) = self.size;
        let renderer = self.host.renderer(w, h).expect("built in new");
        renderer.wait_idle();
        let wall_ms = ui.edit_ms + ui.ms + rusty_dev::stats::ms_since(drawing);
        let gpu = renderer.take_gpu_times();
        let frame = cost.map(|(cpu_ms, stages, counters, view)| Drawn {
            frame: RenderedFrame {
                counters,
                cpu_ms,
                stages,
                gpu,
            },
            view,
        });
        let after = self.probed();
        (ui, frame, gpu_speed(before, after), wall_ms)
    }

    pub(super) fn draw(&mut self, game: &mut GameWorld) {
        game.edit_frame();
        let (output, jobs, drawn) = self.ui(game);
        self.render(game, output, &jobs, drawn);
        let (w, h) = self.size;
        self.host.renderer(w, h).expect("built in new").wait_idle();
    }

    /// The editor's UI for one frame, tessellated, and the viewport it laid out.
    fn ui(
        &mut self,
        game: &GameWorld,
    ) -> (
        egui::FullOutput,
        Vec<egui::ClippedPrimitive>,
        ViewportInteraction,
    ) {
        let (w, h) = self.size;
        self.ctx
            .begin_pass(super::super::raw_input(w, h, self.drawn));
        self.drawn += 1;
        let drawn = super::super::draw_ui(&mut self.ui, &self.ctx, game, false, self.texture);
        let mut output = self.ctx.end_pass();
        let shapes = std::mem::take(&mut output.shapes);
        let jobs = self.ctx.tessellate(shapes, output.pixels_per_point);
        (output, jobs, drawn)
    }

    fn render(
        &mut self,
        game: &GameWorld,
        output: egui::FullOutput,
        jobs: &[egui::ClippedPrimitive],
        drawn: ViewportInteraction,
    ) -> Option<Cost> {
        let (w, h) = self.size;
        let host = &mut self.host;
        let (egui, texture) = (&mut self.egui, &mut self.texture);
        let rendered =
            super::super::viewport::render_viewport(host, egui, game, &self.ui, drawn, texture);
        let renderer = host.renderer(w, h).expect("built in new");
        let cost = rendered.map(|view| {
            let cpu_ms = renderer.frame_cost().cpu_ms;
            let stages = renderer.frame_stages().to_vec();
            (cpu_ms, stages, renderer.frame_counters, view)
        });
        self.paint(output, jobs);
        cost
    }

    /// Upload the frame's egui textures and paint its tessellated `jobs` over the
    /// editor's target, as the window paints its surface.
    fn paint(&mut self, mut output: egui::FullOutput, jobs: &[egui::ClippedPrimitive]) {
        let (w, h) = self.size;
        let renderer = self.host.renderer(w, h).expect("built in new");
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                let (device, queue) = (&renderer.device, &renderer.queue);
                self.egui.update_texture(device, queue, *id, delta);
            }
        }
        let painted = Painted {
            jobs,
            pixels_per_point: output.pixels_per_point,
        };
        paint_jobs(renderer, &mut self.egui, &painted, &self.target, self.size);
        for id in &output.textures_delta.free {
            self.egui.free_texture(id);
        }
        output.textures_delta.clear();
    }

    #[cfg(test)]
    pub(super) fn editor_ui(&mut self) -> &mut EditorUi {
        &mut self.ui
    }

    /// The GPU clock probe's time, `None` without one.
    fn probed(&mut self) -> Option<f64> {
        let (w, h) = self.size;
        let probe = self.probe.as_ref()?;
        probe.time(self.host.renderer(w, h)?)
    }
}

type Cost = (
    f64,
    Vec<(&'static str, f64)>,
    rusty_render::RenderCounters,
    (u32, u32),
);

pub(super) fn pose(ui: &mut EditorUi, game: &GameWorld) {
    let scene = game.scene().borrow();
    let views = rusty_dev::bench::scenes::views::pick(&scene, &game.resources.nav.borrow());
    let mut cameras: Vec<u32> = rusty_scene::camera::active_cameras(&scene)
        .filter(|(_, target)| target.is_none())
        .map(|(id, _)| id)
        .collect();
    cameras.sort_unstable();
    ui.selected_entity_id = cameras.first().copied();
    let base = Camera::new(glam::Vec3::ZERO, 0.0, 0.0);
    let at = views.views.first().and_then(|view| match view.view {
        View::Camera(id) => camera_for_entity(&base, &scene, id),
        View::Pose {
            position,
            yaw,
            pitch,
        } => Some(Camera::new(position, yaw, pitch)),
    });
    if let Some(at) = at {
        ui.scene_view.camera = SceneCamera::from_camera(&at);
    }
}
