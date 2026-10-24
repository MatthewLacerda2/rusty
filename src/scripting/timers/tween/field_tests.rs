//! Every animatable field (#424) reads and writes its component through the
//! authoring ops, and a missing component reads `None` and refuses the write.

use glam::{Vec2, Vec3, Vec4};

use super::super::{Ease, Property, Tween};
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

#[test]
fn a_loaded_fov_past_its_bound_tweens_from_the_first_step() {
    let (mut s, id) = dressed();
    // A scene file skips the op: the stored lens is out of Unity's bounds.
    s.world.camera_mut(id).expect("camera").fov = 500.0;
    let fov = Field::Fov;
    let from = fov.get(&s, id).expect("camera");
    assert_eq!(from.x, 179.0, "the start is the held value");
    let tween = Tween {
        property: Property::Field(fov),
        from: None,
        to: Vec4::splat(60.0),
        duration: 1.0,
        ease: Ease::Linear,
        loops: Some(1),
        yoyo: false,
        on_complete: None,
        group: None,
    };
    let (first, _) = tween.sample(from, 0.1);
    assert!(fov.set(&mut s, id, first));
    let after = s.world.camera(id).expect("camera").fov;
    assert!(after < 179.0, "the first step moves the lens, read {after}");
}

#[test]
fn every_clamped_field_reads_its_stored_value_as_held() {
    let (mut s, id) = dressed();
    let w = &mut s.world;
    w.canvas_group_mut(id).expect("group").alpha = 3.0;
    w.image_mut(id).expect("image").fill_amount = -1.0;
    w.light_mut(id).expect("light").range = -2.0;
    w.audio_mut(id).expect("audio").volume = 4.0;
    w.rect_transform_mut(id).expect("rect").anchor_min = Vec2::splat(2.0);
    for (field, held) in [
        (Field::Alpha, 1.0),
        (Field::FillAmount, 0.0),
        (Field::Range, 0.0),
        (Field::Volume, 1.0),
        (Field::AnchorMin, 1.0),
    ] {
        let got = field.get(&s, id).expect(field.path()).x;
        assert_eq!(got, held, "{}", field.path());
    }
}
