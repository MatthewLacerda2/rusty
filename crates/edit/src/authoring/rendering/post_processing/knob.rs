use crate::authoring::fields::table::Field;
use rusty_components::PostProcessingComponent;

#[derive(Debug)]
pub struct Knob {
    /// Its path inside the effect (`automatic.compensation`, `center.x` for a lane): the tween
    /// path's last part.
    pub field: &'static str,
    /// Registers `Graphics.Get<api>` / `Set<api>`; `None` for a knob sharing a call (white
    /// balance's two are one `SetWhiteBalance`).
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
