//! [`PixelSize`]: render's own width × height in physical pixels, so no render API
//! has to name the windowing crate to say how big something is (#988).

/// A width × height in physical pixels — a view's targets, the swapchain, a UI pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PixelSize {
    pub width: u32,
    pub height: u32,
}

impl PixelSize {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}
