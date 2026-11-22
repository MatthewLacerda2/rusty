#[derive(Clone, Default)]
pub struct Authored {
    pub volume_fields: Vec<String>,
    pub camera_fields: Vec<String>,
    pub ops: Vec<(String, Vec<String>)>,
    pub camera_knobs: Vec<(String, Vec<String>)>,
}
