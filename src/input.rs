use std::collections::HashSet;

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
}
