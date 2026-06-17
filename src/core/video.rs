//! src/core/video.rs — `VideoSettings`: resolution / vsync / fullscreen state.
//!
//! Runtime video settings (issue #89): the windowed app's framebuffer resolution,
//! whether vsync is on, and whether the window is fullscreen. This is the pure,
//! GPU-free **data + decision** half of the feature — what the platform/render
//! layer should reconfigure when a setting changes. The actual surface/window
//! reconfiguration lives in `render` / `main.rs` (the determinism-exempt layer);
//! the live preset switch rides the existing quality cell.
//!
//! ## Where it lives in the determinism story
//! Video settings are a platform/render-layer concern, never read by `FixedUpdate`.
//! They are **one-way** writes into render-only state, so changing resolution,
//! vsync or fullscreen cannot alter how the deterministic sim evolves. `core` sits
//! outside the determinism guard's sim dirs and stays decoupled from wgpu / winit /
//! egui — this module is plain data, no GPU types.
//!
//! ## Persistence
//! Settings round-trip through [`Storage`](crate::core::storage::Storage) under the
//! `video` namespace as a flat JSON object (`width`, `height`, `vsync`,
//! `fullscreen`). Loaded once at startup (a boundary read, a sim input like the
//! scene), flushed at Stop / quit — never read from disk inside a tick. A
//! missing/partial blob degrades to [`VideoSettings::default`].

use serde_json::{Map, Value};

/// The `Storage` namespace video settings persist under.
pub const VIDEO_NAMESPACE: &str = "video";

/// Default windowed framebuffer width (matches the boot window size in `main`).
pub const DEFAULT_WIDTH: u32 = 1280;
/// Default windowed framebuffer height (matches the boot window size in `main`).
pub const DEFAULT_HEIGHT: u32 = 720;

/// Runtime video settings: framebuffer resolution, vsync, and fullscreen. Pure
/// data — the render/platform layer maps these onto a wgpu surface + winit window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoSettings {
    /// Framebuffer width in physical pixels (clamped to at least 1 on apply).
    pub width: u32,
    /// Framebuffer height in physical pixels (clamped to at least 1 on apply).
    pub height: u32,
    /// Vsync on (`Fifo` present mode) vs off (`Immediate`, tear-for-latency).
    pub vsync: bool,
    /// Borderless-fullscreen the window vs windowed.
    pub fullscreen: bool,
}

impl Default for VideoSettings {
    /// The shipped windowed defaults: 1280x720, vsync on, windowed.
    fn default() -> Self {
        Self {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            vsync: true,
            fullscreen: false,
        }
    }
}

impl VideoSettings {
    /// Resolution as a `(width, height)` pair, each clamped to at least 1 so a
    /// degenerate stored value can never produce a zero-sized surface.
    pub fn resolution(&self) -> (u32, u32) {
        (self.width.max(1), self.height.max(1))
    }

    /// What this settings change requires of the render/platform layer, relative to
    /// `previous`. Lets the apply site skip work that hasn't changed (a vsync flip
    /// reconfigures the surface but needn't resize; a resolution change resizes).
    pub fn diff(&self, previous: &VideoSettings) -> VideoChange {
        VideoChange {
            resolution: self.resolution() != previous.resolution(),
            vsync: self.vsync != previous.vsync,
            fullscreen: self.fullscreen != previous.fullscreen,
        }
    }

    /// Serialize to a flat JSON object for `Storage::set_namespace`.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("width".into(), Value::from(self.width));
        map.insert("height".into(), Value::from(self.height));
        map.insert("vsync".into(), Value::Bool(self.vsync));
        map.insert("fullscreen".into(), Value::Bool(self.fullscreen));
        Value::Object(map)
    }

    /// Build settings from a stored JSON object. Missing or wrong-typed fields fall
    /// back to the default, so a malformed/partial blob degrades gracefully rather
    /// than failing — exactly how [`Keymap::from_json`] handles a partial keymap.
    ///
    /// [`Keymap::from_json`]: crate::core::keymap::Keymap::from_json
    pub fn from_json(value: &Value) -> Self {
        let d = Self::default();
        let Value::Object(map) = value else {
            return d;
        };
        let u32_at = |k: &str, fallback: u32| {
            map.get(k)
                .and_then(Value::as_u64)
                .map(|n| n as u32)
                .unwrap_or(fallback)
        };
        let bool_at =
            |k: &str, fallback: bool| map.get(k).and_then(Value::as_bool).unwrap_or(fallback);
        Self {
            width: u32_at("width", d.width),
            height: u32_at("height", d.height),
            vsync: bool_at("vsync", d.vsync),
            fullscreen: bool_at("fullscreen", d.fullscreen),
        }
    }
}

/// Which render/platform reconfigurations a `VideoSettings` change requires. Each
/// flag is independent so the apply site does the minimum work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct VideoChange {
    /// The framebuffer resolution changed — resize the surface + depth/post-FX.
    pub resolution: bool,
    /// Vsync toggled — reconfigure the surface with a new present mode.
    pub vsync: bool,
    /// Fullscreen toggled — set/clear the window's fullscreen mode.
    pub fullscreen: bool,
}

impl VideoChange {
    /// Whether anything at all needs reconfiguring.
    pub fn any(&self) -> bool {
        self.resolution || self.vsync || self.fullscreen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_windowed_720p_vsync_on() {
        let v = VideoSettings::default();
        assert_eq!(v.resolution(), (1280, 720));
        assert!(v.vsync);
        assert!(!v.fullscreen);
    }

    #[test]
    fn resolution_clamps_zero_to_one() {
        let v = VideoSettings {
            width: 0,
            height: 0,
            ..Default::default()
        };
        assert_eq!(v.resolution(), (1, 1));
    }

    #[test]
    fn json_round_trips() {
        let v = VideoSettings {
            width: 1920,
            height: 1080,
            vsync: false,
            fullscreen: true,
        };
        assert_eq!(VideoSettings::from_json(&v.to_json()), v);
    }

    #[test]
    fn partial_blob_degrades_to_defaults() {
        let blob = serde_json::json!({ "width": 800, "vsync": false });
        let v = VideoSettings::from_json(&blob);
        assert_eq!(v.width, 800);
        assert_eq!(v.height, DEFAULT_HEIGHT, "missing height falls back");
        assert!(!v.vsync);
        assert!(!v.fullscreen, "missing fullscreen falls back to default");
    }

    #[test]
    fn non_object_blob_is_all_defaults() {
        assert_eq!(
            VideoSettings::from_json(&Value::Null),
            VideoSettings::default()
        );
    }

    #[test]
    fn diff_flags_only_what_changed() {
        let base = VideoSettings::default();

        let vsync_off = VideoSettings {
            vsync: false,
            ..base
        };
        let c = vsync_off.diff(&base);
        assert!(c.vsync && !c.resolution && !c.fullscreen);
        assert!(c.any());

        let resized = VideoSettings {
            width: 1920,
            height: 1080,
            ..base
        };
        let c = resized.diff(&base);
        assert!(c.resolution && !c.vsync && !c.fullscreen);

        let fs = VideoSettings {
            fullscreen: true,
            ..base
        };
        let c = fs.diff(&base);
        assert!(c.fullscreen && !c.resolution && !c.vsync);

        // No change -> nothing to do.
        assert!(!base.diff(&base).any());
    }
}
