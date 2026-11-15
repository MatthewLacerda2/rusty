//! The Game view's own size (#2207): Unity's Game view resolution dropdown (*Free
//! Aspect*, an aspect, a fixed resolution) and its *Low Resolution Aspect Ratios*
//! toggle, which here is "1x on HiDPI".
//!
//! With [`GameViewSize::Free`] the Game view is the panel's physical pixels, as it
//! always was. An aspect is the largest size of that ratio inside the panel; a fixed
//! size is exactly that size whatever the panel. Anything but `Free` is drawn
//! letterboxed and scaled into the panel, as a demo replay is, and the sim, the UI,
//! the pointer and the renderer all read the resolved size
//! ([`GameViewSetting::resolve`]), never the panel's. "1x on HiDPI" sizes `Free` and an
//! aspect in the panel's points rather than its pixels, so a Retina panel shows the
//! game at the 1x density; a fixed size is fixed either way.
//!
//! Here, below the editor and the shell, because `Editor.SetGameViewSize` (the `dev`
//! layer) and the editor's dropdown set it and the shell reads it. It is the editor's
//! state, never the project's: the set lasts the editor session, as Unity's per-user
//! Game view does. The mailbox half is `gizmos`' own: a script's set lands in the
//! shared [`Application`] at once (read back on the next line) and the editor window
//! takes it at the frame boundary, mirroring its own choice back each frame.

use super::Application;

/// The largest width or height a Game view may be given: wgpu's default texture
/// limit, which every target the Game view renders into must fit.
pub const MAX_GAME_VIEW_SIDE: u32 = 8192;

/// What the Game view's size follows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GameViewSize {
    /// The panel's own size (Unity's *Free Aspect*): the default.
    #[default]
    Free,
    /// The largest `width:height` rectangle inside the panel.
    Aspect(u32, u32),
    /// Exactly `width × height` pixels, letterboxed and scaled into the panel.
    Fixed(u32, u32),
}

impl GameViewSize {
    /// The dropdown's presets, in its order, with their labels: Unity's common set.
    pub const PRESETS: &[(GameViewSize, &'static str)] = &[
        (GameViewSize::Free, "Free Aspect"),
        (GameViewSize::Aspect(16, 9), "16:9"),
        (GameViewSize::Aspect(16, 10), "16:10"),
        (GameViewSize::Aspect(4, 3), "4:3"),
        (GameViewSize::Fixed(1920, 1080), "1920x1080"),
        (GameViewSize::Fixed(1280, 720), "1280x720"),
    ];

    /// A size `Editor.SetGameViewSize` / `SetGameViewAspect` may set: both sides
    /// from 1 to [`MAX_GAME_VIEW_SIDE`]. `Free` is always valid.
    pub fn is_valid(self) -> bool {
        let side = |v: u32| (1..=MAX_GAME_VIEW_SIDE).contains(&v);
        match self {
            GameViewSize::Free => true,
            GameViewSize::Aspect(w, h) | GameViewSize::Fixed(w, h) => side(w) && side(h),
        }
    }

    /// `"Free"`, `"W:H"` (an aspect) or `"WxH"` (a fixed size), as
    /// [`label`](Self::label) writes a custom one; `None` for anything else or a
    /// size [`is_valid`](Self::is_valid) refuses.
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

    /// The label the dropdown shows: a preset's, else `"W:H"` or `"WxH"`.
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

/// The Game view's size setting: what it follows and whether "1x on HiDPI" is on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameViewSetting {
    pub size: GameViewSize,
    /// Unity's *Low Resolution Aspect Ratios*: size `Free` and an aspect by the
    /// panel's points, not its physical pixels.
    pub low_resolution: bool,
}

impl GameViewSetting {
    /// The Game view's pixel size inside a panel of `panel` physical pixels at
    /// `pixels_per_point`; `None` when it is the panel's own (`Free`, "1x on HiDPI"
    /// off, or a scale of 1 or less), which draws as it always did. Each side is at
    /// least 1.
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

    /// The display scale the game sees (`Video.GetDisplayScale`) on a `window`
    /// scale: 1 while "1x on HiDPI" sizes the view by points (`Free` or an aspect),
    /// else the window's.
    pub fn display_scale(self, window: f32) -> f32 {
        let by_points = !matches!(self.size, GameViewSize::Fixed(..));
        match self.low_resolution && by_points && window > 1.0 {
            true => 1.0,
            false => window,
        }
    }
}

/// The mailbox: a script's settings in, the editor's choice mirrored out.
#[derive(Default)]
pub struct GameViewDesk {
    viewer: bool,
    setting: GameViewSetting,
    requests: Vec<GameViewSetting>,
}

impl Application {
    /// Mark the host as one with a Game view whose size a script may set (the
    /// editor window, once when built).
    pub fn mark_game_view_host(&mut self) {
        self.game_view.viewer = true;
    }

    /// Whether the host has a Game view whose size a script may set.
    pub fn game_view_host(&self) -> bool {
        self.game_view.viewer
    }

    /// The Game view's setting, as the editor last mirrored it and scripts set it.
    pub fn game_view_setting(&self) -> GameViewSetting {
        self.game_view.setting
    }

    /// A script sets the Game view: read back at once, taken by the editor at the
    /// next frame.
    pub fn request_game_view(&mut self, setting: GameViewSetting) {
        self.game_view.setting = setting;
        self.game_view.requests.push(setting);
    }

    /// The editor's frame: the newest script request (each is a whole setting)
    /// replaces its `setting`, and the result is mirrored here.
    pub fn sync_game_view(&mut self, setting: &mut GameViewSetting) {
        if let Some(last) = std::mem::take(&mut self.game_view.requests).pop() {
            *setting = last;
        }
        self.game_view.setting = *setting;
    }
}

#[cfg(test)]
#[path = "game_view_tests.rs"]
mod tests;
