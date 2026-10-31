//! The editor captured with a demo open on the Game view's timeline (#1598): the
//! strip, its marks and the letterboxed replay, for a branch that changes them to
//! look at.

use std::path::Path;

use super::{capture_with, initial_ui, EditorCaptureOptions};
use crate::app::GameWorld;
use crate::core::application::DemoRequest;
use crate::dev::capture::CaptureHost;
use crate::scripting::LogLevel;

/// [`capture`](super::capture) with the demo at `demo` opened through
/// `Editor.OpenDemo`'s path and replayed to `tick`: `Err` when it does not open
/// (the console's reason) or `tick` is past its end.
pub fn capture_demo(
    game: &mut GameWorld,
    path: impl AsRef<Path>,
    opts: &EditorCaptureOptions,
    demo: &Path,
    tick: u64,
) -> Result<bool, String> {
    // The replay repeats a scenario's `Debug.*` calls: the editor's surface (#737).
    crate::dev::lua_surface::install_api();
    let mut ui = initial_ui(game, opts)?;
    let console_before = game.console().borrow().messages.len();
    let open = DemoRequest::Open(demo.to_string_lossy().into_owned());
    for request in [open, DemoRequest::Seek(tick)] {
        game.resources
            .application
            .borrow_mut()
            .request_demo(request);
        ui.advance_demo(game, 0.0);
        // Only the timeline's own refusals: a replay that diverges or a script
        // error in it is still a demo to look at.
        let refused = (game.console().borrow().messages.iter())
            .skip(console_before)
            .find(|(text, level)| *level == LogLevel::Error && is_refusal(text))
            .map(|(text, _)| text.clone());
        if let Some(reason) = refused {
            return Err(reason);
        }
    }
    if !ui.demo_open() {
        return Err(format!("{} did not open", demo.display()));
    }
    let at = game.resources.application.borrow().demo_tick();
    if at.map(|(current, _)| current) != Some(tick) {
        return Err(format!("the replay stands at {at:?}, not tick {tick}"));
    }
    capture_with(&mut CaptureHost::new(), game, &mut ui, path, opts)
}

/// A console line the timeline writes when it refuses to open or seek.
fn is_refusal(text: &str) -> bool {
    ["Editor.OpenDemo:", "Editor.SeekDemo:", "Editor demo:"]
        .iter()
        .any(|prefix| text.starts_with(prefix))
}
