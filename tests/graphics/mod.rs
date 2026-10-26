//! The `Graphics` scripting namespace: its per-camera post-FX knobs (`api`), which
//! camera they edit (`main_camera`), the colour grade (`grading`), the lens effects
//! (`lens`), the lens flare (`lens_flare`), the depth of field (`depth_of_field`), the exposure (`exposure`) and the
//! player graphics options (`options`).

mod api;
mod depth_of_field;
mod exposure;
mod grading;
mod lens;
mod lens_flare;
mod main_camera;
mod options;
