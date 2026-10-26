//! Depth of field on a post-processing volume (#1445): Unity URP's override of that
//! name, in its two modes. *Gaussian* blurs the background from a start to an end
//! distance (cheap, menus and cutscenes); *Bokeh* models a lens, so what lies nearer
//! or further than the focus distance blurs with the aperture and focal length
//! (aiming down sights).

use serde::{Deserialize, Serialize};

crate::core::named_enum! {
    /// Which depth of field runs. Declared in the order the editor lists them (the
    /// default first); the names are Unity's.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    pub enum DofMode {
        /// No depth of field: no pass runs.
        #[default]
        Off = "Off",
        /// The background blurs, from `gaussian_start` to `gaussian_end`.
        Gaussian = "Gaussian",
        /// A lens: both sides of `focus_distance` blur, with a round bokeh.
        Bokeh = "Bokeh",
    }
}

/// The depth-of-field knobs: the mode and both modes' parameters, each kept while
/// the other mode runs.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DepthOfField {
    pub mode: DofMode,
    /// Gaussian: the distance from the camera, in metres, where the blur starts.
    pub gaussian_start: f32,
    /// Gaussian: where the blur reaches its full radius; nearer than
    /// `gaussian_start`, it is drawn at the start (an instant edge).
    pub gaussian_end: f32,
    /// Gaussian: how wide the full blur is, `0.5..=1.5` (Unity's range).
    pub gaussian_max_radius: f32,
    /// Bokeh: the distance in focus, in metres.
    pub focus_distance: f32,
    /// Bokeh: the f-stop, `1..=32`: lower blurs more.
    pub aperture: f32,
    /// Bokeh: the lens's focal length in millimetres, `1..=300`: longer blurs more.
    pub focal_length: f32,
}

impl DepthOfField {
    /// The Gaussian radius range Unity's slider spans.
    pub const MIN_MAX_RADIUS: f32 = 0.5;
    pub const MAX_MAX_RADIUS: f32 = 1.5;
    /// The f-stop range.
    pub const MIN_APERTURE: f32 = 1.0;
    pub const MAX_APERTURE: f32 = 32.0;
    /// The focal-length range, in millimetres.
    pub const MIN_FOCAL_LENGTH: f32 = 1.0;
    pub const MAX_FOCAL_LENGTH: f32 = 300.0;
    /// The nearest focus distance a write accepts, in metres.
    pub const MIN_FOCUS_DISTANCE: f32 = 0.1;
    /// The farthest focus distance, in metres: where `+inf` (focus at infinity)
    /// lands, past any level.
    pub const MAX_FOCUS_DISTANCE: f32 = 10_000.0;
}

impl Default for DepthOfField {
    /// Off, with Unity URP's defaults ready for either mode: Gaussian from 10 m to
    /// 30 m at radius 1, Bokeh focused at 10 m through a 50 mm lens at f/5.6.
    fn default() -> Self {
        Self {
            mode: DofMode::Off,
            gaussian_start: 10.0,
            gaussian_end: 30.0,
            gaussian_max_radius: 1.0,
            focus_distance: 10.0,
            aperture: 5.6,
            focal_length: 50.0,
        }
    }
}
