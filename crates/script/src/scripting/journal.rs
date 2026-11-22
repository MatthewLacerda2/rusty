//! A host's acts on a playing world besides input (a REPL line, a scenario's API call),
//! kept in order while a demo records so its replay repeats them at the same tick.

#[derive(Clone, Debug, PartialEq)]
pub enum Plain {
    Nil,
    Bool(bool),
    Int(i64),
    Number(f64),
    Str(String),
    List(Vec<Plain>),
    /// String keys, sorted by key.
    Map(Vec<(String, Plain)>),
}

/// `Taint` is an act no replay can repeat, in a few words: the replay warns it may differ
/// from that tick on.
#[derive(Clone, Debug, PartialEq)]
pub enum HostCall {
    Eval(String),
    Call { function: String, args: Vec<Plain> },
    Taint(String),
}

#[derive(Debug, Default)]
pub struct HostJournal {
    recording: bool,
    calls: Vec<HostCall>,
}

impl HostJournal {
    pub fn start(&mut self) {
        self.recording = true;
        self.calls.clear();
    }

    pub fn stop(&mut self) {
        self.recording = false;
        self.calls.clear();
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    pub fn push(&mut self, call: HostCall) {
        if self.recording {
            self.calls.push(call);
        }
    }

    pub fn take(&mut self) -> Vec<HostCall> {
        std::mem::take(&mut self.calls)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_recording_journal_keeps_acts() {
        let mut journal = HostJournal::default();
        journal.push(HostCall::Eval("x = 1".into()));
        assert!(journal.take().is_empty(), "nothing records by default");
        journal.start();
        journal.push(HostCall::Taint("an Inspector edit".into()));
        assert_eq!(journal.take().len(), 1);
        assert!(journal.take().is_empty(), "a take drains");
        journal.push(HostCall::Eval("y = 2".into()));
        journal.stop();
        assert!(journal.take().is_empty(), "stopping drops what was left");
    }
}
