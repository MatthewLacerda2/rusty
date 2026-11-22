/// Lines the history keeps.
const MAX_HISTORY: usize = 100;

#[derive(Default)]
pub struct ReplInput {
    pub buffer: String,
    history: Vec<String>,
    recall: Option<usize>,
    draft: String,
}

impl ReplInput {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

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
        r.recall_prev();
        assert_eq!(r.buffer, "one");
        r.recall_next();
        assert_eq!(r.buffer, "two");
        r.recall_next();
        assert_eq!(r.buffer, "three");
        r.recall_next();
        assert_eq!(r.buffer, "draft");
        r.recall_next();
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
