//! Link hit testing uses only the visible, owned cells and terminal columns.

use super::Cache;
use crate::platform::links::{MAX_URL_BYTES, WebLink};
use eframe::egui::{self, Rect};
use terminal_core::{Cell, Flags, Point};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Link {
    pub url: WebLink,
    /// Inclusive linear cell indices in the visible viewport.
    pub start: usize,
    pub end: usize,
}

impl Cache {
    pub(super) fn link_interaction(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        rect: Rect,
    ) -> (Option<Link>, Option<WebLink>) {
        self.link_pointer_owned = self.pressed_link.is_some();
        let hit = |cache: &Self, pos: egui::Pos2| {
            if !rect.contains(pos)
                || pos.x >= rect.left() + f32::from(cache.columns) * cache.cell.x
                || pos.y >= rect.top() + f32::from(cache.lines) * cache.cell.y
            {
                return None;
            }
            cache.link_at(super::geometry::point_at(
                rect,
                cache.cell,
                cache.columns,
                cache.lines,
                cache.display_offset,
                pos,
            ))
        };
        let mut open = None;
        let contains_pointer = response.contains_pointer();
        let clicked = response.clicked_by(egui::PointerButton::Primary);
        ui.input(|input| {
            for event in &input.events {
                let egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers,
                } = event
                else {
                    continue;
                };
                if *pressed && contains_pointer && link_modifier(*modifiers) {
                    self.pressed_link = hit(self, *pos);
                    self.link_pointer_owned |= self.pressed_link.is_some();
                } else if !pressed && let Some(pressed) = self.pressed_link.take() {
                    self.link_pointer_owned = true;
                    if clicked
                        && link_modifier(*modifiers)
                        && hit(self, *pos).as_ref() == Some(&pressed)
                    {
                        open = Some(pressed.url);
                    }
                }
            }
        });
        if !ui.input(|input| input.focused && input.pointer.primary_down()) {
            self.pressed_link = None;
        }
        let hovered =
            if response.contains_pointer() && ui.input(|input| link_modifier(input.modifiers)) {
                ui.input(|input| input.pointer.hover_pos())
                    .and_then(|pos| hit(self, pos))
            } else {
                None
            };
        (hovered, open)
    }

    pub(super) fn link_at(&self, point: Point) -> Option<Link> {
        let row = usize::try_from(point.line + self.display_offset as i32).ok()?;
        let columns = usize::from(self.columns);
        if row >= self.sources.len() || point.column >= columns {
            return None;
        }
        let mut index = row * columns + point.column;
        if self
            .source_cell(index)?
            .flags
            .contains(Flags::WIDE_CHAR_SPACER)
        {
            index = index.checked_sub(1)?;
        }
        let cell = self.source_cell(index)?;
        if cell
            .flags
            .intersects(Flags::HIDDEN | Flags::LEADING_WIDE_CHAR_SPACER)
        {
            return None;
        }
        if let Some(target) = &cell.hyperlink {
            let url = WebLink::new(target)?;
            let same_link = |index| {
                self.source_cell(index).is_some_and(|cell| {
                    !cell.flags.contains(Flags::HIDDEN)
                        && cell.hyperlink.as_deref() == Some(target.as_ref())
                })
            };
            let mut start = index;
            let mut end = index;
            while start > 0 && index - start < MAX_URL_BYTES && same_link(start - 1) {
                start -= 1;
            }
            while end - start < MAX_URL_BYTES && same_link(end + 1) {
                end += 1;
            }
            return Some(Link { url, start, end });
        }

        // Inspect just the token under the pointer, crossing soft wraps but
        // never hard line breaks. Work and temporary storage are bounded.
        if delimiter(cell) {
            return None;
        }
        let mut start = index;
        while start > 0
            && index - start < MAX_URL_BYTES
            && self.connected(start - 1, start)
            && self
                .source_cell(start - 1)
                .is_some_and(|cell| !delimiter(cell))
        {
            start -= 1;
        }
        if index - start == MAX_URL_BYTES {
            return None;
        }
        let mut end = index;
        while end - start < MAX_URL_BYTES
            && self.connected(end, end + 1)
            && self
                .source_cell(end + 1)
                .is_some_and(|cell| !delimiter(cell))
        {
            end += 1;
        }
        if end - start == MAX_URL_BYTES {
            return None;
        }
        if (end + 1).is_multiple_of(columns)
            && self.source_cell(end)?.flags.contains(Flags::WRAPLINE)
            && self.source_cell(end + 1).is_none()
        {
            return None;
        }
        let mut text = String::new();
        let mut positions = Vec::new();
        for index in start..=end {
            let cell = self.source_cell(index)?;
            if cell
                .flags
                .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
            {
                continue;
            }
            positions.push((text.len(), index));
            text.push(cell.c);
            text.extend(&cell.extra);
            if text.len() > MAX_URL_BYTES {
                return None;
            }
        }
        let byte = positions.iter().find(|(_, cell)| *cell == index)?.0;
        for (offset, _) in text.char_indices() {
            let candidate = &text[offset..];
            if !candidate
                .get(..7)
                .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
                && !candidate
                    .get(..8)
                    .is_some_and(|s| s.eq_ignore_ascii_case("https://"))
            {
                continue;
            }
            if text[..offset]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
            {
                continue;
            }
            let candidate = trim_url(candidate);
            let limit = offset + candidate.len();
            if byte < offset || byte >= limit {
                continue;
            }
            let url = WebLink::new(candidate)?;
            let start = positions.iter().find(|(byte, _)| *byte == offset)?.1;
            let end = positions.iter().rev().find(|(byte, _)| *byte < limit)?.1;
            let end = end + usize::from(self.source_cell(end)?.flags.contains(Flags::WIDE_CHAR));
            return Some(Link { url, start, end });
        }
        None
    }

    fn source_cell(&self, index: usize) -> Option<&Cell> {
        let columns = usize::from(self.columns);
        if columns == 0 {
            return None;
        }
        self.sources.get(index / columns)?.get(index % columns)
    }

    fn connected(&self, left: usize, right: usize) -> bool {
        let columns = usize::from(self.columns);
        columns > 0
            && (!right.is_multiple_of(columns)
                || self
                    .source_cell(left)
                    .is_some_and(|cell| cell.flags.contains(Flags::WRAPLINE)))
    }
}

