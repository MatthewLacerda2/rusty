//! src/shell/editor/icon.rs — the rusty logo as the editor's window and Dock icon (#669).
//!
//! The PNGs are embedded, so the editor needs no file on disk at run time. Linux/X11
//! takes the window icon from winit. macOS has no per-window icon, so the Dock icon is
//! set on `NSApplication` instead, which works for an unbundled `cargo run` binary.
//! Wayland compositors read the icon from a `.desktop` file, not the window, so there
//! it is a no-op. Only the editor calls this: a shipped game shows its own icon.

use winit::window::{Icon, Window};

/// The window icon: winit hands it to X11's `_NET_WM_ICON`.
const WINDOW_ICON_PNG: &[u8] = include_bytes!("../../../assets/icon/rusty-256.png");
/// The Dock icon: 512 px fills a 256 pt Retina Dock tile.
#[cfg(any(target_os = "macos", test))]
const DOCK_ICON_PNG: &[u8] = include_bytes!("../../../assets/icon/rusty-512.png");

/// Give `window` (and, on macOS, the app's Dock tile) the rusty logo.
pub fn apply(window: &Window) {
    window.set_window_icon(window_icon());
    #[cfg(target_os = "macos")]
    macos::set_dock_icon(DOCK_ICON_PNG);
}

fn window_icon() -> Option<Icon> {
    let rgba = decode(WINDOW_ICON_PNG)?;
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).ok()
}

/// Decode an embedded PNG to RGBA; `None` only if the embedded bytes are corrupt.
fn decode(png: &[u8]) -> Option<image::RgbaImage> {
    image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()
        .map(|img| img.into_rgba8())
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    /// Set the Dock icon from PNG bytes. Off the main thread (never, in the editor)
    /// or on bytes AppKit can't read, the Dock keeps its default.
    pub fn set_dock_icon(png: &[u8]) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let data = NSData::with_bytes(png);
        let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        // SAFETY: on the main thread (`mtm`), with a live `NSImage`; AppKit retains it.
        unsafe { app.setApplicationIconImage(Some(&image)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icons_decode_to_their_sizes() {
        for (png, side) in [(WINDOW_ICON_PNG, 256), (DOCK_ICON_PNG, 512)] {
            let rgba = decode(png).expect("an embedded icon is a valid PNG");
            assert_eq!(rgba.dimensions(), (side, side));
        }
    }

    #[test]
    fn the_window_icon_builds() {
        assert!(window_icon().is_some());
    }
}
