//! The component fields a tween may drive (#424), resolved by name.
//!
//! A property path is `"Component.field"` — `"CanvasGroup.alpha"`,
//! `"RectTransform.anchored_position"`. Only the numeric fields listed here are
//! animatable, so a typo is an error at `Tween.To`, never a silent no-op. Writes
//! route through the same `scene::authoring` ops the inspector and the component
//! namespaces use, so a tweened alpha is clamped exactly like a `SetAlpha`.
//!
//! Every value travels as a `Vec4`; a property uses its first `arity` lanes.

use glam::{Vec2, Vec3, Vec4};

use crate::scene::authoring::{
    audio, camera, canvas_group, image, light, rect_transform as rect, text,
};
use crate::scene::Scene;

/// One animatable component field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Property {
    Position,
    Rotation,
    Scale,
    AnchoredPosition,
    SizeDelta,
    Pivot,
    AnchorMin,
    AnchorMax,
    Alpha,
    ImageColor,
    FillAmount,
    TextColor,
    FontSize,
    LightColor,
    Intensity,
    Range,
    Fov,
    Volume,
}

/// `(path, property, arity)` — the whole animatable surface.
const PROPERTIES: [(&str, Property, usize); 18] = [
    ("Transform.position", Property::Position, 3),
    ("Transform.rotation", Property::Rotation, 3),
    ("Transform.scale", Property::Scale, 3),
    (
        "RectTransform.anchored_position",
        Property::AnchoredPosition,
        2,
    ),
    ("RectTransform.size_delta", Property::SizeDelta, 2),
    ("RectTransform.pivot", Property::Pivot, 2),
    ("RectTransform.anchor_min", Property::AnchorMin, 2),
    ("RectTransform.anchor_max", Property::AnchorMax, 2),
    ("CanvasGroup.alpha", Property::Alpha, 1),
    ("Image.color", Property::ImageColor, 4),
    ("Image.fill_amount", Property::FillAmount, 1),
    ("Text.color", Property::TextColor, 4),
    ("Text.font_size", Property::FontSize, 1),
    ("Light.color", Property::LightColor, 3),
    ("Light.intensity", Property::Intensity, 1),
    ("Light.range", Property::Range, 1),
    ("Camera.fov", Property::Fov, 1),
    ("AudioSource.volume", Property::Volume, 1),
];

impl Property {
    /// Resolve a `"Component.field"` path.
    pub(crate) fn parse(path: &str) -> Option<Self> {
        PROPERTIES.iter().find(|(p, ..)| *p == path).map(|e| e.1)
    }

    /// Every accepted path, for the error a typo gets.
    pub(crate) fn paths() -> String {
        PROPERTIES.map(|(p, ..)| p).join(", ")
    }

    /// The path this property was parsed from.
    pub(crate) fn path(self) -> &'static str {
        self.entry().0
    }

    /// How many numbers the value has (1 = a number, 2..4 = a vector / colour).
    pub(crate) fn arity(self) -> usize {
        self.entry().2
    }

    fn entry(self) -> &'static (&'static str, Property, usize) {
        PROPERTIES
            .iter()
            .find(|(_, p, _)| *p == self)
            .expect("every Property is listed in PROPERTIES")
    }

    /// The field's current value, or `None` when `id` lacks the component.
    pub(crate) fn get(self, scene: &Scene, id: u32) -> Option<Vec4> {
        let w = &scene.world;
        let v3 = |v: Vec3| v.extend(0.0);
        let v2 = |v: Vec2| v.extend(0.0).extend(0.0);
        let f = |v: f32| Vec4::new(v, 0.0, 0.0, 0.0);
        Some(match self {
            Self::Position => v3(w.transform(id)?.position),
            Self::Rotation => v3(w.transform(id)?.euler_angles()),
            Self::Scale => v3(w.transform(id)?.scale),
            Self::AnchoredPosition => v2(w.rect_transform(id)?.anchored_position),
            Self::SizeDelta => v2(w.rect_transform(id)?.size_delta),
            Self::Pivot => v2(w.rect_transform(id)?.pivot),
            Self::AnchorMin => v2(w.rect_transform(id)?.anchor_min),
            Self::AnchorMax => v2(w.rect_transform(id)?.anchor_max),
            Self::Alpha => f(w.canvas_group(id)?.alpha),
            Self::ImageColor => w.image(id)?.color,
            Self::FillAmount => f(w.image(id)?.fill_amount),
            Self::TextColor => w.text(id)?.color,
            Self::FontSize => f(w.text(id)?.font_size),
            Self::LightColor => v3(w.light(id)?.color),
            Self::Intensity => f(w.light(id)?.intensity),
            Self::Range => f(w.light(id)?.range),
            Self::Fov => f(w.camera(id)?.fov),
            Self::Volume => f(w.audio(id)?.volume),
        })
    }

    /// Write `v` through the field's authoring op. `false` when `id` lacks the
    /// component (it was removed mid-tween).
    pub(crate) fn set(self, scene: &mut Scene, id: u32, v: Vec4) -> bool {
        let w = &mut scene.world;
        let xy = Vec2::new(v.x, v.y);
        macro_rules! write {
            ($get:ident, |$c:ident| $body:expr) => {
                w.$get(id)
                    .map(|mut guard| {
                        let $c = &mut *guard;
                        $body;
                    })
                    .is_some()
            };
        }
        let written = match self {
            Self::Position => write!(transform_mut, |c| c.position = v.truncate()),
            Self::Rotation => write!(transform_mut, |c| c.set_euler_angles(v.truncate())),
            Self::Scale => write!(transform_mut, |c| c.scale = v.truncate()),
            Self::AnchoredPosition => {
                write!(rect_transform_mut, |c| rect::set_anchored_position(c, xy))
            }
            Self::SizeDelta => write!(rect_transform_mut, |c| rect::set_size_delta(c, xy)),
            Self::Pivot => write!(rect_transform_mut, |c| rect::set_pivot(c, xy)),
            Self::AnchorMin => write!(rect_transform_mut, |c| rect::set_anchor_min(c, xy)),
            Self::AnchorMax => write!(rect_transform_mut, |c| rect::set_anchor_max(c, xy)),
            Self::Alpha => write!(canvas_group_mut, |c| canvas_group::set_alpha(c, v.x)),
            Self::ImageColor => write!(image_mut, |c| image::set_color(c, v)),
            Self::FillAmount => write!(image_mut, |c| image::set_fill_amount(c, v.x)),
            Self::TextColor => write!(text_mut, |c| text::set_color(c, v)),
            Self::FontSize => write!(text_mut, |c| text::set_font_size(c, v.x)),
            Self::LightColor => write!(light_mut, |c| light::set_color(c, v.truncate())),
            Self::Intensity => write!(light_mut, |c| light::set_intensity(c, v.x)),
            Self::Range => write!(light_mut, |c| light::set_range(c, v.x)),
            Self::Fov => write!(camera_mut, |c| camera::set_fov(c, v.x)),
            Self::Volume => write!(audio_mut, |c| audio::set_volume(c, v.x)),
        };
        // A moved transform drags its collider along, as `Transform.Set*` does.
        if written && matches!(self, Self::Position | Self::Rotation | Self::Scale) {
            scene.update_entity_collider(id);
        }
        written
    }
}
