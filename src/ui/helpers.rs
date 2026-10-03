//! Label and window helpers, plus the shared controls of the visual system.
pub use super::controls::*;
use eframe::egui::{self, Rect, Sense, Vec2};
pub fn compact_path(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    if let Some(home) = directories::BaseDirs::new()
        && let Ok(relative) = path.strip_prefix(home.home_dir())
    {
        return format!("~/{}", relative.display());
    }
    text
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

/// Pull request numbers that open their pull request, laid out leading from
/// a trailing edge. Interaction is claimed first so the surface beneath can
/// follow their hover; `paint` then draws them over it.
pub struct PullRequestChips<'a> {
    /// A number for each link, or the newest number standing for all of them.
    chips: Vec<(Rect, std::sync::Arc<egui::Galley>, egui::Response)>,
    links: &'a [neptune_model::PullRequest],
    menu: bool,
    /// Leading edge of the chips; the trailing edge given when there are none.
    pub left: f32,
}
impl<'a> PullRequestChips<'a> {
    /// The most links shown side by side; more are listed in a menu.
    const SIDE_BY_SIDE: usize = 2;

    /// The newest link sits at `right` with the older one before it. More
    /// links, or links without room to stay clear of `limit`, share one chip
    /// that lists them.
    pub fn layout(
        ui: &egui::Ui,
        id: egui::Id,
        links: &'a [neptune_model::PullRequest],
        (limit, right, middle): (f32, f32, f32),
        accent: egui::Color32,
    ) -> Self {
        let number = |link: &neptune_model::PullRequest| {
            ui.painter().layout_no_wrap(
                link.number().to_string(),
                crate::theme::medium(11.5),
                accent,
            )
        };
        let place = |left: f32, width: f32| {
            Rect::from_min_max(
                egui::pos2(left - width, middle - 9.0),
                egui::pos2(left, middle + 9.0),
            )
        };
        let mut chips = Vec::new();
        let mut left = right;
        let numbers: Vec<_> = links.iter().rev().map(number).collect();
        let side_by_side: f32 = numbers.iter().map(|n| n.size().x + 29.0).sum();
        let menu = links.len() > Self::SIDE_BY_SIDE || right - side_by_side < limit;
        if !menu {
            for (link, number) in links.iter().rev().zip(numbers) {
                let chip = place(left, number.size().x + 28.0);
                left = chip.left() - 1.0;
                let response = ui.interact(chip, id.with(link.url()), Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Link,
                        true,
                        format!("Open pull request {}", link.label()),
                    )
                });
                chips.push((chip, number, response));
            }
        } else if let Some(number) = numbers.into_iter().next()
            && links.len() > 1
            && right - (number.size().x + 41.0) >= limit
        {
            let chip = place(left, number.size().x + 41.0);
            left = chip.left() - 1.0;
            let response = ui.interact(chip, id.with("menu"), Sense::click());
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Pull requests")
            });
            chips.push((chip, number, response));
        }
        Self {
            chips,
            links,
            menu,
            left,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.chips.is_empty()
    }
    pub fn hovered(&self) -> bool {
        self.chips.iter().any(|chip| chip.2.hovered())
    }
    pub fn paint(
        self,
        painter: &egui::Painter,
        p: crate::theme::Palette,
        actions: &mut Vec<super::Action>,
    ) {
        use crate::icons::{self, Icon};
        let open = |link: &neptune_model::PullRequest, actions: &mut Vec<super::Action>| {
            if let Some(link) = crate::platform::links::WebLink::new(link.url()) {
                actions.push(super::Action::OpenLink(link));
            }
        };
        // Chips run newest first, as the links do from their end.
        for ((chip, number, response), link) in self.chips.into_iter().zip(self.links.iter().rev())
        {
            let listing = self.menu
                && egui::Popup::is_id_open(
                    &response.ctx,
                    egui::Popup::default_response_id(&response),
                );
            if response.hovered() || listing {
                painter.rect_filled(chip, 5, crate::theme::tint(p.accent, 0.14));
            }
            icons::paint(
                painter,
                Rect::from_center_size(
                    egui::pos2(chip.left() + 11.5, chip.center().y),
                    Vec2::splat(13.0),
                ),
                Icon::PullRequest,
                p.accent,
            );
            galley_at(
                painter,
                egui::pos2(chip.left() + 22.0, chip.center().y + 1.0),
                number,
            );
            let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
            if !self.menu {
                if response
                    .on_hover_text(format!("Open pull request {}", link.label()))
                    .clicked()
                {
                    open(link, actions);
                }
                continue;
            }
            icons::paint(
                painter,
                Rect::from_center_size(
                    egui::pos2(chip.right() - 10.0, chip.center().y + 0.5),
                    Vec2::splat(10.0),
                ),
                Icon::ChevronDown,
                p.accent,
            );
            egui::Popup::menu(&response).show(|ui| {
                // As wide as its longest entry, like the other menus' rows.
                let widest = self
                    .links
                    .iter()
                    .map(|link| {
                        let text = crate::theme::regular(13.0);
                        ui.painter()
                            .layout_no_wrap(link.label(), text, p.fg)
                            .size()
                            .x
                    })
                    .fold(0.0, f32::max);
                menu_layout(ui, (widest + 46.0).clamp(120.0, 320.0));
                for link in self.links.iter().rev() {
                    if menu_item(ui, p, Icon::PullRequest, &link.label(), "", false) {
                        open(link, actions);
                        ui.close();
                    }
                }
            });
            if !listing {
                response.on_hover_text(format!("{} pull requests", self.links.len()));
            }
        }
    }
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
