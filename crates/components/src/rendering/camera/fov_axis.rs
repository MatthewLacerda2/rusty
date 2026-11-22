//! The stored `fov` is always the **vertical** angle (as Unity's `Camera.fieldOfView`); the axis
//! only changes how the Inspector shows and edits it, so flipping it never changes the draw.

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum FovAxis {
        #[default]
        Vertical = "Vertical",
        Horizontal = "Horizontal",
    }
}

/// Degrees, at `aspect` (width / height); Unity's `Camera.VerticalToHorizontalFieldOfView`.
pub fn vertical_to_horizontal_fov(vertical: f32, aspect: f32) -> f32 {
    let half = (vertical.to_radians() * 0.5).tan() * aspect;
    2.0 * half.atan().to_degrees()
}

/// Degrees; Unity's `Camera.HorizontalToVerticalFieldOfView`.
pub fn horizontal_to_vertical_fov(horizontal: f32, aspect: f32) -> f32 {
    let half = (horizontal.to_radians() * 0.5).tan() / aspect;
    2.0 * half.atan().to_degrees()
}

impl FovAxis {
    pub fn show(self, vertical: f32, aspect: f32) -> f32 {
        match self {
            Self::Vertical => vertical,
            Self::Horizontal => vertical_to_horizontal_fov(vertical, aspect),
        }
    }

    pub fn store(self, value: f32, aspect: f32) -> f32 {
        match self {
            Self::Vertical => value,
            Self::Horizontal => horizontal_to_vertical_fov(value, aspect),
        }
    }
}
