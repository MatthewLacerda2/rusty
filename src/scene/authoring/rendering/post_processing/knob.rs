//! What every post effect's authoring module declares (#1467): its field-table row
//! and its numeric knobs, gathered in [`super::EFFECTS`].
//!
//! A knob is one number of an effect, read and written through the effect's op, so
//! the Inspector's slider, a `Graphics.Get/Set<api>` pair and a
//! `Tween.To(volume, "Graphics.<effect>.<knob>", …)` reach the same value through
//! the same clamp.

use crate::components::PostProcessingComponent;
use crate::scene::authoring::fields::table::Field;

/// One numeric knob of an effect.
#[derive(Debug)]
pub struct Knob {
    /// Its path inside the effect's settings (`intensity`, `automatic.compensation`,
    /// `center.x` for an array's lane): the tween path's last part.
    pub field: &'static str,
    /// The `Graphics.Get<api>` / `Set<api>` pair it is, when it is one on its own
    /// (white balance's two knobs are one `SetWhiteBalance(temperature, tint)`).
    pub api: Option<&'static str>,
    pub get: fn(&PostProcessingComponent) -> f32,
    /// Through the effect's op, so it clamps as the op does.
    pub set: fn(&mut PostProcessingComponent, f32),
}

/// One post effect's authoring parts.
pub struct Effect {
    /// The effect, as `PostProcessingComponent` names its field.
    pub name: &'static str,
    /// Its row of the volume's field table: the whole settings object, through
    /// the ops.
    pub field: Field<PostProcessingComponent>,
    /// Its numeric knobs, in the Inspector's order.
    pub knobs: &'static [Knob],
}

impl Effect {
    /// The knob at `field` inside this effect.
    pub fn knob(&self, field: &str) -> Option<&'static Knob> {
        self.knobs.iter().find(|k| k.field == field)
    }
}
