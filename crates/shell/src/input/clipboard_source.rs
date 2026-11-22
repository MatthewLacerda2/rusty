//! crates/shell/src/input/clipboard_source.rs — the real [`ClipboardSource`].

use super::clipboard::ClipboardSource;

/// The OS clipboard for this window.
pub enum OsClipboard {
    #[cfg(target_os = "linux")]
    Wayland(smithay_clipboard::Clipboard),
    Arboard(arboard::Clipboard),
}

impl OsClipboard {
    /// Open the clipboard `window` can reach; `None` (logged) where there is none.
    pub fn open(window: &winit::window::Window) -> Option<Self> {
        #[cfg(target_os = "linux")]
        if let Some(clipboard) = wayland(window) {
            return Some(Self::Wayland(clipboard));
        }
        let _ = window;
        match arboard::Clipboard::new() {
            Ok(clipboard) => Some(Self::Arboard(clipboard)),
            Err(err) => {
                log::warn!("[Input] clipboard unavailable: {err}");
                None
            }
        }
    }
}

/// The Wayland clipboard, when the window is a Wayland one.
#[cfg(target_os = "linux")]
fn wayland(window: &winit::window::Window) -> Option<smithay_clipboard::Clipboard> {
    use winit::raw_window_handle::{HasDisplayHandle, RawDisplayHandle};
    let RawDisplayHandle::Wayland(display) = window.display_handle().ok()?.as_raw() else {
        return None;
    };
    // SAFETY: `display` is winit's live `wl_display`, owned by the event loop, which
    // outlives the shell holding this clipboard (egui-winit does exactly this).
    Some(unsafe { smithay_clipboard::Clipboard::new(display.display.as_ptr()) })
}

impl ClipboardSource for OsClipboard {
    fn read(&mut self) -> Option<String> {
        match self {
            #[cfg(target_os = "linux")]
            Self::Wayland(clipboard) => clipboard.load().ok(),
            Self::Arboard(clipboard) => clipboard.get_text().ok(),
        }
    }

    fn write(&mut self, text: &str) {
        match self {
            #[cfg(target_os = "linux")]
            Self::Wayland(clipboard) => clipboard.store(text),
            Self::Arboard(clipboard) => {
                if let Err(err) = clipboard.set_text(text) {
                    log::warn!("[Input] clipboard write failed: {err}");
                }
            }
        }
    }
}
