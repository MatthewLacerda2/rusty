// The procedural sky's art-directed shape, tuned by eye; `scene`'s `SkySettings::resolved` and
// `skybox::procedural` mirror each value.

#define_import_path sky::look

const SKY_HORIZON_TO_WHITE: f32 = 0.6;
const SKY_GROUND_OF_ZENITH: f32 = 0.25;
const SKY_ZENITH_EXPONENT: f32 = 0.5;
const SKY_GROUND_EXPONENT: f32 = 0.35;
const CLOUD_DOME_MIN_HEIGHT: f32 = 0.15;
const CLOUD_CELLS_PER_UNIT: f32 = 1.6;
const CLOUD_FBM_EDGE_START: f32 = 0.45;
const CLOUD_FBM_EDGE_END: f32 = 0.80;
const CLOUD_BAND_START: f32 = 0.05;
const CLOUD_BAND_END: f32 = 0.35;
const CLOUD_TO_WHITE: f32 = 0.85;
