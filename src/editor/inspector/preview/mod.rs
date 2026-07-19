//! src/editor/inspector/preview/mod.rs — the Inspector's "Preview" tab (#352).
//!
//! Unity/Unreal/Blender's material-preview pattern: an isolated offscreen scene
//! (mesh toggle + procedural sky + a hardcoded light) shown inside the Inspector for
//! models, textures, shaders and materials, with drag-to-orbit + wheel-to-zoom.
//!
//! Split across three files: `scene` builds the isolated [`crate::scene::Scene`]
//! (never the active editor World), `orbit` is the pure camera math, and this file
//! is the egui surface — the tab strip, mesh toggle, and the image widget that turns
//! pointer input into orbit updates. The actual GPU render happens in `main.rs`
//! (`render_preview_scene`), the same two-phase split the Scene/Game viewport uses
//! (`editor/viewport/mod.rs`): this module only lays out the panel and records what
//! to render next; `main.rs` is the only place a `Renderer` is in scope.

pub mod orbit;
pub mod scene;

pub use orbit::OrbitState;
pub use scene::{build_preview_scene, PreviewMesh, PreviewSubject};

use crate::editor::{theme::Theme, EditorUi};
use crate::render::Camera;

/// The rebuilt isolated scene, kept until the subject or mesh choice changes so a
/// held Preview tab doesn't re-import a model/texture from disk every frame.
pub struct PreviewCache {
    key: String,
    pub scene: crate::scene::Scene,
}

/// What `main.rs`'s `render_preview_scene` needs to render this frame: the orbit
/// camera and the image rect size (to size the offscreen target). Recomputed by
/// [`draw`] every frame the Preview tab is visible; `None` otherwise, so the
/// front-end skips the extra render when nobody's looking at it.
pub struct PreviewRequest {
    pub camera: Camera,
    pub size: egui::Vec2,
    /// Set only for [`PreviewSubject::Shader`] — the `.wgsl` path `main.rs` compiles
    /// a one-off pipeline from for this render (#352's shader-preview arm).
    pub shader_override: Option<String>,
}

/// A "Details / Preview" tab strip, shared by the assets dispatch (`assets/mod.rs`)
/// and the Material component card, since materials have no on-disk file to hang a
/// dispatch arm on. Mirrors the Scene/Game strip in `editor/viewport/mod.rs`.
pub fn draw_tab_strip(ui: &mut egui::Ui, active: &mut bool, t: Theme) {
    egui::Frame::none()
        .fill(t.bg_tier1)
        .inner_margin(t.space_xs)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.selectable_label(!*active, "Details").clicked() {
                    *active = false;
                }
                if ui.selectable_label(*active, "Preview").clicked() {
                    *active = true;
                }
            });
        });
}

/// Draw the Preview tab's body: the mesh toggle and the offscreen image, sensing
/// drag (orbit) and scroll (zoom). Rebuilds the isolated scene when `subject`'s
/// identity or the mesh choice changed since last frame; otherwise reuses the
/// cached one. Reads `editor.preview_texture_id` — the previous frame's rendered
/// preview (`None` before the first render), set by `main.rs` before `EditorUi::draw`
/// runs — same one-frame-behind contract as the main viewport's `viewport_texture`.
pub fn draw(ui: &mut egui::Ui, editor: &mut EditorUi, subject: PreviewSubject) {
    let t = editor.theme;
    let texture = editor.preview_texture_id;
    mesh_toggle(ui, editor);

    let key = format!("{:?}|{}", editor.preview_mesh, subject.cache_key());
    let stale = editor.preview_cache.as_ref().is_none_or(|c| c.key != key);
    if stale {
        editor.preview_cache = Some(PreviewCache {
            key,
            scene: build_preview_scene(editor.preview_mesh, &subject),
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
    editor
        .preview_orbit
        .apply_input(response.drag_delta(), scroll);

    editor.preview_request = Some(PreviewRequest {
        camera: editor.preview_orbit.camera(),
        size: response.rect.size(),
        shader_override,
    });
}

/// The sphere/cube/Suzanne toggle row.
fn mesh_toggle(ui: &mut egui::Ui, editor: &mut EditorUi) {
    ui.horizontal(|ui| {
        for mesh in PreviewMesh::ALL {
            if ui
                .selectable_label(editor.preview_mesh == mesh, mesh.label())
                .clicked()
            {
                editor.preview_mesh = mesh;
            }
        }
    });
    ui.add_space(4.0);
}

/// The offscreen preview image, filling the remaining space. Before the first
/// render exists, a neutral fill stands in — identical contract to the main
/// viewport's `draw_image` (`editor/viewport/mod.rs`).
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
