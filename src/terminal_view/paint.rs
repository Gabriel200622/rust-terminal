//! Painting consumes prepared draw data and emits selection interactions.

use super::cache::Cache;
use crate::{
    config::{Config, Cursor},
    theme::Palette,
};
use eframe::egui::{self, FontId, Pos2, Rect, Stroke, Vec2};
use terminal_core::{CursorShape, Mode as TermMode, Point, SelectionType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionInteraction {
    Start { point: Point, kind: SelectionType },
    Update(Point),
    Clear,
}

pub struct PaintResult {
    pub response: egui::Response,
    pub interaction: Option<SelectionInteraction>,
}

impl Cache {
    #[allow(clippy::too_many_arguments)]
    pub fn paint(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        config: &Config,
        p: Palette,
        active: bool,
        search: &str,
        preedit: &str,
    ) -> PaintResult {
        let selection = self.selection;
        let font = FontId::monospace(config.font_size);
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0, self.background);
        for (y, row) in self.rows.iter().enumerate() {
            let top = rect.top() + y as f32 * self.cell.y;
            for &(start, end, color) in &row.backgrounds {
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(rect.left() + start as f32 * self.cell.x, top),
                        Vec2::new((end - start) as f32 * self.cell.x, self.cell.y),
                    ),
                    0,
                    color,
                );
            }
            if let Some(range) = selection {
                let line = y as i32 - self.display_offset as i32;
                for x in 0..self.columns as usize {
                    if range.contains(Point::new(line, x)) {
                        painter.rect_filled(
                            Rect::from_min_size(
                                Pos2::new(rect.left() + x as f32 * self.cell.x, top),
                                self.cell,
                            ),
                            0,
                            p.selection,
                        );
                    }
                }
            }
            if !search.is_empty() {
                // ASCII and Unicode character offsets are kept distinct from UTF-8 bytes.
                for (byte, _) in row.text.match_indices(search) {
                    let col = row
                        .text_columns
                        .iter()
                        .rev()
                        .find(|(offset, _)| *offset <= byte)
                        .map(|(_, col)| *col)
                        .unwrap_or(0);
                    let end = row
                        .text_columns
                        .iter()
                        .find(|(offset, _)| *offset >= byte + search.len())
                        .map(|(_, col)| *col)
                        .unwrap_or(self.columns as usize);
                    let len = end.saturating_sub(col);
                    painter.rect_filled(
                        Rect::from_min_size(
                            Pos2::new(rect.left() + col as f32 * self.cell.x, top),
                            Vec2::new(len as f32 * self.cell.x, self.cell.y),
                        ),
                        2,
                        p.accent.gamma_multiply(0.25),
                    );
                }
            }
            for run in &row.runs {
                painter.galley(
                    Pos2::new(
                        rect.left() + run.column as f32 * self.cell.x,
                        top + (self.cell.y - run.galley.size().y) * 0.5,
                    ),
                    run.galley.clone(),
                    p.fg,
                );
            }
        }
        if let Some((x, y, shape)) = self.cursor {
            let pos = rect.min + Vec2::new(x as f32 * self.cell.x, y as f32 * self.cell.y);
            let cursor = Rect::from_min_size(pos, self.cell);
            let focused = active && ui.input(|i| i.focused);
            let blink =
                !config.cursor_blink || ui.input(|i| ((i.time * 2.0) as u64).is_multiple_of(2));
            if focused && blink {
                let shape = match config.cursor {
                    Cursor::Beam => CursorShape::Beam,
                    Cursor::Underline => CursorShape::Underline,
                    Cursor::Block => shape,
                };
                match shape {
                    CursorShape::Beam => {
                        painter.rect_filled(
                            Rect::from_min_size(pos, Vec2::new(1.5, self.cell.y)),
                            0,
                            p.accent,
                        );
                    }
                    CursorShape::Underline => {
                        painter.line_segment(
                            [cursor.left_bottom(), cursor.right_bottom()],
                            Stroke::new(2.0, p.accent),
                        );
                    }
                    _ => {
                        painter.rect_filled(cursor, 1, p.accent.gamma_multiply(0.42));
                    }
                }
            } else if !focused {
                painter.rect_stroke(
                    cursor,
                    1,
                    Stroke::new(1.0, p.muted),
                    egui::StrokeKind::Inside,
                );
            }
            if focused {
                ui.ctx().output_mut(|o| {
                    o.ime = Some(egui::output::IMEOutput {
                        rect,
                        cursor_rect: cursor,
                        purpose: egui::IMEPurpose::Normal,
                        should_interrupt_composition: false,
                    })
                });
            }
            if focused && config.cursor_blink {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
            }
            if !preedit.is_empty() {
                painter.text(
                    pos + Vec2::new(0.0, self.cell.y),
                    egui::Align2::LEFT_TOP,
                    preedit,
                    font,
                    p.accent,
                );
            }
        }
        let response = ui.interact(
            rect,
            ui.id().with("terminal"),
            egui::Sense::click_and_drag(),
        );
        response.clone().on_hover_cursor(egui::CursorIcon::Text);
        let interaction = response.interact_pointer_pos().and_then(|pos| {
            if self.mode.intersects(TermMode::MOUSE_MODE) && !ui.input(|i| i.modifiers.shift) {
                return None;
            }
            let point = super::geometry::point_at(
                rect,
                self.cell,
                self.columns,
                self.lines,
                self.display_offset,
                pos,
            );
            if response.double_clicked() {
                Some(SelectionInteraction::Start {
                    point,
                    kind: SelectionType::Semantic,
                })
            } else if response.drag_started() {
                Some(SelectionInteraction::Start {
                    point,
                    kind: SelectionType::Simple,
                })
            } else if response.dragged() {
                Some(SelectionInteraction::Update(point))
            } else if response.clicked() {
                Some(SelectionInteraction::Clear)
            } else {
                None
            }
        });
        PaintResult {
            response,
            interaction,
        }
    }
}
