//! The look scene the shape screenshot test renders (#425): SDF shapes, a
//! gradient, effects and the blend modes on a canvas whose reference resolution
//! equals the shot (one reference unit = one pixel), over a black backdrop.

use glam::{Vec2, Vec4};
use rusty::components::{
    CanvasComponent, GradientStop, ImageComponent, RectTransformComponent, ShapeComponent,
    ShapeCorner, ShapeGlow, ShapeKind, ShapeShadow, UiBlend, UiGradient,
};
use rusty::scene::Scene;

pub const SIZE: u32 = 256;

/// A shape of `color` with the rest of its settings from `f`.
fn shape(color: [f32; 4], f: impl FnOnce(&mut ShapeComponent)) -> ShapeComponent {
    let mut s = ShapeComponent {
        color: Vec4::from_array(color),
        ..Default::default()
    };
    f(&mut s);
    s
}

/// A bottom-left-anchored element with rect `[x, y, width, height]` under `parent`.
fn at(scene: &mut Scene, parent: u32, r: [f32; 4]) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: Vec2::new(r[0], r[1]),
        size_delta: Vec2::new(r[2], r[3]),
        world_anchor: None,
    };
    scene.world.set_rect_transform(id, Some(rt));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

fn put(scene: &mut Scene, root: u32, r: [f32; 4], s: ShapeComponent) {
    let id = at(scene, root, r);
    scene.world.set_shape(id, Some(s));
}

/// The scene: see `ui_shapes_screenshot` for what each element proves.
pub fn shapes() -> Scene {
    let mut scene = Scene::new();
    let root = scene.add_entity("Canvas".to_string());
    let canvas = CanvasComponent {
        reference_resolution: Vec2::splat(SIZE as f32),
        ..Default::default()
    };
    scene.world.set_canvas(root, Some(canvas));
    let black = ImageComponent {
        color: Vec4::new(0.0, 0.0, 0.0, 1.0),
        ..Default::default()
    };
    let full = at(&mut scene, root, [0.0, 0.0, SIZE as f32, SIZE as f32]);
    scene.world.set_image(full, Some(black));
    add_primitives(&mut scene, root);
    add_gradient_line_glow(&mut scene, root);
    add_blends(&mut scene, root);
    scene
}

/// Row 1: a chamfered red rect with a hard yellow shadow, a rounded green rect, a
/// blue ring; row 2: a white quarter pie.
fn add_primitives(scene: &mut Scene, root: u32) {
    let chamfer = shape([1.0, 0.0, 0.0, 1.0], |s| {
        s.corner = ShapeCorner::Chamfer;
        s.radius = Vec4::splat(20.0);
        s.shadow = ShapeShadow {
            offset: Vec2::new(6.0, -6.0),
            blur: 0.0,
            color: Vec4::new(1.0, 1.0, 0.0, 1.0),
        };
    });
    put(scene, root, [16.0, 16.0, 64.0, 64.0], chamfer);
    let round = shape([0.0, 1.0, 0.0, 1.0], |s| s.radius = Vec4::splat(24.0));
    put(scene, root, [96.0, 16.0, 64.0, 64.0], round);
    let ring = shape([0.0, 0.0, 1.0, 1.0], |s| {
        s.kind = ShapeKind::Ring;
        s.inner_radius = 20.0;
    });
    put(scene, root, [176.0, 16.0, 64.0, 64.0], ring);
    let pie = shape([1.0, 1.0, 1.0, 1.0], |s| {
        s.kind = ShapeKind::Ring;
        (s.arc_start, s.arc_end) = (0.0, 90.0);
    });
    put(scene, root, [16.0, 96.0, 64.0, 64.0], pie);
}

/// Row 2: a red → blue gradient bar, a dashed white line and a glowing white square.
fn add_gradient_line_glow(scene: &mut Scene, root: u32) {
    let stop = |t, c: [f32; 4]| GradientStop {
        t,
        color: Vec4::from_array(c),
    };
    let gradient = shape([1.0; 4], |s| {
        s.gradient = Some(UiGradient {
            stops: vec![
                stop(0.0, [1.0, 0.0, 0.0, 1.0]),
                stop(1.0, [0.0, 0.0, 1.0, 1.0]),
            ],
            ..Default::default()
        });
    });
    put(scene, root, [96.0, 104.0, 96.0, 32.0], gradient);
    let line = shape([1.0; 4], |s| {
        s.kind = ShapeKind::Line;
        (s.thickness, s.dash, s.gap) = (4.0, 8.0, 8.0);
    });
    put(scene, root, [96.0, 144.0, 96.0, 16.0], line);
    let glow = shape([1.0; 4], |s| {
        s.glow = ShapeGlow {
            size: 12.0,
            intensity: 1.0,
            color: Vec4::new(0.0, 1.0, 1.0, 1.0),
        };
    });
    put(scene, root, [208.0, 112.0, 32.0, 32.0], glow);
}

/// Row 3: a mid-grey strip with an additive, a multiply and a screen square on it.
fn add_blends(scene: &mut Scene, root: u32) {
    put(
        scene,
        root,
        [16.0, 176.0, 224.0, 64.0],
        shape([0.5, 0.5, 0.5, 1.0], |_| {}),
    );
    let modes = [
        (UiBlend::Additive, [0.25, 0.0, 0.0, 1.0]),
        (UiBlend::Multiply, [0.5, 1.0, 1.0, 1.0]),
        (UiBlend::Screen, [0.5, 0.0, 0.0, 1.0]),
    ];
    for (i, (blend, color)) in modes.into_iter().enumerate() {
        let x = 24.0 + i as f32 * 72.0;
        put(
            scene,
            root,
            [x, 184.0, 48.0, 48.0],
            shape(color, |s| s.blend = blend),
        );
    }
}
