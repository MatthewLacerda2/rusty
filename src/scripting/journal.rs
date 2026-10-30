//! src/scripting/journal.rs — what a host did to the world between ticks (#1581).
//!
//! Input reaches the sim through `InputState`, which a demo records tick by tick.
//! Everything else a host does to a playing world is an input too: a console or
//! MCP line run on the live runtime, a scenario's API call, an Inspector edit.
//! While a demo records, the journal keeps each such act in order, and the
//! recorder drains it at the next tick boundary, so a replay can do the same thing
//! before the same tick. An act the journal cannot carry as plain data is a
//! [`HostCall::Taint`]: the replay warns from that tick on rather than diverge in
//! silence.

/// A value crossing the API surface as plain data: what a recorded call's
/// arguments may hold. A function, a userdata or a cycle is not plain.
#[derive(Clone, Debug, PartialEq)]
pub enum Plain {
    Nil,
    Bool(bool),
    Int(i64),
    Number(f64),
    Str(String),
    /// A Lua sequence (`{1, 2, 3}`).
    List(Vec<Plain>),
    /// A Lua table with string keys, sorted by key.
    Map(Vec<(String, Plain)>),
}

/// One act of a host on the world, between two ticks.
#[derive(Clone, Debug, PartialEq)]
pub enum HostCall {
    /// A REPL line run on the live runtime (`ScriptManager::eval`: the console,
    /// MCP's `eval`, the command channel).
    Eval(String),
    /// An API function a host's own VM called (a scenario's
    /// `Transform.SetPosition(id, 1, 2, 3)`): `Namespace.Function` and its arguments.
    Call { function: String, args: Vec<Plain> },
    /// Something reached the world that cannot be replayed, said in a few words.
    Taint(String),
}

/// The host acts since the last tick boundary, kept only while a demo records.
#[derive(Debug, Default)]
pub struct HostJournal {
    recording: bool,
    calls: Vec<HostCall>,
}

impl HostJournal {
    /// Keep every act from now on (a demo started recording).
    pub fn start(&mut self) {
        self.recording = true;
        self.calls.clear();
    }

    /// Stop keeping acts and drop the ones not taken.
    pub fn stop(&mut self) {
        self.recording = false;
        self.calls.clear();
    }

    pub fn is_recording(&self) -> bool {
        self.recording
    }

    /// Note an act; a no-op unless a demo records.
    pub fn push(&mut self, call: HostCall) {
        if self.recording {
            self.calls.push(call);
        }
    }

    /// The acts since the last take, oldest first.
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
