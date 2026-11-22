//! The Inspector's Preview tab in a capture: `--preview` opens it and the
//! capture renders its image through the window's own call, so the tab shows a
//! frame, not the neutral fill it shows before one exists.

use rusty::app::ProjectSource;
use rusty::editor::theme::Theme;
use rusty::editor::PreviewMesh;
use rusty::shell::editor::capture::{self, EditorCaptureOptions, PreviewRect};

/// The share of the tab's image `rect` (as the capture's layout placed it) that is
/// neither the pre-render fill nor a panel fill.
fn rendered_share(img: &image::RgbaImage, rect: PreviewRect) -> f32 {
    let theme = Theme::dark();
    let fills = [theme.bg_tier0, theme.bg_tier1, theme.bg_tier2];
    let near = |p: [u8; 4], c: [u8; 4]| (0..3).all(|i| p[i].abs_diff(c[i]) <= 3);
    let (mut hits, mut total) = (0u32, 0u32);
    for y in rect.y0..rect.y1.min(img.height()) {
        for x in rect.x0..rect.x1.min(img.width()) {
            let p = img.get_pixel(x, y).0;
            total += 1;
            hits += u32::from(!fills.iter().any(|c| near(p, c.to_array())));
        }
    }
    hits as f32 / total.max(1) as f32
}

/// `--select Player --preview <mesh>`, over `frames` frames.
fn on_player(mesh: PreviewMesh, frames: u32) -> EditorCaptureOptions {
    EditorCaptureOptions {
        select: Some("Player".to_string()),
        preview: Some(mesh),
        frames,
        ..Default::default()
    }
}

#[test]
fn the_preview_tab_shows_its_render_on_each_mesh() {
    let game = capture::default_world(ProjectSource::defaults());
    let mut host = rusty::dev::capture::CaptureHost::new();
    for mesh in [PreviewMesh::Sphere, PreviewMesh::Plane] {
        let out = crate::temp::dir().join(format!("{}.png", mesh.name()));
        let opts = on_player(mesh, capture::DEFAULT_FRAMES);
        let shot = capture::capture_shot(&mut host, &game, &out, &opts).expect("captures");
        let Some(shot) = shot else {
            eprintln!("no GPU adapter — skipping the preview capture");
            return;
        };
        let rect = shot.preview.expect("the tab's image rect");
        assert!(rect.x1 - rect.x0 > 50 && rect.y1 - rect.y0 > 50, "{rect:?}");
        let img = image::open(&out).expect("a readable PNG").to_rgba8();
        let share = rendered_share(&img, rect);
        assert!(share > 0.9, "{mesh:?}: only {share} of the tab is rendered");
    }
}

#[test]
fn a_last_frame_before_the_image_is_rendered_is_an_error() {
    let game = capture::default_world(ProjectSource::defaults());
    let out = crate::temp::dir().join("early.png");
    match capture::capture(&game, &out, &on_player(PreviewMesh::Sphere, 1)) {
        Ok(false) => eprintln!("no GPU adapter — skipping the preview capture"),
        Ok(true) => panic!("a frame with no rendered image was written"),
        Err(err) => {
            assert!(err.contains("before its image was rendered"), "{err}");
            assert!(!out.exists(), "no capture is written without the image");
        }
    }
}

#[test]
fn preview_on_a_card_without_the_tab_is_an_error() {
    let game = capture::default_world(ProjectSource::defaults());
    let out = crate::temp::dir().join("x.png");
    let opts = EditorCaptureOptions {
        select: Some("Sun".to_string()),
        ..on_player(PreviewMesh::Sphere, capture::DEFAULT_FRAMES)
    };
    // Refused before the renderer starts: no adapter is needed to see it.
    let err = capture::capture(&game, &out, &opts).unwrap_err();
    assert!(err.contains("no Preview tab"), "{err}");
    assert!(!out.exists(), "no capture is written without the tab");
}
