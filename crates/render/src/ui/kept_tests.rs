//! A kept canvas mesh is never stale (#2416): each input a mesh is built from,
//! changed in turn, rebuilds exactly the canvas it touches, and every frame's kept
//! meshes equal a fresh build's byte for byte.

use glam::{Vec2, Vec4};

use super::*;
use rusty_components::{ImageType, RectTransformComponent};
use rusty_scene::Scene;

#[path = "kept_inputs_tests.rs"]
mod inputs;

/// Two canvases: `a` holds a sliced, textured panel with a label under it, `b` one
/// solid bar.
pub(super) struct Hud {
    pub(super) scene: Scene,
    pub(super) a: u32,
    pub(super) panel: u32,
    pub(super) label: u32,
    pub(super) bar: u32,
}

pub(super) fn element(scene: &mut Scene, parent: u32, at: Vec2) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        anchored_position: at,
        size_delta: Vec2::new(300.0, 80.0),
        ..Default::default()
    };
    scene.world.set_rect_transform(id, Some(rt));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

pub(super) fn hud() -> Hud {
    let mut scene = Scene::new();
    let [a, b] = ["A", "B"].map(|n| {
        let id = scene.add_entity(n.to_string());
        scene.world.set_canvas(id, Some(CanvasComponent::default()));
        id
    });
    let panel = element(&mut scene, a, Vec2::ZERO);
    let image = ImageComponent {
        texture: Some("panel.png".into()),
        image_type: ImageType::Sliced,
        border: Vec4::splat(4.0),
        ..Default::default()
    };
    scene.world.set_image(panel, Some(image));
    let label = element(&mut scene, panel, Vec2::new(0.0, 10.0));
    let text = TextComponent {
        text: "AMMO 30".into(),
        ..Default::default()
    };
    scene.world.set_text(label, Some(text));
    let bar = element(&mut scene, b, Vec2::new(0.0, -200.0));
    scene.world.set_image(bar, Some(ImageComponent::default()));
    Hud {
        scene,
        a,
        panel,
        label,
        bar,
    }
}

/// One view's kept meshes, drawn frame after frame.
pub(super) struct Rig {
    kept: KeptMeshes,
    pub(super) screen: Vec2,
    pub(super) texture: Vec2,
    /// A canvas this view does not draw (an editor-mode view's filter).
    pub(super) skip: Option<u32>,
    pub(super) atlases: FontAtlases,
}

impl Rig {
    pub(super) fn new() -> Self {
        let (kept, atlases) = Default::default();
        let (screen, texture) = (Vec2::new(1920.0, 1080.0), Vec2::splat(32.0));
        Self {
            kept,
            atlases,
            screen,
            texture,
            skip: None,
        }
    }

    /// Draw `scene`, check the kept meshes against a fresh build, and return how
    /// many canvases were built.
    pub(super) fn frame(&mut self, scene: &Scene) -> usize {
        let world = &scene.world;
        let layout = UiLayout::compute_in(
            world,
            &rusty_ui::UiView::screen(self.screen, Default::default()),
        );
        let (texture, skip) = (self.texture, self.skip);
        let tex = |_: &str| Some(texture);
        let kept = self.kept.update(
            (world, &layout),
            self.screen,
            (&tex, &mut self.atlases),
            &|c| Some(c) != skip,
        );
        let fresh = build_canvas_meshes(world, &layout, self.screen, &tex, &mut self.atlases);
        let fresh: Vec<_> = fresh
            .into_iter()
            .filter(|m| Some(m.canvas) != skip)
            .collect();
        assert_eq!(kept.len(), fresh.len(), "one mesh per drawn canvas");
        for (k, f) in kept.iter().zip(&fresh) {
            let bytes = |m: &CanvasMesh| bytemuck::cast_slice::<_, u8>(&m.vertices).to_vec();
            assert_eq!(bytes(k), bytes(f), "canvas {}: stale vertices", f.canvas);
            assert_eq!(**k, *f, "canvas {}: stale batches or masks", f.canvas);
        }
        self.kept.built
    }
}

#[test]
fn a_still_hud_builds_nothing_and_one_moving_canvas_builds_alone() {
    let mut hud = hud();
    let mut rig = Rig::new();
    assert_eq!(
        rig.frame(&hud.scene),
        2,
        "the first frame builds every canvas"
    );
    assert_eq!(rig.frame(&hud.scene), 0, "a still HUD builds none");
    for i in 0..3 {
        let mut rt = hud.scene.world.rect_transform_mut(hud.bar).expect("rect");
        rt.anchored_position.x = i as f32 * 10.0 + 1.0;
        drop(rt);
        assert_eq!(rig.frame(&hud.scene), 1, "only the moving canvas");
    }
}
