mod file_feed;
pub use file_feed::{FileFeed, FileLine, FILE_FEED_CAP};

/// The console log buffer and severity levels shared by gameplay scripts, the
/// REPL and the editor's bottom panel. The `print` hook and every `Debug.*`
/// binding funnel through here.
pub struct ConsoleLogs {
    pub messages: Vec<(String, LogLevel)>,
    /// The record the newest entry collapses, and how many times it has arrived in a
    /// row ([`ConsoleLogs::repeated`]).
    repeat: Option<(String, u32)>,
    /// How many entries were ever added, the dropped ones included: a mark for
    /// [`ConsoleLogs::since`].
    logged: u64,
    /// What a recording demo has not taken yet (#1597); `None` while no demo
    /// records, which makes a mark a no-op.
    feed: Option<DemoFeed>,
    /// Every warning and error since [`keep_warnings`](Self::keep_warnings), none
    /// dropped by the buffer's cap: what `Harness.Warnings` reads (#2054). `None`
    /// keeps none, so the editor's long-lived console never grows.
    warnings: Option<Vec<(String, LogLevel)>>,
    /// What the frame log on disk has not taken yet; `None` while no log runs.
    file: Option<FileFeed>,
}

/// What the console hands a recording demo's index each tick (#1597).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemoFeed {
    pub marks: Vec<Mark>,
    /// Every line as it came, a repeat the buffer folds into `msg (×N)` included.
    pub lines: Vec<(String, LogLevel)>,
}

/// A game fact a script marked for a demo's index (`Debug.Mark(label [, value])`).
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub label: String,
    pub value: Option<super::Plain>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl Default for ConsoleLogs {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleLogs {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            repeat: None,
            logged: 0,
            feed: None,
            warnings: None,
            file: None,
        }
    }

    /// Keep every warning and error from now on, past the buffer's cap: a headless
    /// run's scenario reads them with `Harness.Warnings` (#2054).
    pub fn keep_warnings(&mut self) {
        self.warnings.get_or_insert_with(Vec::new);
    }

    /// The warnings and errors kept since [`keep_warnings`](Self::keep_warnings),
    /// oldest first, a repeat folded as the buffer shows it (`msg (×3)`).
    pub fn warnings(&self) -> &[(String, LogLevel)] {
        self.warnings.as_deref().unwrap_or_default()
    }

    /// Feed a demo from now on (`true`, a demo started recording), or stop
    /// (`false`): marks are dropped as they come.
    pub fn feed_demo(&mut self, feed: bool) {
        self.feed = feed.then(DemoFeed::default);
    }

    /// Note a mark; a no-op unless a demo records.
    pub fn mark(&mut self, mark: Mark) {
        if let Some(feed) = &mut self.feed {
            feed.marks.push(mark);
        }
    }

    /// The marks and lines since the last take, oldest first.
    pub fn take_feed(&mut self) -> DemoFeed {
        self.feed.as_mut().map(std::mem::take).unwrap_or_default()
    }

    /// Keep every line for the frame log on disk from now on (`true`; idempotent),
    /// or stop (`false`).
    pub fn feed_file(&mut self, on: bool) {
        if !on {
            self.file = None;
        } else if self.file.is_none() {
            self.file = Some(FileFeed::default());
        }
    }

    /// The lines since the last take, oldest first (none while no log is fed).
    pub fn take_file_feed(&mut self) -> FileFeed {
        self.file.as_mut().map(std::mem::take).unwrap_or_default()
    }

    fn feed_line(&mut self, msg: &str, level: &LogLevel) {
        if let Some(feed) = &mut self.feed {
            feed.lines.push((msg.to_string(), level.clone()));
        }
    }

    pub fn info(&mut self, msg: String) {
        self.add_log(msg, LogLevel::Info);
    }

    pub fn warn(&mut self, msg: String) {
        self.add_log(msg, LogLevel::Warning);
    }

    pub fn error(&mut self, msg: String) {
        self.add_log(msg, LogLevel::Error);
    }

    /// Log a record that arrived `times` times in a row. When it repeats the newest
    /// entry, that entry counts up instead (`msg (×3)`), as Unity's Console collapses
    /// a repeated warning, so one warning raised every frame cannot push everything
    /// else out of the buffer.
    pub fn repeated(&mut self, msg: String, level: LogLevel, times: u32) {
        if let Some(file) = &mut self.file {
            file.push(&msg, &level, times, true);
        }
        self.feed_line(&Self::collapsed(&msg, times), &level);
        if let Some((text, count)) = &self.repeat {
            let newest = self.messages.last();
            if *text == msg && newest == Some(&(Self::collapsed(text, *count), level.clone())) {
                let count = count + times;
                if let Some(entry) = self.messages.last_mut() {
                    entry.0 = Self::collapsed(&msg, count);
                }
                // The newest entry is a repeat's, so it is the newest warning too.
                if let (LogLevel::Warning | LogLevel::Error, Some(kept)) =
                    (&level, self.warnings.as_mut().and_then(|w| w.last_mut()))
                {
                    kept.0 = Self::collapsed(&msg, count);
                }
                self.repeat = Some((msg, count));
                return;
            }
        }
        self.add_entry(Self::collapsed(&msg, times), level);
        self.repeat = Some((msg, times));
    }

    /// How the console shows a record that arrived `count` times in a row.
    pub fn collapsed(msg: &str, count: u32) -> String {
        match count {
            0 | 1 => msg.to_string(),
            n => format!("{msg} (×{n})"),
        }
    }

    /// A mark to read the entries added after it with [`since`](Self::since).
    pub fn logged(&self) -> u64 {
        self.logged
    }

    /// The entries added since `mark` ([`logged`](Self::logged)) still in the buffer,
    /// oldest first.
    pub fn since(&self, mark: u64) -> impl Iterator<Item = &(String, LogLevel)> {
        let added = usize::try_from(self.logged - mark).unwrap_or(usize::MAX);
        let kept = added.min(self.messages.len());
        self.messages[self.messages.len() - kept..].iter()
    }

    fn add_log(&mut self, msg: String, level: LogLevel) {
        if let Some(file) = &mut self.file {
            file.push(&msg, &level, 1, false);
        }
        self.feed_line(&msg, &level);
        self.add_entry(msg, level);
    }

    fn add_entry(&mut self, msg: String, level: LogLevel) {
        self.logged += 1;
        self.repeat = None;
        if self.messages.len() >= 100 {
            self.messages.remove(0);
        }
        if let (LogLevel::Warning | LogLevel::Error, Some(kept)) = (&level, &mut self.warnings) {
            kept.push((msg.clone(), level.clone()));
        }
        self.messages.push((msg, level));
    }
}

#[cfg(test)]
mod tests;
