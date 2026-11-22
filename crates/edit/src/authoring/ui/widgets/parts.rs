use glam::{Vec2, Vec4};

use rusty_components::{
    ImageComponent, RectTransformComponent, ScriptFieldValue, SelectableComponent, TextAlignment,
    TextComponent,
};
use rusty_scene::Scene;

pub const SCRIPT_DIR: &str = "assets/scripts/ui";

/// Unity's default UI colours: white panels, dark grey text.
pub const WHITE: Vec4 = Vec4::ONE;
pub const DARK: Vec4 = Vec4::new(0.196, 0.196, 0.196, 1.0);

#[derive(Clone, Copy)]
pub struct Place(pub Vec2, pub Vec2, pub Vec2, pub Vec2, pub Vec2);

impl Place {
    pub fn centred(size: Vec2) -> Self {
        Self(
            Vec2::splat(0.5),
            Vec2::splat(0.5),
            Vec2::splat(0.5),
            Vec2::ZERO,
            size,
        )
    }

    pub fn stretch(min: Vec2, max: Vec2, grow: Vec2) -> Self {
        Self(min, max, Vec2::splat(0.5), Vec2::ZERO, grow)
    }

    pub fn fill(inset: Vec2) -> Self {
        Self::stretch(Vec2::ZERO, Vec2::ONE, -2.0 * inset)
    }

    fn rect(self) -> RectTransformComponent {
        RectTransformComponent {
            anchor_min: self.0,
            anchor_max: self.1,
            pivot: self.2,
            anchored_position: self.3,
            size_delta: self.4,
            world_anchor: None,
        }
    }
}

pub fn node(scene: &mut Scene, name: &str, parent: Option<u32>, place: Place) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.set_rect_transform(id, Some(place.rect()));
    if let Some(p) = parent {
        // A fresh node under a live parent cannot form a cycle.
        let _ = scene.set_parent(id, Some(p));
    }
    id
}

pub fn image(scene: &mut Scene, id: u32, color: Vec4) {
    let img = ImageComponent {
        color,
        ..Default::default()
    };
    scene.world.set_image(id, Some(img));
}

pub fn text(scene: &mut Scene, id: u32, s: &str, size: f32, alignment: TextAlignment) {
    let t = TextComponent {
        text: s.to_string(),
        font_size: size,
        color: DARK,
        alignment,
        wrap: false,
        rich_text: false,
        raycast_target: false,
        ..Default::default()
    };
    scene.world.set_text(id, Some(t));
}

pub fn selectable(scene: &mut Scene, id: u32, target: Option<u32>) {
    let s = SelectableComponent {
        target_graphic: target.filter(|&t| t != id),
        ..Default::default()
    };
    scene.world.set_selectable(id, Some(s));
}

pub fn script(scene: &mut Scene, id: u32, name: &str) {
    script_with(scene, id, name, &[]);
}

pub fn script_with(scene: &mut Scene, id: u32, name: &str, values: &[(&str, ScriptFieldValue)]) {
    let values = values.iter().map(|(k, v)| (k.to_string(), v.clone()));
    let path = format!("{SCRIPT_DIR}/{name}.lua");
    crate::authoring::add_script(&mut scene.world, id, &path, values.collect());
}

pub fn label(
    scene: &mut Scene,
    parent: u32,
    s: &str,
    size: f32,
    align: TextAlignment,
    inset: Vec2,
) -> u32 {
    let id = node(scene, "Label", Some(parent), Place::fill(inset));
    text(scene, id, s, size, align);
    id
}
