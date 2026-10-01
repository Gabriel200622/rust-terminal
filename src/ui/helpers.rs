//! Label and window helpers, plus the shared controls of the visual system.
pub use super::controls::*;
use eframe::egui::{self, Rect, Sense, Vec2};
use std::path::PathBuf;
pub fn compact_path(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    if let Some(home) = directories::BaseDirs::new()
        && let Ok(relative) = path.strip_prefix(home.home_dir())
    {
        return format!("~/{}", relative.display());
    }
    text
}
pub fn expand_home(text: &str) -> PathBuf {
    if (text == "~" || text.starts_with("~/"))
        && let Some(home) = directories::BaseDirs::new()
    {
        return home.home_dir().join(text.strip_prefix("~/").unwrap_or(""));
    }
    PathBuf::from(text)
}
pub fn ellipsize(text: &str, max: usize) -> String {
    if text.chars().count() > max {
        format!(
            "{}…",
            text.chars().take(max.saturating_sub(1)).collect::<String>()
        )
    } else {
        text.into()
    }
}
pub fn regex_escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if ".+*?()|[]{}^$\\".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

pub fn tail_label(text: &str, max: usize) -> String {
    let len = text.chars().count();
    if max == 0 {
        return String::new();
    }
    if len > max {
        format!(
            "…{}",
            text.chars()
                .skip(len - max.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        text.into()
    }
}

pub fn resize_edges(ui: &mut egui::Ui, bounds: Rect) {
    if ui.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    use egui::ResizeDirection::*;
    for (dir, rect, cursor) in [
        (
            North,
            Rect::from_min_size(
                bounds.min + Vec2::new(8.0, 0.0),
                Vec2::new(bounds.width() - 16.0, 4.0),
            ),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            South,
            Rect::from_min_size(
                bounds.left_bottom() - Vec2::new(-8.0, 4.0),
                Vec2::new(bounds.width() - 16.0, 4.0),
            ),
            egui::CursorIcon::ResizeVertical,
        ),
        (
            West,
            Rect::from_min_size(
                bounds.min + Vec2::new(0.0, 8.0),
                Vec2::new(4.0, bounds.height() - 16.0),
            ),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            East,
            Rect::from_min_size(
                bounds.right_top() + Vec2::new(-4.0, 8.0),
                Vec2::new(4.0, bounds.height() - 16.0),
            ),
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            NorthWest,
            Rect::from_min_size(bounds.min, Vec2::splat(8.0)),
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            NorthEast,
            Rect::from_min_size(bounds.right_top() - Vec2::new(8.0, 0.0), Vec2::splat(8.0)),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            SouthWest,
            Rect::from_min_size(bounds.left_bottom() - Vec2::new(0.0, 8.0), Vec2::splat(8.0)),
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            SouthEast,
            Rect::from_min_size(bounds.max - Vec2::splat(8.0), Vec2::splat(8.0)),
            egui::CursorIcon::ResizeNwSe,
        ),
    ] {
        let r = ui
            .interact(
                rect,
                ui.id().with(("window-resize", format!("{dir:?}"))),
                Sense::drag(),
            )
            .on_hover_cursor(cursor);
        if r.drag_started() {
            crate::platform::window::send(
                ui.ctx(),
                crate::platform::window::WindowOperation::Resize(dir),
            );
        }
    }
}
pub fn path_label(path: &std::path::Path, max: usize) -> String {
    let full = compact_path(path);
    if full.chars().count() <= max {
        return full;
    }
    let mut tail = String::new();
    for component in path.components().rev() {
        let segment = component.as_os_str().to_string_lossy();
        let candidate = if tail.is_empty() {
            segment.to_string()
        } else {
            format!("{segment}/{tail}")
        };
        if candidate.chars().count() + 2 > max {
            break;
        }
        tail = candidate;
    }
    if tail.is_empty() {
        tail = ellipsize(
            &path.file_name().unwrap_or_default().to_string_lossy(),
            max.saturating_sub(2),
        );
    }
    format!("…/{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_labels_truncate_safely() {
        assert_eq!(ellipsize("日本語の端末", 4), "日本語…");
        assert_eq!(ellipsize("", 0), "");
        assert_eq!(regex_escape("a.b[0]"), "a\\.b\\[0\\]");
    }
}
