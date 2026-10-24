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
        if let Some((text, count)) = &self.repeat {
            let newest = self.messages.last();
            if *text == msg && newest == Some(&(Self::collapsed(text, *count), level.clone())) {
                let count = count + times;
                if let Some(entry) = self.messages.last_mut() {
                    entry.0 = Self::collapsed(&msg, count);
                }
                self.repeat = Some((msg, count));
                return;
            }
        }
        self.add_log(Self::collapsed(&msg, times), level);
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
        self.logged += 1;
        self.repeat = None;
        if self.messages.len() >= 100 {
            self.messages.remove(0);
        }
        self.messages.push((msg, level));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeated_record_counts_up_on_the_newest_entry() {
        let mut log = ConsoleLogs::new();
        log.repeated("[Physics] skipped".into(), LogLevel::Warning, 1);
        log.repeated("[Physics] skipped".into(), LogLevel::Warning, 2);
        assert_eq!(log.messages.len(), 1);
        assert_eq!(log.messages[0].0, "[Physics] skipped (×3)");
    }

    #[test]
    fn another_entry_between_starts_a_new_run() {
        let mut log = ConsoleLogs::new();
        log.repeated("[Asset] gone".into(), LogLevel::Warning, 1);
        log.info("script output".into());
        log.repeated("[Asset] gone".into(), LogLevel::Warning, 1);
        log.repeated("[Asset] gone".into(), LogLevel::Error, 1);
        let texts: Vec<_> = log.messages.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(
            texts,
            [
                "[Asset] gone",
                "script output",
                "[Asset] gone",
                "[Asset] gone"
            ]
        );
    }

    #[test]
    fn since_reads_what_came_after_the_mark_even_past_the_cap() {
        let mut log = ConsoleLogs::new();
        log.info("before".into());
        let mark = log.logged();
        assert_eq!(log.since(mark).count(), 0);
        for i in 0..150 {
            log.error(format!("e{i}"));
        }
        let after: Vec<_> = log.since(mark).map(|(m, _)| m.as_str()).collect();
        assert_eq!(after.len(), 100, "only what the buffer still holds");
        assert_eq!((after[0], after[99]), ("e50", "e149"));
    }
}
