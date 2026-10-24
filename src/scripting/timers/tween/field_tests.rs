//! Every animatable field (#424) reads and writes its component through the
//! authoring ops, and a missing component reads `None` and refuses the write.

use glam::{Vec3, Vec4};

use super::{Field, FIELDS};
use crate::components::{
    AudioSourceComponent, CanvasGroupComponent, ImageComponent, RectTransformComponent,
    TextComponent,
};
use crate::scene::authoring::{default_camera, default_light};
use crate::scene::Scene;

/// An entity carrying every component a tween can drive.
fn dressed() -> (Scene, u32) {
    let mut s = Scene::new();
    let id = s.add_entity("Dressed".to_string());
    let w = &mut s.world;
    w.set_rect_transform(id, Some(RectTransformComponent::default()));
    w.set_canvas_group(id, Some(CanvasGroupComponent::default()));
    w.set_image(id, Some(ImageComponent::default()));
    w.set_text(id, Some(TextComponent::default()));
    w.set_light(id, Some(default_light()));
    w.set_camera(id, Some(default_camera()));
    w.set_audio(id, Some(AudioSourceComponent::default()));
    (s, id)
}

/// `v` with only the first `arity` lanes kept.
fn lanes(v: Vec4, arity: usize) -> Vec4 {
    Vec4::from_array(std::array::from_fn(|i| if i < arity { v[i] } else { 0.0 }))
}

#[test]
fn every_field_reads_back_what_it_wrote() {
    let (mut s, id) = dressed();
    for &(path, field, arity) in FIELDS {
        assert_eq!(Field::parse(path), Some(field));
        assert_eq!(field.path(), path);
        assert_eq!(field.arity(), arity);
        // In range for every clamp (unit colours, anchors, alpha, a font's minimum,
        // a lens's 1–179° field of view).
        let probe = match path {
            "Camera.fov" => Vec4::splat(60.0),
            _ => Vec4::new(0.75, 0.5, 0.25, 0.625),
        };
        let v = lanes(probe, arity);
        assert!(field.set(&mut s, id, v), "{path} writes");
        let got = lanes(field.get(&s, id).expect(path), arity);
        assert!(got.abs_diff_eq(v, 1e-4), "{path}: wrote {v}, read {got}");
    }
}

#[test]
fn writes_go_through_the_authoring_clamps() {
    let (mut s, id) = dressed();
    let over = Vec4::splat(3.0);
    for (path, clamped) in [
        ("CanvasGroup.alpha", 1.0),
        ("Image.fill_amount", 1.0),
        ("Image.color", 1.0),
        ("Text.color", 1.0),
    ] {
        let field = Field::parse(path).expect(path);
        assert!(field.set(&mut s, id, over));
        assert_eq!(field.get(&s, id).expect(path).x, clamped, "{path}");
    }
    let intensity = Field::parse("Light.intensity").expect("intensity");
    intensity.set(&mut s, id, Vec4::splat(-2.0));
    assert_eq!(
        intensity.get(&s, id).expect("light").x,
        0.0,
        "no negative light"
    );
}

#[test]
fn a_missing_component_reads_none_and_refuses_the_write() {
    let mut s = Scene::new();
    let bare = s.add_entity("Bare".to_string());
    for &(path, field, _) in FIELDS {
        let on_transform = path.starts_with("Transform.");
        assert_eq!(field.get(&s, bare).is_some(), on_transform, "{path}");
        assert_eq!(field.set(&mut s, bare, Vec4::ONE), on_transform, "{path}");
    }
    let position = Field::parse("Transform.position").expect("position");
    assert_eq!(
        s.world.transform(bare).expect("transform").position,
        Vec3::ONE
    );
    assert!(
        position.get(&s, bare + 99).is_none(),
        "a dead id reads None"
    );
    assert!(Field::parse("Light.colour").is_none());
    assert!(Field::paths().contains("Camera.fov"));
}
