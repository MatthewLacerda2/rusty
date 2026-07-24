//! src/preview/ — the asset-preview machinery, shared by the editor and the agent.
//!
//! One isolated preview scene (#352) with two front-ends: the Inspector's Preview tab
//! (`editor/inspector/preview.rs`, human-facing, with drag-to-orbit) and the headless
//! `Debug.Preview` capture (`dev/preview.rs`, agent-facing, fixed framing) — the same
//! "one surface, three callers" rule the scripting API follows (#353).
//!
//! Nothing here touches egui, wgpu or the active editor World: [`scene`] builds a
//! brand-new throwaway [`crate::scene::Scene`] and [`orbit`] is pure camera math. That
//! is the point of the module living outside `editor/` — the headless path must not
//! reach into the UI tree to render a preview, or the two front-ends drift into two
//! preview scenes.
//!
//! Allowed deps: asset, components, render (for `Camera`), scene.

pub mod orbit;
pub mod scene;

pub use orbit::OrbitState;
pub use scene::{build_preview_scene, PreviewMesh, PreviewSubject, SUZANNE_PATH};

/// The preview subject for a file extension, or `None` for a kind with no preview
/// story (audio/scene/prefab/lua/unsupported). The single dispatch table both
/// front-ends use, so the editor tab and `Debug.Preview` accept exactly the same set
/// of assets — a new previewable extension is one arm here, not one per front-end.
///
/// `extension` is matched case-insensitively. [`PreviewSubject::Material`] is absent
/// by construction: a material asset lives in `Scene::materials` and has no file on
/// disk to dispatch on, so it is only ever selected directly (the Material component
/// card builds it from the entity's `MaterialComponent`).
pub fn subject_for_path(path: &str) -> Option<PreviewSubject> {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match extension.as_str() {
        "png" | "tga" | "jpg" | "jpeg" => Some(PreviewSubject::Texture(path.to_string())),
        "fbx" | "obj" | "gltf" | "glb" => Some(PreviewSubject::Model(path.to_string())),
        "wgsl" => Some(PreviewSubject::Shader(path.to_string())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_covers_the_three_file_backed_kinds() {
        assert!(matches!(
            subject_for_path("a/b.png"),
            Some(PreviewSubject::Texture(_))
        ));
        assert!(matches!(
            subject_for_path("a/b.glb"),
            Some(PreviewSubject::Model(_))
        ));
        assert!(matches!(
            subject_for_path("a/b.wgsl"),
            Some(PreviewSubject::Shader(_))
        ));
    }

    #[test]
    fn dispatch_is_case_insensitive_and_rejects_unpreviewable() {
        assert!(matches!(
            subject_for_path("A/B.PNG"),
            Some(PreviewSubject::Texture(_))
        ));
        assert!(subject_for_path("a/b.lua").is_none());
        assert!(subject_for_path("a/b").is_none());
    }
}
