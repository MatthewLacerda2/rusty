use crate::authoring::fields::table::Field;
use rusty_components::PostProcessingComponent;

#[derive(Debug)]
pub struct Knob {
    /// `center.x` for an array's lane: the tween path's last part.
    pub field: &'static str,
    /// White balance's two knobs are one `SetWhiteBalance(temperature, tint)`.
    pub api: Option<&'static str>,
    pub get: fn(&PostProcessingComponent) -> f32,
    pub set: fn(&mut PostProcessingComponent, f32),
}

pub struct Effect {
    pub name: &'static str,
    pub field: Field<PostProcessingComponent>,
    pub knobs: &'static [Knob],
}

impl Effect {
    pub fn knob(&self, field: &str) -> Option<&'static Knob> {
        self.knobs.iter().find(|k| k.field == field)
    }
}
