//! What the console hands the rolling frame log on disk (`dev::frame_log`): every
//! line as it came, past the buffer's cap of 100, so a load that logs 150 lines or
//! a warning the buffer folds into `msg (×N)` still reaches the file.

use super::LogLevel;

/// Lines a take may hold: past it, a line is counted in [`FileFeed::lost`]. A take
/// is one window frame, so this bounds what one frame can put on disk.
pub const FILE_FEED_CAP: usize = 1024;

/// One line for the file: the record, its level, and how many times it arrived in
/// a row since the last take (a repeat folded, so a warning raised every tick is
/// one line per frame, not one per tick).
#[derive(Clone, Debug, PartialEq)]
pub struct FileLine {
    pub msg: String,
    pub level: LogLevel,
    pub times: u32,
}

/// The lines since the last take, oldest first, and how many the cap lost.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FileFeed {
    pub lines: Vec<FileLine>,
    pub lost: u64,
    /// Whether the newest line is a repeat's, which a repeat of it counts up.
    repeat: bool,
}

impl FileFeed {
    /// Add `times` arrivals of `msg`; a repeat (`fold`) of the newest line, itself
    /// a repeat's, counts it up instead of adding one.
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
