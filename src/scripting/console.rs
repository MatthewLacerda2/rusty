/// The console log buffer and severity levels shared by gameplay scripts, the
/// REPL and the editor's bottom panel. The `print` hook and every `Debug.*`
/// binding funnel through here.
pub struct ConsoleLogs {
    pub messages: Vec<(String, LogLevel)>,
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

    fn add_log(&mut self, msg: String, level: LogLevel) {
        if self.messages.len() >= 100 {
            self.messages.remove(0);
        }
        self.messages.push((msg, level));
    }
}
