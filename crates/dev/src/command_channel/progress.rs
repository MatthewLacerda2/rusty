/// A Lua block comment, so a line whose prefix is not stripped still evaluates as the
/// command alone.
pub const ASK: &str = "--[[progress]] ";

pub fn asks(line: &str) -> (bool, &str) {
    match line.strip_prefix(ASK) {
        Some(rest) => (true, rest),
        None => (false, line),
    }
}

pub(crate) enum Reply {
    Progress(String),
    Done(String),
}

pub fn progress_line(text: &str) -> String {
    serde_json::json!({ "progress": text }).to_string()
}

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
