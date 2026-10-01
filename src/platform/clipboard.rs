//! Clipboard reads and writes stay outside terminal rendering and the model.

pub trait Clipboard {
    fn read(&mut self) -> Result<String, String>;
    fn write(&mut self, text: &str) -> Result<(), String>;
}

/// Opens the native clipboard only when an operation is requested.
#[derive(Default)]
pub struct DesktopClipboard;

impl Clipboard for DesktopClipboard {
    fn read(&mut self) -> Result<String, String> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .map_err(|error| error.to_string())
    }

    fn write(&mut self, text: &str) -> Result<(), String> {
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(text))
            .map_err(|error| error.to_string())
    }
}

/// A clipboard whose contents belong to a test, independent of the desktop.
#[derive(Default, Debug)]
pub struct MemoryClipboard {
    pub text: String,
    pub writes: usize,
}

impl Clipboard for MemoryClipboard {
    fn read(&mut self) -> Result<String, String> {
        Ok(self.text.clone())
    }

    fn write(&mut self, text: &str) -> Result<(), String> {
        self.text = text.to_owned();
        self.writes += 1;
        Ok(())
    }
}

/// egui defers copy operations until the frame output reaches the window host.
pub fn copy(ctx: &eframe::egui::Context, text: String) {
    ctx.copy_text(text);
}

pub fn read() -> Result<String, String> {
    DesktopClipboard.read()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_clipboard_round_trips_unicode_without_desktop_access() {
        let mut clipboard = MemoryClipboard::default();
        clipboard.write("你好 café 🦀").unwrap();
        assert_eq!(clipboard.read().unwrap(), "你好 café 🦀");
        assert_eq!(clipboard.writes, 1);
    }
}
