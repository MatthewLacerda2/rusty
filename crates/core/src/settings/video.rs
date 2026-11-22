use serde_json::{Map, Value};

pub const VIDEO_NAMESPACE: &str = "video";

pub const DEFAULT_WIDTH: u32 = 1280;
pub const DEFAULT_HEIGHT: u32 = 720;

pub const MAX_SIDE: u32 = 8192;

pub fn parse_size(text: &str) -> Result<(u32, u32), String> {
    let bad = || format!("bad size {text:?}: expected <W>x<H>, each 1..={MAX_SIDE}");
    let (w, h) = text.split_once('x').ok_or_else(bad)?;
    let side = |s: &str| s.parse().ok().filter(|n| (1..=MAX_SIDE).contains(n));
    Ok((side(w).ok_or_else(bad)?, side(h).ok_or_else(bad)?))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoSettings {
    /// Framebuffer width in physical pixels (clamped to at least 1 on apply).
    pub width: u32,
    /// Framebuffer height in physical pixels (clamped to at least 1 on apply).
    pub height: u32,
    pub vsync: bool,
    pub fullscreen: bool,
}

impl Default for VideoSettings {
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

    pub fn diff(&self, previous: &VideoSettings) -> VideoChange {
        VideoChange {
            resolution: self.resolution() != previous.resolution(),
            vsync: self.vsync != previous.vsync,
            fullscreen: self.fullscreen != previous.fullscreen,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("width".into(), Value::from(self.width));
        map.insert("height".into(), Value::from(self.height));
        map.insert("vsync".into(), Value::Bool(self.vsync));
        map.insert("fullscreen".into(), Value::Bool(self.fullscreen));
        Value::Object(map)
    }

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct VideoChange {
    pub resolution: bool,
    pub vsync: bool,
    pub fullscreen: bool,
}

impl VideoChange {
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
    fn parse_size_reads_w_x_h_and_refuses_the_rest() {
        assert_eq!(parse_size("2940x1388"), Ok((2940, 1388)));
        assert_eq!(parse_size("1x8192"), Ok((1, MAX_SIDE)));
        for bad in [
            "", "1280", "0x720", "1280x0", "8193x1", "-1x2", "12x34x5", "ax720",
        ] {
            assert!(parse_size(bad).is_err(), "{bad:?} should be refused");
        }
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

        assert!(!base.diff(&base).any());
    }
}
