//! crates/editor/src/inspector/preview.rs — the Inspector's "Preview" tab.

pub use rusty_preview::{
    build_preview_scene, framing, subject_for_path, FramedOrbit, OrbitState, PreviewMesh,
    PreviewSubject,
};

use crate::theme::{chrome, Theme};
use crate::EditorUi;
use rusty_scene::Camera;

/// The rebuilt isolated scene, kept until the subject or mesh choice changes so a
/// held Preview tab doesn't re-import a model/texture from disk every frame.
pub struct PreviewCache {
    key: String,
    pub scene: rusty_scene::Scene,
}

/// What `shell::editor::viewport::render_preview_scene` needs to render this frame: the orbit
/// camera and the image rect (its size sizes the offscreen target; `editor-capture`
pub struct PreviewRequest {
    pub camera: Camera,
    /// The image's rect, in points, as this frame laid it out.
    pub rect: egui::Rect,
    /// Set only for [`PreviewSubject::Shader`] — the `.wgsl` path the shell compiles
    /// a one-off pipeline from for this render.
    pub shader_override: Option<String>,
}

/// A "Details / Preview" tab strip, shared by the assets dispatch (`assets/mod.rs`)
pub fn draw_tab_strip(ui: &mut egui::Ui, active: &mut bool, t: Theme) {
    ui.horizontal(|ui| {
        if chrome::tab(ui, &t, !*active, "Details").clicked() {
            *active = false;
        }
        if chrome::tab(ui, &t, *active, "Preview").clicked() {
            *active = true;
        }
    });
    ui.add_space(t.space_xs);
}

pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, subject: PreviewSubject) {
    let t = editor.theme;
    let texture = editor.preview_texture_id;
    mesh_toggle(ui, editor);
    editor
        .preview_orbit
        .frame(framing(editor.preview_mesh, &subject));

    let key = format!("{:?}|{}", editor.preview_mesh, subject.cache_key());
    let stale = editor.preview_cache.as_ref().is_none_or(|c| c.key != key);
    if stale {
        editor.preview_cache = Some(PreviewCache {
            key,
            scene: build_preview_scene(editor.preview_mesh, None, &subject)
                .unwrap_or_else(|_| rusty_scene::Scene::new()),
        });
    }

    let shader_override = match &subject {
        PreviewSubject::Shader(path) => Some(path.clone()),
        _ => None,
    };

    let response = draw_image(ui, texture, t);
    let scroll = if response.hovered() {
        ui.input(|i| i.smooth_scroll_delta.y)
    } else {
        0.0
    };
    let drag = response.drag_delta();
    let orbit = &mut editor.preview_orbit.orbit;
    orbit.apply_input(drag.x, drag.y, scroll);

    editor.preview_request = Some(PreviewRequest {
        camera: orbit.camera(),
        rect: response.rect,
        shader_override,
    });
}

fn mesh_toggle(ui: &mut egui::Ui, editor: &mut EditorUi) {
    ui.horizontal(|ui| {
        for mesh in PreviewMesh::ALL {
            if ui
                .selectable_label(editor.preview_mesh == mesh, mesh.name())
                .clicked()
            {
                editor.preview_mesh = mesh;
            }
        }
    });
    ui.add_space(4.0);
}

fn draw_image(ui: &mut egui::Ui, texture: Option<egui::TextureId>, t: Theme) -> egui::Response {
    let size = ui.available_size();
    match texture {
        Some(id) => ui.add(
            egui::Image::new((id, size))
                .sense(egui::Sense::click_and_drag())
                .maintain_aspect_ratio(false),
        ),
        None => {
            let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
            ui.painter().rect_filled(rect, 0.0, t.bg_tier0);
            response
        }
    }
}
