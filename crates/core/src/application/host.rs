use super::Application;

impl Application {
    pub fn request_quit(&mut self) {
        self.quit_requested = true;
    }

    /// Without clearing it: the harness keeps the run ended.
    pub fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    pub fn take_quit_request(&mut self) -> bool {
        std::mem::take(&mut self.quit_requested)
    }

    pub fn host_play(&self) -> bool {
        self.host_play
    }

    pub fn set_host_play(&mut self, playing: bool) {
        self.host_play = playing;
    }

    pub fn play_heading(&self) -> bool {
        self.play_request.unwrap_or(self.host_play)
    }

    /// A later request before the frame boundary replaces this one.
    pub fn request_play(&mut self, playing: bool) {
        self.play_request = Some(playing);
    }

    pub fn take_play_request(&mut self) -> Option<bool> {
        self.play_request.take()
    }

    pub fn standalone(&self) -> bool {
        self.standalone
    }

    pub fn mark_standalone(&mut self) {
        self.standalone = true;
    }

    pub fn mark_bake_host(&mut self) {
        self.bake_host = true;
    }

    /// Only the editor window steps a background bake (`Lighting.GenerateAsync()`).
    pub fn bake_host(&self) -> bool {
        self.bake_host
    }
}
