//! A kept layout is never stale (#2412): every frame's equals a fresh
//! `UiLayout::compute_in` (`Rig::frame` compares), an element joining or leaving
//! the UI lays it out again, the inputs one by one (`inputs`) and the hierarchy's
//! (`hierarchy`).

use glam::{Vec2, Vec3};

use super::*;
use rusty_components::{
    CanvasComponent, LayoutAxisFit, LayoutElementComponent, LayoutGroupComponent,
};
use rusty_scene::{Camera, Scene};

#[path = "hierarchy_tests.rs"]
mod hierarchy;
#[path = "inputs_tests.rs"]
mod inputs;

/// A canvas holding a group with a sized label, a plain label, and an Image fitted
/// to its texture.
pub(super) struct Ui {
    pub(super) scene: Scene,
    pub(super) canvas: u32,
    pub(super) sized: u32,
    pub(super) plain: u32,
    pub(super) icon: u32,
}

pub(super) fn element(scene: &mut Scene, parent: u32) -> u32 {
    let id = scene.add_entity("E".to_string());
    let rt = RectTransformComponent {
        size_delta: Vec2::new(200.0, 40.0),
        ..Default::default()
    };
    scene.world.set_rect_transform(id, Some(rt));
    scene.set_parent(id, Some(parent)).expect("parent exists");
    id
}

pub(super) fn ui() -> Ui {
    let mut scene = Scene::new();
    let canvas = scene.add_entity("Canvas".to_string());
    scene
        .world
        .set_canvas(canvas, Some(CanvasComponent::default()));
    let group = element(&mut scene, canvas);
    scene
        .world
        .set_layout_group(group, Some(LayoutGroupComponent::default()));
    let [sized, plain] = [group, canvas].map(|parent| {
        let id = element(&mut scene, parent);
        let text = TextComponent {
            text: "AMMO 30".into(),
            ..Default::default()
        };
        scene.world.set_text(id, Some(text));
        id
    });
    let icon = element(&mut scene, canvas);
    let fit = LayoutElementComponent {
        horizontal_fit: LayoutAxisFit::PreferredSize,
        ..Default::default()
    };
    scene.world.set_layout_element(icon, Some(fit));
    let image = ImageComponent {
        texture: Some("icon.png".into()),
        ..Default::default()
    };
    scene.world.set_image(icon, Some(image));
    Ui {
        scene,
        canvas,
        sized,
        plain,
        icon,
    }
}

/// One view's kept layout, laid out frame after frame.
pub(super) struct Rig {
    kept: KeptLayout,
    pub(super) screen: Vec2,
    pub(super) camera: Option<Camera>,
    pub(super) textures: Rc<TextureSizes>,
}

impl Rig {
    pub(super) fn new() -> Self {
        let mut textures = TextureSizes::default();
        textures.set("icon.png", Vec2::splat(64.0));
        Self {
            kept: KeptLayout::default(),
            screen: Vec2::new(1920.0, 1080.0),
            camera: Some(Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0)),
            textures: Rc::new(textures),
        }
    }

    /// Lay `world` out, check it against a fresh layout, and say whether it was
    /// computed.
    pub(super) fn frame(&mut self, world: &World) -> bool {
        let t = Rc::clone(&self.textures);
        let view = match &self.camera {
            Some(cam) => UiView::with_camera(self.screen, cam.clone(), t),
            None => UiView::screen(self.screen, t),
        };
        let kept = self.kept.update(world, &view);
        assert_eq!(*kept, UiLayout::compute_in(world, &view), "a stale layout");
        self.kept.computed()
    }
}

#[test]
fn an_element_joining_or_leaving_the_ui_lays_it_out_again() {
    let (mut hud, mut rig) = (ui(), Rig::new());
    rig.frame(&hud.scene.world);
    let loose = hud.scene.add_entity("Loose".to_string());
    let rt = RectTransformComponent::default();
    hud.scene.world.set_rect_transform(loose, Some(rt));
    assert!(
        !rig.frame(&hud.scene.world),
        "outside any canvas: not laid out"
    );
    hud.scene
        .set_parent(loose, Some(hud.canvas))
        .expect("parent");
    assert!(rig.frame(&hud.scene.world), "re-parented under the canvas");
    hud.scene.set_parent(loose, None).expect("unparent");
    assert!(rig.frame(&hud.scene.world), "taken out again");
    assert!(rig.frame(&ui().scene.world), "another World");
}
