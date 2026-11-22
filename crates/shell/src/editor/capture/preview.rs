//! crates/shell/src/editor/capture/preview.rs — the Inspector's Preview tab in a
//! capture.

use rusty_editor::inspector::preview::subject_for_path;
use rusty_editor::{EditorUi, PreviewMesh};
use rusty_render::RenderView;
use rusty_scene::Scene;

/// `--preview`'s mesh name, in any case; an unknown one is an error listing the meshes.
pub fn parse_mesh(name: &str) -> Result<PreviewMesh, String> {
    PreviewMesh::parse(name).ok_or_else(|| {
        format!(
            "--preview: no mesh {name:?} (one of {})",
            PreviewMesh::names()
        )
    })
}

/// `--preview` beside `--picker`: refused, since the project picker has no Inspector.
pub fn refuse_with_picker(preview: Option<PreviewMesh>) -> Result<(), String> {
    match preview {
        Some(_) => Err(
            "--preview opens an Inspector tab, which the project picker \
                        (--picker) has none of: drop one of the two"
                .to_string(),
        ),
        None => Ok(()),
    }
}

pub(super) fn open(
    ui: &mut EditorUi,
    scene: &Scene,
    mesh: Option<PreviewMesh>,
) -> Result<(), String> {
    let Some(mesh) = mesh else {
        return Ok(());
    };
    match (ui.selected_entity_id, &ui.selected_asset_path) {
        (None, None) => return Err(no_tab("nothing is selected")),
        (Some(id), _) if scene.world.material(id).is_none() => {
            return Err(no_tab("the selected entity has no Material"));
        }
        (_, Some(path)) if subject_for_path(path).is_none() => {
            return Err(no_tab(&format!("the asset {path:?} has no preview")));
        }
        _ => {}
    }
    (ui.preview_tab_active, ui.preview_mesh) = (true, mesh);
    Ok(())
}

/// The Preview tab's image rect in the written PNG, in pixels: `x0..x1` × `y0..y1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewRect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

#[derive(Clone, Copy, Debug, Default)]
enum Frame {
    #[default]
    NoRequest,
    Unrendered,
    Shown(egui::Rect),
}

#[derive(Default)]
pub(super) struct PreviewTarget {
    view: Option<RenderView>,
    texture_id: Option<egui::TextureId>,
    last: Frame,
}

impl PreviewTarget {
    /// Hand the last frame's preview texture to the tab, as the window does
    /// before `EditorUi::draw`.
    pub(super) fn bind(&self, ui: &mut EditorUi) {
        ui.preview_texture_id = self.texture_id;
    }

    pub(super) fn render(
        &mut self,
        gpu: (&mut rusty_render::Renderer, &mut egui_wgpu::Renderer),
        ui: &mut EditorUi,
    ) {
        self.last = match (&ui.preview_request, self.texture_id) {
            (None, _) => Frame::NoRequest,
            (Some(_), None) => Frame::Unrendered,
            (Some(request), Some(_)) => Frame::Shown(request.rect),
        };
        let target = (&mut self.view, &mut self.texture_id);
        super::super::viewport::render_preview_scene(gpu, ui, target, 1.0);
    }

    /// The written frame's image rect, clipped to the `width`×`height` capture: an
    /// error when that frame drew no rendered image.
    pub(super) fn shown(&self, width: u32, height: u32) -> Result<PreviewRect, String> {
        let screen =
            egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32, height as f32));
        let rect = match self.last {
            Frame::Shown(rect) => rect.intersect(screen),
            Frame::NoRequest => return Err(no_tab("the last frame drew no Preview tab")),
            Frame::Unrendered => {
                return Err(
                    "--preview: the last frame drew the tab before its image was \
                            rendered (a zero-size image rect, or too few --frames)"
                        .to_string(),
                )
            }
        };
        let shown = PreviewRect {
            x0: rect.min.x.ceil() as u32,
            y0: rect.min.y.ceil() as u32,
            x1: rect.max.x.floor() as u32,
            y1: rect.max.y.floor() as u32,
        };
        if shown.x0 >= shown.x1 || shown.y0 >= shown.y1 {
            return Err(format!(
                "--preview: the tab's image is empty on screen ({rect:?})"
            ));
        }
        Ok(shown)
    }
}

/// `--preview` over a selection whose card has no Preview tab: an error, never a
/// capture without it.
fn no_tab(why: &str) -> String {
    format!(
        "--preview: {why}, so its card has no Preview tab; select an entity with a \
         Material, or a model, texture or shader asset"
    )
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod preview_tests;
