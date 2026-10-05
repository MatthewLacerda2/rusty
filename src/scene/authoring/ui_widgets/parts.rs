//! src/scene/authoring/ui_widgets/parts.rs — the building blocks every widget uses.
//!
//! A widget is a small tree of plain GameObjects: each node gets a
//! `RectTransform` placed with Unity's anchor semantics, then any of an `Image`,
//! a `Text`, a `Selectable` and the widget's Lua script. Nothing here is
//! widget-specific; the widget files only say which nodes, where, and what colour.

use glam::{Vec2, Vec4};

use crate::components::{
    ImageComponent, RectTransformComponent, ScriptComponent, ScriptFieldValue, SelectableComponent,
    TextAlignment, TextComponent,
};
use crate::scene::Scene;

/// Where the engine's widget scripts are seeded (engine-owned: rewritten on boot).
pub const SCRIPT_DIR: &str = "assets/scripts/ui";

/// Unity's default UI colours: white panels, dark grey text.
pub const WHITE: Vec4 = Vec4::ONE;
pub const DARK: Vec4 = Vec4::new(0.196, 0.196, 0.196, 1.0);

/// A rect: `anchor_min`, `anchor_max`, `pivot`, `anchored_position`, `size_delta`.
#[derive(Clone, Copy)]
pub struct Place(pub Vec2, pub Vec2, pub Vec2, pub Vec2, pub Vec2);

impl Place {
    /// A fixed-size element centred on its parent.
    pub fn centred(size: Vec2) -> Self {
        Self(
            Vec2::splat(0.5),
            Vec2::splat(0.5),
            Vec2::splat(0.5),
            Vec2::ZERO,
            size,
        )
    }

    /// Stretch over the anchor region `min..max`, grown by `grow` (negative insets).
    pub fn stretch(min: Vec2, max: Vec2, grow: Vec2) -> Self {
        Self(min, max, Vec2::splat(0.5), Vec2::ZERO, grow)
    }

    /// Fill the whole parent, inset by `inset` on every side.
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

/// A new node named `name` under `parent`, placed at `place`.
pub fn node(scene: &mut Scene, name: &str, parent: Option<u32>, place: Place) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.set_rect_transform(id, Some(place.rect()));
    if let Some(p) = parent {
        // A fresh node under a live parent cannot form a cycle.
        let _ = scene.set_parent(id, Some(p));
    }
    id
}

/// Give `id` a solid `Image` of `color`.
pub fn image(scene: &mut Scene, id: u32, color: Vec4) {
    let img = ImageComponent {
        color,
        ..Default::default()
    };
    scene.world.set_image(id, Some(img));
}

/// Give `id` a plain (no rich text, no wrap), non-raycast `Text`.
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

/// Make `id` a `Selectable` whose state shows on `target` (`None` = itself).
pub fn selectable(scene: &mut Scene, id: u32, target: Option<u32>) {
    let s = SelectableComponent {
        target_graphic: target.filter(|&t| t != id),
        ..Default::default()
    };
    scene.world.set_selectable(id, Some(s));
}

/// Attach the engine widget script `name` (`button` → `SCRIPT_DIR/button.lua`).
pub fn script(scene: &mut Scene, id: u32, name: &str) {
    script_with(scene, id, name, &[]);
}

/// [`script`], with some of its fields set away from the schema defaults.
pub fn script_with(scene: &mut Scene, id: u32, name: &str, values: &[(&str, ScriptFieldValue)]) {
    if let Some(mut scripts) = scene.world.scripts_mut(id) {
        scripts.push(ScriptComponent {
            path: format!("{SCRIPT_DIR}/{name}.lua"),
            values: values
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            ..Default::default()
        });
    }
}

/// A label filling its parent, inset by `inset`.
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
