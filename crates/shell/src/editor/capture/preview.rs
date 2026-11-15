//! crates/shell/src/editor/capture/preview.rs — the Inspector's Preview tab in a
//! capture (#2239).
//!
//! `--preview [<mesh>]` opens the tab on whatever `--select`/`--select-asset` shows,
//! and every frame renders the tab's request through the window's own
//! `viewport::render_preview_scene`, so the tab shows the image the editor would.

use rusty_editor::{EditorUi, PreviewMesh};
use rusty_render::RenderView;

/// `--preview`'s mesh name, in any case; an unknown one is an error listing the meshes.
pub fn parse_mesh(name: &str) -> Result<PreviewMesh, String> {
    PreviewMesh::parse(name).ok_or_else(|| {
        format!(
            "--preview: no mesh {name:?} (one of {})",
            PreviewMesh::names()
        )
    })
}

/// Open the Preview tab on `mesh` before the first frame: an error with nothing
/// selected, which has no tab to open.
pub(super) fn open(ui: &mut EditorUi, mesh: Option<PreviewMesh>) -> Result<(), String> {
    let Some(mesh) = mesh else {
        return Ok(());
    };
    if ui.selected_entity_id.is_none() && ui.selected_asset_path.is_none() {
        return Err(no_tab("nothing is selected"));
    }
    (ui.preview_tab_active, ui.preview_mesh) = (true, mesh);
    Ok(())
}

/// The tab's offscreen target across a capture's frames, and whether the tab
/// asked for a render in any of them.
#[derive(Default)]
pub(super) struct PreviewTarget {
    view: Option<RenderView>,
    texture_id: Option<egui::TextureId>,
    pub(super) shown: bool,
}

impl PreviewTarget {
    /// Hand the last frame's preview texture to the tab, as the window does
    /// before `EditorUi::draw`.
    pub(super) fn bind(&self, ui: &mut EditorUi) {
        ui.preview_texture_id = self.texture_id;
    }

    /// Render the tab's request of this frame, if it made one.
    pub(super) fn render(
        &mut self,
        gpu: (&mut rusty_render::Renderer, &mut egui_wgpu::Renderer),
        ui: &mut EditorUi,
    ) {
        self.shown |= ui.preview_request.is_some();
        let target = (&mut self.view, &mut self.texture_id);
        super::super::viewport::render_preview_scene(gpu, ui, target, 1.0);
    }
}

/// `--preview` over a selection whose card has no Preview tab (an entity with no
/// Material, an audio or scene asset): an error, never a capture without it.
pub(super) fn no_tab(why: &str) -> String {
    format!(
        "--preview: {why}; select an entity with a Material, or a model, texture or shader asset"
    )
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod preview_tests;
