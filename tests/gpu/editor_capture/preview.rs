//! The Inspector's Preview tab in a capture (#2239): `--preview` opens it and the
//! capture renders its image through the window's own call, so the tab shows a
//! frame, not the neutral fill it shows before one exists.

use rusty::app::ProjectSource;
use rusty::editor::theme::Theme;
use rusty::editor::PreviewMesh;
use rusty::shell::editor::capture::{self, EditorCaptureOptions};

/// The share of the Preview tab's image rect (the inspector's bottom right, at the
/// default 1600x900) that is neither the pre-render fill nor a panel fill.
fn rendered_share(img: &image::RgbaImage) -> f32 {
    let theme = Theme::dark();
    let fills = [theme.bg_tier0, theme.bg_tier1, theme.bg_tier2];
    let near = |p: [u8; 4], c: [u8; 4]| (0..3).all(|i| p[i].abs_diff(c[i]) <= 3);
    let (mut hits, mut total) = (0u32, 0u32);
    for y in 700..870 {
        for x in 1380..1570 {
            let p = img.get_pixel(x, y).0;
            total += 1;
            hits += u32::from(!fills.iter().any(|c| near(p, c.to_array())));
        }
    }
    hits as f32 / total as f32
}

#[test]
fn the_preview_tab_shows_its_render_on_each_mesh() {
    let game = capture::default_world(ProjectSource::defaults());
    let mut host = rusty::dev::capture::CaptureHost::new();
    for mesh in [PreviewMesh::Sphere, PreviewMesh::Plane] {
        let out = crate::temp::dir().join(format!("{}.png", mesh.name()));
        let opts = EditorCaptureOptions {
            select: Some("Player".to_string()),
            preview: Some(mesh),
            ..Default::default()
        };
        let written = capture::capture_into(&mut host, &game, &out, &opts).expect("captures");
        if !written {
            eprintln!("no GPU adapter — skipping the preview capture");
            return;
        }
        let img = image::open(&out).expect("a readable PNG").to_rgba8();
        let share = rendered_share(&img);
        assert!(share > 0.9, "{mesh:?}: only {share} of the tab is rendered");
    }
}

#[test]
fn preview_on_a_card_without_the_tab_is_an_error() {
    let game = capture::default_world(ProjectSource::defaults());
    let out = crate::temp::dir().join("x.png");
    let opts = EditorCaptureOptions {
        select: Some("Sun".to_string()),
        preview: Some(PreviewMesh::Sphere),
        ..Default::default()
    };
    let err = capture::capture(&game, &out, &opts).unwrap_err();
    assert!(err.contains("no Preview tab"), "{err}");
    assert!(!out.exists(), "no capture is written without the tab");
}
