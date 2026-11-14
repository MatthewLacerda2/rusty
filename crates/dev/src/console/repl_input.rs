//! The REPL input line and its shell-style history recall.

/// Most lines history keeps; the oldest is dropped past it.
const MAX_HISTORY: usize = 100;

/// A single line of REPL input plus a bounded history, owned by whatever UI hosts
/// the console (the editor's bottom panel). Kept separate from `ConsoleLogs` so
/// the log buffer stays a pure sink with no UI state.
///
/// Recall works like a shell's readline: `recall_prev` (Up) walks back through
/// submitted lines, `recall_next` (Down) walks forward and, past the newest,
/// restores the line that was being typed. Recalled text is a copy in `buffer`;
/// history entries are never edited.
#[derive(Default)]
pub struct ReplInput {
    /// The text currently being edited in the input line.
    pub buffer: String,
    /// Previously submitted lines, newest last.
    history: Vec<String>,
    /// Index into `history` while recalling; `None` when editing a fresh line.
    recall: Option<usize>,
    /// The line being typed when recall began, restored by walking past the newest.
    draft: String,
}

impl ReplInput {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Take the current buffer, record it in history (unless blank or an immediate
    /// repeat of the previous entry), and clear the buffer and any recall.
    /// Returns `None` if the buffer is blank.
    pub fn take_submit(&mut self) -> Option<String> {
        let line = std::mem::take(&mut self.buffer);
        self.recall = None;
        self.draft.clear();
        if line.trim().is_empty() {
            return None;
        }
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
            if self.history.len() > MAX_HISTORY {
                self.history.remove(0);
            }
        }
        Some(line)
    }

    #[cfg(test)]
    pub fn history(&self) -> &[String] {
        &self.history
    }

    /// Up: replace the buffer with the previous entry (stays on the oldest).
    pub fn recall_prev(&mut self) {
        let next = match self.recall {
            None if self.history.is_empty() => return,
            None => {
                self.draft = std::mem::take(&mut self.buffer);
                self.history.len() - 1
            }
            Some(i) => i.saturating_sub(1),
        };
        self.recall = Some(next);
        self.buffer = self.history[next].clone();
    }

    /// Down: replace the buffer with the next entry; past the newest, restore the draft.
    pub fn recall_next(&mut self) {
        let Some(i) = self.recall else { return };
        if i + 1 < self.history.len() {
            self.recall = Some(i + 1);
            self.buffer = self.history[i + 1].clone();
        } else {
            self.recall = None;
            self.buffer = std::mem::take(&mut self.draft);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(lines: &[&str]) -> ReplInput {
        let mut r = ReplInput::new();
        for l in lines {
            r.buffer = (*l).to_string();
            r.take_submit();
        }
        r
    }

    #[test]
    fn take_submit_records_history() {
        let mut input = ReplInput::new();
        assert!(input.take_submit().is_none());
        input.buffer = "  ".to_string();
        assert!(input.take_submit().is_none());
        input.buffer = "print(1)".to_string();
        assert_eq!(input.take_submit().as_deref(), Some("print(1)"));
        assert!(input.buffer.is_empty());
        assert_eq!(input.history(), &["print(1)".to_string()]);
    }

    #[test]
    fn immediate_repeat_is_not_recorded_but_still_returned() {
        let mut r = with(&["a", "b"]);
        r.buffer = "b".into();
        assert_eq!(r.take_submit().as_deref(), Some("b"));
        r.buffer = "a".into();
        r.take_submit();
        assert_eq!(r.history(), ["a", "b", "a"]);
    }

    #[test]
    fn up_down_walk_and_restore_draft() {
        let mut r = with(&["one", "two", "three"]);
        r.buffer = "draft".into();
        r.recall_prev();
        assert_eq!(r.buffer, "three");
        r.recall_prev();
        r.recall_prev();
        assert_eq!(r.buffer, "one");
        r.recall_prev(); // stays on the oldest
        assert_eq!(r.buffer, "one");
        r.recall_next();
        assert_eq!(r.buffer, "two");
        r.recall_next();
        assert_eq!(r.buffer, "three");
        r.recall_next(); // past the newest
        assert_eq!(r.buffer, "draft");
        r.recall_next(); // not recalling: no-op
        assert_eq!(r.buffer, "draft");
    }

    #[test]
    fn empty_history_up_keeps_buffer() {
        let mut r = ReplInput::new();
        r.buffer = "typing".into();
        r.recall_prev();
        assert_eq!(r.buffer, "typing");
    }

    #[test]
    fn editing_a_recalled_line_leaves_history_intact() {
        let mut r = with(&["print(1)"]);
        r.recall_prev();
        r.buffer.push('0');
        assert_eq!(r.history(), ["print(1)"]);
        assert_eq!(r.take_submit().as_deref(), Some("print(1)0"));
        assert_eq!(r.history(), ["print(1)", "print(1)0"]);
        r.recall_prev();
        assert_eq!(r.buffer, "print(1)0");
    }

    #[test]
    fn history_is_bounded_dropping_the_oldest() {
        let mut r = ReplInput::new();
        for n in 0..MAX_HISTORY + 5 {
            r.buffer = n.to_string();
            r.take_submit();
        }
        assert_eq!(r.history().len(), MAX_HISTORY);
        assert_eq!(r.history()[0], "5");
        assert_eq!(r.history().last().unwrap(), &(MAX_HISTORY + 4).to_string());
    }
}
