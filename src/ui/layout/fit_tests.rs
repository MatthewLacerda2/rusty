//! Content-fit tests (#421): a wrapping text box that grows downward, a label
//! sized to its string, an image at its native size, a list that grows with its
//! items, and text sizing a group's child.

use glam::Vec2;

use super::super::fixture::{assert_rect, preferred, Fixture};
use crate::components::{
    ImageComponent, LayoutAxisFit, LayoutElementComponent, LayoutGroupComponent, LayoutKind,
    TextComponent,
};
use crate::ui::text::preferred_size;

fn fitter(h: LayoutAxisFit, v: LayoutAxisFit) -> LayoutElementComponent {
    LayoutElementComponent {
        horizontal_fit: h,
        vertical_fit: v,
        ..LayoutElementComponent::default()
    }
}

fn sentence() -> TextComponent {
    TextComponent {
        text: "the quick brown fox jumps over the lazy dog".to_string(),
        font_size: 32.0,
        ..TextComponent::default()
    }
}

/// A bare panel (no group) carrying `le`.
fn panel(size: Vec2, le: LayoutElementComponent) -> Fixture {
    let mut f = Fixture::new(size, LayoutGroupComponent::default());
    f.scene.world.set_layout_group(f.panel, None);
    f.scene.world.set_layout_element(f.panel, Some(le));
    f
}

#[test]
fn a_wrapping_text_box_grows_to_its_wrapped_height_around_its_pivot() {
    let mut f = panel(
        Vec2::new(200.0, 10.0),
        fitter(LayoutAxisFit::Unconstrained, LayoutAxisFit::PreferredSize),
    );
    f.scene.world.set_text(f.panel, Some(sentence()));
    let h = preferred_size(&sentence(), 200.0).y;
    assert!(h > 64.0, "wraps onto several lines: {h}");
    // The pivot (centre) stays put: the box grows up and down by (h - 10) / 2.
    assert_rect(f.rect(f.panel), [0.0, 5.0 - h / 2.0], [200.0, h]);
}

#[test]
fn a_label_fits_its_unwrapped_string() {
    let mut f = panel(
        Vec2::new(10.0, 10.0),
        fitter(LayoutAxisFit::PreferredSize, LayoutAxisFit::PreferredSize),
    );
    f.scene.world.set_text(f.panel, Some(sentence()));
    let want = preferred_size(&sentence(), f32::MAX);
    assert!((f.rect(f.panel).1 - want).abs().max_element() < 1e-3);
}

#[test]
fn an_image_fits_its_native_texture_size() {
    let path = crate::test_temp::dir().join("rusty_421_native_64x32.png");
    image::RgbaImage::new(64, 32)
        .save(&path)
        .expect("write png");
    let mut f = panel(
        Vec2::ZERO,
        fitter(LayoutAxisFit::PreferredSize, LayoutAxisFit::PreferredSize),
    );
    let img = ImageComponent {
        texture: Some(path.to_string_lossy().into_owned()),
        ..ImageComponent::default()
    };
    f.scene.world.set_image(f.panel, Some(img));
    assert_rect(f.rect(f.panel), [-32.0, -16.0], [64.0, 32.0]);
}

#[test]
fn a_fitted_list_grows_with_its_items() {
    let g = LayoutGroupComponent {
        kind: LayoutKind::Vertical,
        spacing: Vec2::new(0.0, 5.0),
        child_force_expand_height: false,
        ..LayoutGroupComponent::default()
    };
    let mut f = Fixture::new(Vec2::new(100.0, 0.0), g);
    let fit = fitter(LayoutAxisFit::Unconstrained, LayoutAxisFit::PreferredSize);
    f.scene.world.set_layout_element(f.panel, Some(fit));
    for _ in 0..3 {
        f.child(f.panel, preferred(None, Some(20.0)), Vec2::ZERO);
    }
    assert_rect(f.rect(f.panel), [0.0, -35.0], [100.0, 70.0]);
    f.child(f.panel, preferred(None, Some(20.0)), Vec2::ZERO);
    assert_rect(f.rect(f.panel), [0.0, -47.5], [100.0, 95.0]);
}

#[test]
fn a_column_sizes_wrapping_text_at_the_width_it_gives_it() {
    let g = LayoutGroupComponent {
        kind: LayoutKind::Vertical,
        child_force_expand_height: false,
        ..LayoutGroupComponent::default()
    };
    let mut f = Fixture::new(Vec2::new(150.0, 400.0), g);
    let label = f.child(f.panel, None, Vec2::ZERO);
    f.scene.world.set_text(label, Some(sentence()));
    let h = preferred_size(&sentence(), 150.0).y;
    assert_rect(f.rect(label), [0.0, 400.0 - h], [150.0, h]);
}
