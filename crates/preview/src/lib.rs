pub mod orbit;
pub mod scene;
pub mod stand_in;

pub use orbit::{FramedOrbit, OrbitState};
pub use scene::{build_preview_scene, PreviewMesh, PreviewSubject};
pub use stand_in::framing;

/// The one extension dispatch: the editor tab and `Debug.Preview` both call it, so
/// they accept the same files. A material has no file: it is picked directly.
pub fn subject_for_path(path: &str) -> Option<PreviewSubject> {
    if rusty_asset::is_image_path(path) {
        Some(PreviewSubject::Texture(path.to_string()))
    } else if rusty_asset::is_model_path(path) {
        Some(PreviewSubject::Model(path.to_string()))
    } else if rusty_core::paths::has_extension(path, "wgsl") {
        Some(PreviewSubject::Shader(path.to_string()))
    } else {
        None
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
