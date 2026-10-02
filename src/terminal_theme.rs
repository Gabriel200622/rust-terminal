//! Offline terminal palettes and validated custom themes. No terminal-engine types.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[path = "theme_catalog.rs"]
mod catalog;
pub use catalog::BUNDLED;

/// An opaque sRGB color, serialized as a readable #RRGGBB string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HexColor(pub u32);

impl TryFrom<String> for HexColor {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let hex = value.strip_prefix('#').ok_or("Use #RRGGBB")?;
        if hex.len() != 6 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("Use #RRGGBB");
        }
        u32::from_str_radix(hex, 16)
            .map(Self)
            .map_err(|_| "Use #RRGGBB")
    }
}
impl From<HexColor> for String {
    fn from(value: HexColor) -> Self {
        format!("#{:06x}", value.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeColors {
    pub background: HexColor,
    pub foreground: HexColor,
    pub bold: HexColor,
    pub cursor: HexColor,
    pub cursor_text: HexColor,
    pub selection: HexColor,
    pub selection_text: HexColor,
    pub ansi: [HexColor; 16],
}

impl ThemeColors {
    pub const LABELS: [&str; 23] = [
        "Background",
        "Foreground",
        "Bold text",
        "Cursor",
        "Cursor text",
        "Selection",
        "Selected text",
        "Black",
        "Red",
        "Green",
        "Yellow",
        "Blue",
        "Magenta",
        "Cyan",
        "White",
        "Bright black",
        "Bright red",
        "Bright green",
        "Bright yellow",
        "Bright blue",
        "Bright magenta",
        "Bright cyan",
        "Bright white",
    ];
    pub const fn from_rgb(values: [u32; 23]) -> Self {
        let mut ansi = [HexColor(0); 16];
        let mut i = 0;
        while i < 16 {
            ansi[i] = HexColor(values[i + 7]);
            i += 1;
        }
        Self {
            background: HexColor(values[0]),
            foreground: HexColor(values[1]),
            bold: HexColor(values[2]),
            cursor: HexColor(values[3]),
            cursor_text: HexColor(values[4]),
            selection: HexColor(values[5]),
            selection_text: HexColor(values[6]),
            ansi,
        }
    }
    pub fn values(self) -> [HexColor; 23] {
        let mut result = [HexColor(0); 23];
        result[..7].copy_from_slice(&[
            self.background,
            self.foreground,
            self.bold,
            self.cursor,
            self.cursor_text,
            self.selection,
            self.selection_text,
        ]);
        result[7..].copy_from_slice(&self.ansi);
        result
    }
    pub fn is_dark(self) -> bool {
        let c = self.background.0;
        (299 * (c >> 16) + 587 * ((c >> 8) & 255) + 114 * (c & 255)) < 128_000
    }
}

pub struct BundledTheme {
    pub name: &'static str,
    pub colors: ThemeColors,
}

pub fn bundled(name: &str) -> Option<&'static BundledTheme> {
    BUNDLED
        .binary_search_by_key(&name, |theme| theme.name)
        .ok()
        .map(|i| &BUNDLED[i])
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomTheme {
    /// Stable across renames, separate from upstream names.
    pub id: String,
    pub name: String,
    pub colors: ThemeColors,
}

pub const MAX_CUSTOM_THEMES: usize = 128;
pub const MAX_NAME_CHARS: usize = 64;

impl CustomTheme {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.id
                .strip_prefix("custom:")
                .and_then(|id| id.parse::<u64>().ok())
                .is_some(),
            "Invalid custom theme ID"
        );
        ensure!(
            !self.name.trim().is_empty()
                && self.name.chars().count() <= MAX_NAME_CHARS
                && !self.name.chars().any(char::is_control),
            "Theme name must contain 1–64 characters without control characters"
        );
        ensure!(
            self.colors.values().iter().all(|c| c.0 <= 0xffffff),
            "Theme colors must be #RRGGBB"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_is_complete_unique_and_searchable() {
        assert_eq!(BUNDLED.len(), 712);
        for theme in BUNDLED {
            assert_eq!(bundled(theme.name).unwrap().colors, theme.colors);
            assert!(theme.colors.values().iter().all(|c| c.0 <= 0xffffff));
        }
        assert_eq!(
            bundled("Dracula").unwrap().colors.background,
            HexColor(0x282a36)
        );
    }
    #[test]
    fn hex_rejects_partial_or_non_rgb_colors() {
        for value in ["#fff", "ffffff", "#12345678", "#zzzzzz", "#é1234"] {
            assert!(HexColor::try_from(value.to_owned()).is_err());
        }
        assert_eq!(
            HexColor::try_from("#ABcdEF".to_owned()).unwrap(),
            HexColor(0xabcdef)
        );
    }
}
