mod file_feed;
pub use file_feed::{FileFeed, FileLine, FILE_FEED_CAP};

pub struct ConsoleLogs {
    pub messages: Vec<(String, LogLevel)>,
    repeat: Option<(String, u32)>,
    logged: u64,
    feed: Option<DemoFeed>,
    /// Every warning and error since [`keep_warnings`](Self::keep_warnings), none
    /// dropped by the buffer's cap: what `Harness.Warnings` reads. `None`
    /// keeps none, so the editor's long-lived console never grows.
    warnings: Option<Vec<(String, LogLevel)>>,
    file: Option<FileFeed>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemoFeed {
    pub marks: Vec<Mark>,
    pub lines: Vec<(String, LogLevel)>,
}

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

    pub fn keep_warnings(&mut self) {
        self.warnings.get_or_insert_with(Vec::new);
    }

    pub fn warnings(&self) -> &[(String, LogLevel)] {
        self.warnings.as_deref().unwrap_or_default()
    }

    pub fn feed_demo(&mut self, feed: bool) {
        self.feed = feed.then(DemoFeed::default);
    }

    pub fn mark(&mut self, mark: Mark) {
        if let Some(feed) = &mut self.feed {
            feed.marks.push(mark);
        }
    }

    pub fn take_feed(&mut self) -> DemoFeed {
        self.feed.as_mut().map(std::mem::take).unwrap_or_default()
    }

    pub fn feed_file(&mut self, on: bool) {
        if !on {
            self.file = None;
        } else if self.file.is_none() {
            self.file = Some(FileFeed::default());
        }
    }

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

    pub fn collapsed(msg: &str, count: u32) -> String {
        match count {
            0 | 1 => msg.to_string(),
            n => format!("{msg} (×{n})"),
        }
    }

    pub fn logged(&self) -> u64 {
        self.logged
    }

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
        // The buffer's cap in entries; `docs/api/Debug.md` quotes it: change both.
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
