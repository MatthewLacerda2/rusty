//! The command channel's progress lines (#1999): while a command that asked for
//! them bakes, the handler writes each of its bake's progress lines before the
//! reply.
//!
//! Opt-in per command, as MCP's `progressToken` is per request: a client asks by
//! starting the line with [`ASK`], a Lua comment, so a window from before this
//! protocol evaluates the line as it is and replies once. A line without it gets
//! exactly one reply line, the old protocol, byte for byte.

/// The prefix a command line carries to ask for its bake's progress lines: a Lua
/// block comment, which an older window evaluates as nothing.
pub const ASK: &str = "--[[progress]] ";

/// `line` with the client's [`ASK`] taken off, and whether it asked.
pub fn asks(line: &str) -> (bool, &str) {
    match line.strip_prefix(ASK) {
        Some(rest) => (true, rest),
        None => (false, line),
    }
}

/// What the main loop sends a handler: a bake's progress line while the command
/// runs (#1999), then the command's one framed reply.
pub(in crate::dev) enum Reply {
    Progress(String),
    Done(String),
}

/// A progress line as the wire carries it: `{"progress":"…"}`, a line with no
/// `ok` key, so a client tells it from the reply.
pub fn progress_line(text: &str) -> String {
    serde_json::json!({ "progress": text }).to_string()
}

/// The text of a wire line that is a progress line, `None` for a reply.
pub fn parse_progress(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("ok").is_some() {
        return None;
    }
    value.get("progress")?.as_str().map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_progress_line_round_trips_and_a_reply_is_not_one() {
        let line = progress_line("[Lighting] 0:05 lightmaps 3%");
        assert_eq!(
            parse_progress(&line).as_deref(),
            Some("[Lighting] 0:05 lightmaps 3%")
        );
        assert_eq!(parse_progress(r#"{"ok":true,"result":"x"}"#), None);
        assert_eq!(parse_progress(r#"{"ok":false,"error":"progress"}"#), None);
        assert_eq!(parse_progress("not json"), None);
    }

    #[test]
    fn only_a_line_carrying_the_prefix_asks() {
        assert_eq!(
            asks("--[[progress]] Lighting.Generate()"),
            (true, "Lighting.Generate()")
        );
        assert_eq!(asks("Lighting.Generate()"), (false, "Lighting.Generate()"));
        assert_eq!(asks(" --[[progress]] x"), (false, " --[[progress]] x"));
    }

    /// A handler writes each progress line as it comes, then the reply, and stops.
    #[test]
    fn a_handler_writes_the_progress_then_the_reply() {
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Reply::Progress("a".into())).unwrap();
        tx.send(Reply::Done(r#"{"ok":true,"result":"2"}"#.into()))
            .unwrap();
        tx.send(Reply::Progress("never read".into())).unwrap();
        let mut out = Vec::new();
        assert!(super::super::transport::write_replies(&mut out, &rx));
        let text = String::from_utf8(out).unwrap();
        assert_eq!(
            text,
            "{\"progress\":\"a\"}\n{\"ok\":true,\"result\":\"2\"}\n"
        );
        let (gone, rx) = std::sync::mpsc::channel::<Reply>();
        drop(gone);
        let dropped = super::super::transport::write_replies(&mut Vec::new(), &rx);
        assert!(!dropped, "a main loop gone ends the connection");
    }
}
