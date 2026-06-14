use std::collections::HashSet;

/// Frame input as a swappable *source*: the simulation reads this, but it does not
/// care who wrote it — winit (windowed play), the headless harness, or a bot-player
/// script all drive the same state through the same writable API.
#[derive(Clone, Debug)]
pub struct InputState {
    pub keys_pressed: HashSet<String>,
    pub mouse_left_clicked: bool,
    pub space_pressed: bool,
    pub mouse_position: (f64, f64),
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            keys_pressed: HashSet::new(),
            mouse_left_clicked: false,
            space_pressed: false,
            mouse_position: (0.0, 0.0),
        }
    }
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_key_state(&mut self, key_name: &str, pressed: bool) {
        if pressed {
            self.keys_pressed.insert(key_name.to_uppercase());
        } else {
            self.keys_pressed.remove(&key_name.to_uppercase());
        }
    }

    pub fn is_key_down(&self, key_name: &str) -> bool {
        self.keys_pressed.contains(&key_name.to_uppercase())
    }

    /// Press a key (for scripted/bot drivers that don't go through winit).
    pub fn press(&mut self, key_name: &str) {
        self.set_key_state(key_name, true);
    }

    /// Release a key (for scripted/bot drivers that don't go through winit).
    pub fn release(&mut self, key_name: &str) {
        self.set_key_state(key_name, false);
    }
}