fn link_modifier(modifiers: egui::Modifiers) -> bool {
    (modifiers.ctrl || modifiers.mac_cmd) && !modifiers.shift && !modifiers.alt
}

fn delimiter(cell: &Cell) -> bool {
    if cell
        .flags
        .intersects(Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER)
    {
        return false;
    }
    cell.flags.contains(Flags::HIDDEN)
        || cell.c.is_whitespace()
        || cell.c.is_control()
        || matches!(cell.c, '<' | '>' | '"' | '\'' | '`' | '|')
}

fn trim_url(mut url: &str) -> &str {
    let mut unmatched = [0_i32; 3];
    for c in url.chars() {
        match c {
            '(' => unmatched[0] -= 1,
            ')' => unmatched[0] += 1,
            '[' => unmatched[1] -= 1,
            ']' => unmatched[1] += 1,
            '{' => unmatched[2] -= 1,
            '}' => unmatched[2] += 1,
            _ => {}
        }
    }
    loop {
        let Some(last) = url.chars().next_back() else {
            return url;
        };
        let closing = match last {
            ')' => Some(0),
            ']' => Some(1),
            '}' => Some(2),
            _ => None,
        };
        if closing.is_some_and(|index| unmatched[index] > 0)
            || matches!(last, '.' | ',' | ';' | ':' | '!')
        {
            if let Some(index) = closing {
                unmatched[index] -= 1;
            }
            url = &url[..url.len() - last.len_utf8()];
        } else {
            return url;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn cache(text: &str, columns: usize) -> Cache {
        let mut rows = Vec::new();
        let cells: Vec<_> = text
            .chars()
            .enumerate()
            .map(|(column, c)| Cell {
                column: column % columns,
                c,
                ..Default::default()
            })
            .collect();
        for cells in cells.chunks(columns) {
            let mut row = cells.to_vec();
            row.resize_with(columns, Cell::default);
            rows.push(Arc::from(row));
        }
        let mut cache = Cache::default();
        cache.columns = columns as u16;
        cache.lines = rows.len() as u16;
        cache.sources = rows;
        cache
    }

    #[test]
    fn plain_links_keep_queries_and_balanced_parentheses_and_exclude_prose() {
        let cache = cache("See (https://example.com/a_(b)?x=1&y=2#part). next", 80);
        assert_eq!(
            cache.link_at(Point::new(0, 12)).unwrap().url.as_str(),
            "https://example.com/a_(b)?x=1&y=2#part"
        );
        for column in [0, 4, 43, 44, 45, 48] {
            assert!(
                cache.link_at(Point::new(0, column)).is_none(),
                "column {column}"
            );
        }
    }

    #[test]
    fn soft_wrapped_links_and_scrollback_use_grid_columns() {
        let mut cache = cache("https://example.com/wrapped/path rest", 16);
        for row in &mut cache.sources[..2] {
            Arc::make_mut(row)[15].flags.insert(Flags::WRAPLINE);
        }
        cache.display_offset = 9;
        let link = cache.link_at(Point::new(-8, 5)).unwrap();
        assert_eq!(link.url.as_str(), "https://example.com/wrapped/path");
        assert_eq!((link.start, link.end), (0, 31));
        Arc::make_mut(&mut cache.sources[0])[15]
            .flags
            .remove(Flags::WRAPLINE);
        assert!(cache.link_at(Point::new(-8, 5)).is_none());
    }

    #[test]
    fn wide_and_combining_characters_preserve_hit_regions() {
        let mut cache = cache("https://example.com/界 e rest", 48);
        let row = Arc::make_mut(&mut cache.sources[0]);
        row[20].flags.insert(Flags::WIDE_CHAR);
        row[21].flags.insert(Flags::WIDE_CHAR_SPACER);
        row[22].extra.push('\u{301}');
        let link = cache.link_at(Point::new(0, 21)).unwrap();
        assert_eq!(link.url.as_str(), "https://example.com/界e\u{301}");
        assert_eq!(link.end, 22);
        assert!(cache.link_at(Point::new(0, 23)).is_none());
    }

    #[test]
    fn explicit_link_targets_override_their_label_and_hidden_cells_are_ignored() {
        let mut cache = cache("https://label.example/", 32);
        let row = Arc::make_mut(&mut cache.sources[0]);
        for cell in &mut row[..22] {
            cell.hyperlink = Some(Arc::from("https://target.example/"));
        }
        assert_eq!(
            cache.link_at(Point::new(0, 9)).unwrap().url.as_str(),
            "https://target.example/"
        );
        Arc::make_mut(&mut cache.sources[0])[9].hyperlink = Some(Arc::from("file:///tmp/file"));
        assert!(cache.link_at(Point::new(0, 9)).is_none());
        Arc::make_mut(&mut cache.sources[0])[10]
            .flags
            .insert(Flags::HIDDEN);
        assert!(cache.link_at(Point::new(0, 10)).is_none());
    }

    #[test]
    fn hit_testing_bounds_long_tokens_and_does_not_join_hard_lines() {
        let long = cache(
            &format!("https://example.com/{}", "a".repeat(MAX_URL_BYTES + 1)),
            10000,
        );
        assert!(long.link_at(Point::new(0, 20)).is_none());
        let cache = cache("https://example.com/path", 16);
        assert!(cache.link_at(Point::new(1, 2)).is_none());
    }

    struct Gesture {
        ctx: egui::Context,
        cache: Cache,
        time: f64,
    }

    impl Gesture {
        fn new() -> Self {
            let mut gesture = Self {
                ctx: egui::Context::default(),
                cache: cache("https://example.com/ rest", 80),
                time: 0.0,
            };
            gesture.cache.cell = egui::vec2(8.0, 20.0);
            gesture.frame(Vec::new(), egui::Modifiers::NONE);
            gesture.frame(
                vec![egui::Event::PointerMoved(egui::pos2(50.0, 20.0))],
                egui::Modifiers::NONE,
            );
            gesture
        }

        fn button(pos: egui::Pos2, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers,
            }
        }

        fn frame(
            &mut self,
            mut events: Vec<egui::Event>,
            modifiers: egui::Modifiers,
        ) -> (super::super::PaintResult, egui::CursorIcon) {
            self.time += 0.1;
            events.insert(0, egui::Event::ModifiersChanged(modifiers));
            let mut painted = None;
            let mut output = self.ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(700.0, 120.0),
                    )),
                    time: Some(self.time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    painted = Some(self.cache.paint(
                        ui,
                        Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(640.0, 20.0)),
                        &crate::config::Config::default(),
                        crate::theme::Palette::new(crate::config::Theme::Graphite),
                        true,
                        "",
                        "",
                    ));
                },
            );
            output.textures_delta.clear();
            (painted.unwrap(), output.platform_output.cursor_icon)
        }
    }

    #[test]
    fn ctrl_and_command_click_open_once_and_own_the_tui_mouse_gesture() {
        for modifiers in [egui::Modifiers::CTRL, egui::Modifiers::MAC_CMD] {
            let mut gesture = Gesture::new();
            gesture.cache.mode = terminal_core::Mode::MOUSE_REPORT_CLICK;
            let (_, cursor) = gesture.frame(Vec::new(), modifiers);
            assert_eq!(cursor, egui::CursorIcon::PointingHand);
            let pos = egui::pos2(50.0, 20.0);
            let (press, _) = gesture.frame(vec![Gesture::button(pos, true, modifiers)], modifiers);
            assert!(press.open_link.is_none());
            assert!(press.interaction.is_none());
            assert!(gesture.cache.link_pointer_owned);
            let (release, _) =
                gesture.frame(vec![Gesture::button(pos, false, modifiers)], modifiers);
            assert_eq!(release.open_link.unwrap().as_str(), "https://example.com/");
            assert!(release.interaction.is_none());
            assert!(gesture.cache.link_pointer_owned);
            let (next, _) = gesture.frame(Vec::new(), modifiers);
            assert!(next.open_link.is_none());
            assert!(!gesture.cache.link_pointer_owned);
        }
    }

    #[test]
    fn ordinary_clicks_and_shift_selection_do_not_open_links() {
        for modifiers in [
            egui::Modifiers::NONE,
            egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
        ] {
            let mut gesture = Gesture::new();
            let pos = egui::pos2(50.0, 20.0);
            gesture.frame(vec![Gesture::button(pos, true, modifiers)], modifiers);
            let (release, cursor) =
                gesture.frame(vec![Gesture::button(pos, false, modifiers)], modifiers);
            assert!(release.open_link.is_none());
            assert_eq!(
                release.interaction,
                Some(super::super::SelectionInteraction::Clear)
            );
            assert_eq!(cursor, egui::CursorIcon::Text);
            assert!(!gesture.cache.link_pointer_owned);
        }
    }

    #[test]
    fn dragging_or_changing_the_target_cancels_link_activation() {
        let mut gesture = Gesture::new();
        let modifiers = egui::Modifiers::CTRL;
        let start = egui::pos2(50.0, 20.0);
        let end = egui::pos2(110.0, 20.0);
        gesture.frame(vec![Gesture::button(start, true, modifiers)], modifiers);
        let (drag, _) = gesture.frame(vec![egui::Event::PointerMoved(end)], modifiers);
        assert!(drag.interaction.is_none());
        let (release, _) = gesture.frame(vec![Gesture::button(end, false, modifiers)], modifiers);
        assert!(release.open_link.is_none());
        assert!(release.interaction.is_none());

        let mut gesture = Gesture::new();
        gesture.frame(vec![Gesture::button(start, true, modifiers)], modifiers);
        Arc::make_mut(&mut gesture.cache.sources[0])[8].c = 'z';
        let (release, _) = gesture.frame(vec![Gesture::button(start, false, modifiers)], modifiers);
        assert!(release.open_link.is_none());
    }

    #[test]
    fn clipped_soft_wraps_are_not_opened_as_truncated_urls() {
        let mut cache = cache("https://example.com/", 16);
        cache.sources.truncate(1);
        Arc::make_mut(&mut cache.sources[0])[15]
            .flags
            .insert(Flags::WRAPLINE);
        assert!(cache.link_at(Point::new(0, 5)).is_none());
    }
}
