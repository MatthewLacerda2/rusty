//! Unity's Game view resolution dropdown. Anything but `Free` is drawn letterboxed in
//! the panel; the sim, UI, pointer and renderer read the resolved size, never the
//! panel's. Editor state, never the project's.

use super::Application;

pub const MAX_GAME_VIEW_SIDE: u32 = 8192;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameViewSize {
    #[default]
    Free,
    Aspect(u32, u32),
    Fixed(u32, u32),
}

impl GameViewSize {
    // lint: allow(named-enum) the variants carry sizes: a preset list, not a name table
    pub const PRESETS: &[(GameViewSize, &'static str)] = &[
        (GameViewSize::Free, "Free Aspect"),
        (GameViewSize::Aspect(16, 9), "16:9"),
        (GameViewSize::Aspect(16, 10), "16:10"),
        (GameViewSize::Aspect(4, 3), "4:3"),
        (GameViewSize::Fixed(1920, 1080), "1920x1080"),
        (GameViewSize::Fixed(1280, 720), "1280x720"),
    ];

    pub fn is_valid(self) -> bool {
        let side = |v: u32| (1..=MAX_GAME_VIEW_SIDE).contains(&v);
        match self {
            GameViewSize::Free => true,
            GameViewSize::Aspect(w, h) | GameViewSize::Fixed(w, h) => side(w) && side(h),
        }
    }

    pub fn parse(text: &str) -> Option<GameViewSize> {
        let text = text.trim();
        if text == "Free" {
            return Some(GameViewSize::Free);
        }
        let sides = |(w, h): (&str, &str)| Some((w.trim().parse().ok()?, h.trim().parse().ok()?));
        let size = match (text.split_once(':'), text.split_once('x')) {
            (Some(pair), None) => sides(pair).map(|(w, h)| GameViewSize::Aspect(w, h)),
            (None, Some(pair)) => sides(pair).map(|(w, h)| GameViewSize::Fixed(w, h)),
            _ => None,
        };
        size.filter(|size| size.is_valid())
    }

    pub fn label(self) -> String {
        if let Some((_, label)) = Self::PRESETS.iter().find(|(size, _)| *size == self) {
            return (*label).to_string();
        }
        match self {
            GameViewSize::Free => "Free Aspect".to_string(),
            GameViewSize::Aspect(w, h) => format!("{w}:{h}"),
            GameViewSize::Fixed(w, h) => format!("{w}x{h}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameViewSetting {
    pub size: GameViewSize,
    pub low_resolution: bool,
}

impl GameViewSetting {
    /// `None` when the Game view is the panel's own size.
    pub fn resolve(self, panel: (u32, u32), pixels_per_point: f32) -> Option<(u32, u32)> {
        let shrink = self.low_resolution && pixels_per_point > 1.0;
        let scale = if shrink { pixels_per_point } else { 1.0 };
        let base = (panel.0 as f32 / scale, panel.1 as f32 / scale);
        let at_least_1 = |v: f32| (v.round() as u32).max(1);
        let size = match self.size {
            GameViewSize::Free if !shrink => return None,
            GameViewSize::Free => (at_least_1(base.0), at_least_1(base.1)),
            GameViewSize::Aspect(w, h) => {
                let aspect = w.max(1) as f32 / h.max(1) as f32;
                let (bw, bh) = (base.0.max(1.0), base.1.max(1.0));
                if bw / bh > aspect {
                    (at_least_1(bh * aspect), at_least_1(bh))
                } else {
                    (at_least_1(bw), at_least_1(bw / aspect))
                }
            }
            GameViewSize::Fixed(w, h) => (w.max(1), h.max(1)),
        };
        Some(size)
    }

    pub fn display_scale(self, window: f32) -> f32 {
        let fixed = matches!(self.size, GameViewSize::Fixed(..));
        match fixed || (self.low_resolution && window > 1.0) {
            true => 1.0,
            false => window,
        }
    }
}

pub fn letterbox(panel: (f32, f32), (width, height): (u32, u32)) -> ((f32, f32), (f32, f32)) {
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    let extent = if panel.0 / panel.1.max(f32::MIN_POSITIVE) > aspect {
        (panel.1 * aspect, panel.1)
    } else {
        (panel.0, panel.0 / aspect)
    };
    let offset = ((panel.0 - extent.0) / 2.0, (panel.1 - extent.1) / 2.0);
    (offset, extent)
}

#[derive(Default)]
pub struct GameViewDesk {
    viewer: bool,
    setting: GameViewSetting,
    request: Option<GameViewSetting>,
}

impl Application {
    pub fn mark_game_view_host(&mut self) {
        self.game_view.viewer = true;
    }

    pub fn game_view_host(&self) -> bool {
        self.game_view.viewer
    }

    pub fn game_view_setting(&self) -> GameViewSetting {
        self.game_view.setting
    }

    pub fn request_game_view(&mut self, setting: GameViewSetting) {
        self.game_view.setting = setting;
        self.game_view.request = Some(setting);
    }

    pub fn sync_game_view(&mut self, setting: &mut GameViewSetting) {
        if let Some(last) = self.game_view.request.take() {
            *setting = last;
        }
        self.game_view.setting = *setting;
    }
}

#[cfg(test)]
#[path = "game_view_tests.rs"]
mod tests;
