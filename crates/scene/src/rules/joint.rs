use super::sanitize::non_negative;

pub fn drive_value(v: f32) -> f32 {
    non_negative(v)
}
