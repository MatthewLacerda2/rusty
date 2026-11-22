//! The console's side of the frame log (`dev::frame_log`): every line past the buffer's
//! cap, a repeat counted in `times` instead of folded into `msg (×N)`.

use super::LogLevel;

/// Lines a take may hold: past it, a line is counted in [`FileFeed::lost`]. A take
/// is one window frame, so this bounds what one frame can put on disk.
pub const FILE_FEED_CAP: usize = 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct FileLine {
    pub msg: String,
    pub level: LogLevel,
    pub times: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileFeed {
    pub lines: Vec<FileLine>,
    pub lost: u64,
    repeat: bool,
}

impl FileFeed {
    /// `fold`: a `repeated` record, which counts up the newest line only when that one
    /// was a `repeated` record too, as the buffer folds.
    pub(super) fn push(&mut self, msg: &str, level: &LogLevel, times: u32, fold: bool) {
        if let Some(last) = self.lines.last_mut().filter(|_| fold && self.repeat) {
            if last.msg == msg && last.level == *level {
                last.times = last.times.saturating_add(times);
                return;
            }
        }
        self.repeat = fold;
        if self.lines.len() >= FILE_FEED_CAP {
            self.lost += 1;
            return;
        }
        let (msg, level) = (msg.to_string(), level.clone());
        self.lines.push(FileLine { msg, level, times });
    }
}
