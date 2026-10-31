//! What the host asks of the run, and the run of its host: quit, Play and
//! standalone (split out of the module root for size).

use super::Application;

impl Application {
    /// Record that the game asked to quit (`Application.Quit()`).
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    /// Whether a quit is pending, without clearing it (the harness keeps the run ended).
    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    /// Consume a pending quit request: `true` exactly once per `Quit()` call.
    pub fn take_quit_request(&mut self) -> bool {
        std::mem::take(&mut self.quit_requested)
    }

    /// Whether the host asked for Play: the state the next transition heads to.
    pub fn host_play(&self) -> bool {
        self.host_play
    }

    /// Record the host's own request (`GameWorld::set_playing`).
    pub fn set_host_play(&mut self, playing: bool) {
        self.host_play = playing;
    }

    /// Where the next transition heads: a script's request not yet taken, else the
    /// host's.
    pub fn play_heading(&self) -> bool {
        self.play_request.unwrap_or(self.host_play)
    }

    /// Ask the host to enter (`true`) or leave (`false`) Play at the next frame
    /// boundary (#1360). A later request before that boundary replaces this one.
    pub fn request_play(&mut self, playing: bool) {
        self.play_request = Some(playing);
    }

    /// Consume the pending play request: the host's frame boundary calls it once.
    pub fn take_play_request(&mut self) -> Option<bool> {
        self.play_request.take()
    }

    /// Whether this is a standalone run, with no edit mode to return to.
    pub fn standalone(&self) -> bool {
        self.standalone
    }

    /// Mark the run standalone, once, at boot (`GameWorld::boot_standalone`).
    pub fn mark_standalone(&mut self) {
        self.standalone = true;
    }
}
