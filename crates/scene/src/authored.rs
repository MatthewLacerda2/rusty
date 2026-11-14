//! crates/scene/src/authored.rs — what the authoring layer lists for the post
//! effects, as a value a crate below that layer can read: the post effects'
//! completeness gate (`render::postfx::completeness`) takes one, built by
//! `authoring::rendering::post_processing::authored`. Plain names, so the gate's
//! live code names nothing of the authoring layer.

/// The authoring layer's post-effect lists, by name.
#[derive(Clone, Default)]
pub struct Authored {
    /// `PostProcessingComponent`'s serialized fields (the field table's).
    pub volume_fields: Vec<String>,
    /// `CameraComponent`'s serialized fields.
    pub camera_fields: Vec<String>,
    /// The volume's effect rows (`post_processing::EFFECTS`), each with its knob paths.
    pub ops: Vec<(String, Vec<String>)>,
    /// The camera effects' knob tables (`camera::TAA_KNOBS`), each with its knob paths.
    pub camera_knobs: Vec<(String, Vec<String>)>,
}
