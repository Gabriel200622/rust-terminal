use super::helpers::{ellipsize, tail_label};
use super::{Action, PaneRender};
use crate::{
    config::Config,
    icons::{self, Icon},
    theme::Palette,
};
use eframe::egui::{self, Align2, FontId, Layout, Pos2, Rect, Sense, Stroke, UiBuilder, Vec2};
use pace_model::{Axis, PaneId};
use terminal_core::{Mode as TermMode, SessionMetadata, ViewportSnapshot};
pub struct PanePresentation {
    pub metadata: SessionMetadata,
    pub snapshot: ViewportSnapshot,
}
#[allow(clippy::too_many_arguments)]
pub fn draw_node(
    ui: &mut egui::Ui,
    node: &pace_model::Layout,
    rect: Rect,
    panes: &mut std::collections::BTreeMap<PaneId, PaneRender>,
    presentations: &std::collections::BTreeMap<PaneId, PanePresentation>,
    active: PaneId,
    config: &Config,
    p: Palette,
    search: &str,
    actions: &mut Vec<Action>,
    active_rect: &mut Option<Rect>,
) {
    match node {
        pace_model::Layout::Leaf(id) => {
            let multiple = panes.len() > 1;
            let Some(pane) = panes.get_mut(id) else {
                return;
            };
            let selected = *id == active;
            let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 34.0));
            let show_controls = header.width() >= 72.0;
            let show_splits = header.width() >= 260.0;
            let controls_width = if show_splits { 112.0 } else { 40.0 };
            ui.painter().rect_filled(rect, 8, p.bg);
            ui.painter().rect_filled(
                header,
                egui::CornerRadius {
                    nw: 8,
                    ne: 8,
                    sw: 0,
                    se: 0,
                },
                p.sidebar,
            );
            ui.painter().line_segment(
                [header.left_bottom(), header.right_bottom()],
                Stroke::new(1.0, p.border),
            );
            if show_controls {
                icons::paint(
                    ui.painter(),
                    Rect::from_min_size(header.min + Vec2::new(12.0, 9.0), Vec2::splat(16.0)),
                    Icon::Terminal,
                    if selected { p.accent } else { p.muted },
                );
            }
            let Some(presentation) = presentations.get(id) else {
                return;
            };
            let metadata = &presentation.metadata;
            let shell = std::path::Path::new(&metadata.shell)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let custom = !metadata.title.is_empty()
                && !metadata.title.contains('@')
                && !metadata.title.contains(":/")
                && !metadata.title.contains(":~");
            let label = if custom {
                metadata.title.clone()
            } else {
                shell.into_owned()
            };
            let galley = ui.painter().layout_no_wrap(
                ellipsize(&label, 24),
                FontId::proportional(11.0),
                if selected { p.fg } else { p.secondary },
            );
            let label_width = galley.size().x;
            let text_left = (header.left() + 36.0).min(header.right());
            let text_rect = Rect::from_min_max(
                Pos2::new(text_left, header.top()),
                Pos2::new(
                    (header.right() - controls_width - 8.0).max(text_left),
                    header.bottom(),
                ),
            );
            let text_painter = ui.painter().with_clip_rect(text_rect);
            text_painter.galley(
                header.min + Vec2::new(36.0, (34.0 - galley.size().y) * 0.5),
                galley,
                p.fg,
            );
            let folder = metadata
                .cwd
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let path = tail_label(
                &folder,
                ((header.width() - label_width - 182.0) / 6.0).max(0.0) as usize,
            );
            if header.width() > 240.0 {
                text_painter.text(
                    header.min + Vec2::new(47.0 + label_width, 17.0),
                    Align2::LEFT_CENTER,
                    format!("/  {path}"),
                    FontId::proportional(10.0),
                    p.muted,
                );
            }
            if selected && multiple {
                ui.painter().line_segment(
                    [
                        header.left_top() + Vec2::new(10.0, 0.0),
                        header.left_top() + Vec2::new(34.0, 0.0),
                    ],
                    Stroke::new(1.5, p.accent),
                );
            }
            let title_response =
                ui.interact(text_rect, ui.id().with(("pane-title", *id)), Sense::click());
            title_response.clone().on_hover_text(format!(
                "{}\n{}",
                metadata.title,
                metadata.cwd.display()
            ));
            if title_response.clicked() {
                actions.push(Action::Focus(*id));
            }
            ui.scope_builder(
                UiBuilder::new()
                    .id_salt(("pane-controls", *id))
                    .max_rect(Rect::from_min_max(
                        Pos2::new(header.right() - 112.0, header.top() + 3.0),
                        header.max - Vec2::new(5.0, 3.0),
                    ))
                    .layout(Layout::right_to_left(egui::Align::Center)),
                |ui| {
                    ui.set_clip_rect(header.intersect(ui.clip_rect()));
                    if show_controls && icons::button(ui, Icon::Close, "Close terminal").clicked() {
                        actions.push(Action::ClosePane(*id));
                    }
                    if show_splits
                        && icons::button(ui, Icon::SplitHorizontal, "Split below").clicked()
                    {
                        actions.push(Action::Focus(*id));
                        actions.push(Action::Split(*id, Axis::Horizontal));
                    }
                    if show_splits
                        && icons::button(ui, Icon::SplitVertical, "Split right").clicked()
                    {
                        actions.push(Action::Focus(*id));
                        actions.push(Action::Split(*id, Axis::Vertical));
                    }
                },
            );
            let body = Rect::from_min_max(
                header.left_bottom() + Vec2::new(14.0, 12.0),
                rect.max - Vec2::new(12.0, 12.0),
            );
            ui.scope_builder(UiBuilder::new().id_salt(*id).max_rect(body), |ui| {
                if let Some(geometry) = pane.cache.geometry(ui, body, config) {
                    actions.push(Action::Resize(*id, geometry));
                }
                pane.cache.prepare(ui, &presentation.snapshot, config, p);
                let painted = pane.cache.paint(
                    ui,
                    body,
                    config,
                    p,
                    selected,
                    if selected { search } else { "" },
                    &pane.preedit,
                );
                let response = painted.response;
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Other,
                        true,
                        format!("Terminal pane {}", id.get()),
                    )
                });
                if let Some(interaction) = painted.interaction {
                    actions.push(Action::Selection(*id, interaction));
                }
                if response.clicked() || response.drag_started() {
                    actions.push(Action::Focus(*id));
                }
                if !pane.cache.mode.intersects(TermMode::MOUSE_MODE)
                    || ui.input(|i| i.modifiers.shift)
                {
                    response.context_menu(|ui| {
                        if ui.button("Focus terminal").clicked() {
                            actions.push(Action::Focus(*id));
                            actions.push(Action::Zoom);
                            ui.close();
                        }
                        if ui.button("Copy").clicked() {
                            actions.push(Action::Copy(*id));
                            ui.close();
                        }
                        if ui.button("Paste").clicked() {
                            actions.push(Action::Paste(*id));
                            ui.close();
                        }
                        if ui.button("Find").clicked() {
                            actions.push(Action::Focus(*id));
                            actions.push(Action::Find);
                            ui.close();
                        }
                    });
                }
            });
            if !matches!(metadata.status, terminal_core::SessionStatus::Running) {
                let banner = Rect::from_min_size(
                    body.left_bottom() - Vec2::new(0.0, 38.0),
                    Vec2::new(body.width().min(280.0), 32.0),
                );
                ui.painter().rect_filled(banner, 8, p.raised);
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(banner.shrink2(Vec2::new(10.0, 2.0)))
                        .layout(Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        let status = match &metadata.status {
                            terminal_core::SessionStatus::Exited { code, .. } => {
                                format!("Exited · {code}")
                            }
                            terminal_core::SessionStatus::Error(_) => "Session error".into(),
                            _ => String::new(),
                        };
                        let label =
                            ui.label(egui::RichText::new(status).size(11.0).color(p.secondary));
                        if let terminal_core::SessionStatus::Error(error) = &metadata.status {
                            label.on_hover_text(error);
                        }
                        if ui
                            .small_button("Restart")
                            .on_hover_text("Start a new shell · Enter")
                            .clicked()
                        {
                            actions.push(Action::Focus(*id));
                            actions.push(Action::Restart(*id));
                        }
                    },
                );
            } else if let Some(error) = pane.cache.resize_error.clone() {
                let banner = Rect::from_min_size(
                    body.left_bottom() - Vec2::new(0.0, 38.0),
                    Vec2::new(body.width().min(280.0), 32.0),
                );
                ui.painter().rect_filled(banner, 8, p.raised);
                ui.scope_builder(
                    UiBuilder::new()
                        .id_salt(("resize-error", *id))
                        .max_rect(banner.shrink2(Vec2::new(10.0, 2.0)))
                        .layout(Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Resize unavailable")
                                .size(11.0)
                                .color(p.secondary),
                        )
                        .on_hover_text(error);
                        if ui.small_button("Retry").clicked() {
                            pane.cache.retry_resize();
                            ui.ctx().request_repaint();
                        }
                    },
                );
            }
            if selected {
                *active_rect = Some(body);
            }
            if pane.cache.display_offset > 0 {
                let chip = Rect::from_min_size(
                    rect.right_bottom() - Vec2::new(126.0, 42.0),
                    Vec2::new(114.0, 26.0),
                );
                let r = ui.interact(chip, ui.id().with(("scroll-bottom", id)), Sense::click());
                ui.painter().rect_filled(chip, 13, p.hover);
                ui.painter().text(
                    chip.center(),
                    Align2::CENTER_CENTER,
                    "Back to bottom ↓",
                    FontId::proportional(10.0),
                    p.secondary,
                );
                if r.clicked() {
                    actions.push(Action::ScrollBottom(*id));
                }
            }
        }
        pace_model::Layout::Split {
            id: split,
            axis,
            ratio,
            first,
            second,
        } => {
            let vertical = *axis == Axis::Vertical;
            let length = if vertical {
                rect.width()
            } else {
                rect.height()
            };
            let min = if vertical { 200.0 } else { 130.0 };
            let low = (min / length).min(0.45);
            let visible_ratio = ratio.clamp(low, 1.0 - low);
            let cut = length * visible_ratio;
            let (a, b, gap) = if vertical {
                (
                    Rect::from_min_max(rect.min, Pos2::new(rect.left() + cut - 4.0, rect.bottom())),
                    Rect::from_min_max(Pos2::new(rect.left() + cut + 4.0, rect.top()), rect.max),
                    Rect::from_min_max(
                        Pos2::new(rect.left() + cut - 4.0, rect.top()),
                        Pos2::new(rect.left() + cut + 4.0, rect.bottom()),
                    ),
                )
            } else {
                (
                    Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + cut - 4.0)),
                    Rect::from_min_max(Pos2::new(rect.left(), rect.top() + cut + 4.0), rect.max),
                    Rect::from_min_max(
                        Pos2::new(rect.left(), rect.top() + cut - 4.0),
                        Pos2::new(rect.right(), rect.top() + cut + 4.0),
                    ),
                )
            };
            let r = ui
                .interact(gap, ui.id().with(("divider", split.get())), Sense::drag())
                .on_hover_cursor(if vertical {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::ResizeVertical
                });
            if r.dragged()
                && let Some(pos) = r.interact_pointer_pos()
            {
                let ratio = if vertical {
                    (pos.x - rect.left()) / length
                } else {
                    (pos.y - rect.top()) / length
                };
                actions.push(Action::Ratio(*split, ratio.clamp(0.1, 0.9)));
            }
            if r.double_clicked() {
                actions.push(Action::Ratio(*split, 0.5));
            }
            draw_node(
                ui,
                first,
                a,
                panes,
                presentations,
                active,
                config,
                p,
                search,
                actions,
                active_rect,
            );
            draw_node(
                ui,
                second,
                b,
                panes,
                presentations,
                active,
                config,
                p,
                search,
                actions,
                active_rect,
            );
        }
    }
}
