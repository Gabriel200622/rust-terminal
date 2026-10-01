use super::helpers::{ellipsize, path_label};
use super::{Action, WorkspaceView};
use crate::{
    icons::{self, Icon},
    theme::Palette,
};
use eframe::egui::{self, Align2, FontId, Layout, Pos2, Rect, Sense, Stroke, UiBuilder, Vec2};
use pace_model::WorkspaceId;
pub fn titlebar(
    workspaces: &[WorkspaceView],
    active: Option<WorkspaceId>,
    ui: &mut egui::Ui,
    rect: Rect,
    p: Palette,
    radius: u8,
    actions: &mut Vec<Action>,
) {
    let ctx = ui.ctx().clone();
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius {
            nw: radius,
            ne: radius,
            sw: 0,
            se: 0,
        },
        p.sidebar,
    );
    ui.painter().line_segment(
        [rect.left_bottom(), rect.right_bottom()],
        Stroke::new(1.0, p.border),
    );
    let drag = ui.interact(
        rect.shrink2(Vec2::new(180.0, 0.0)),
        ui.id().with("title-drag"),
        Sense::click_and_drag(),
    );
    if drag.drag_started() {
        crate::platform::window::send(&ctx, crate::platform::window::WindowOperation::StartDrag);
    }
    if drag.double_clicked() {
        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        crate::platform::window::send(
            &ctx,
            crate::platform::window::WindowOperation::SetMaximized(!max),
        );
    }
    // A small, original two-stroke mark echoes a terminal prompt.
    let mark = Rect::from_min_size(rect.min + Vec2::new(17.0, 14.0), Vec2::splat(18.0));
    ui.painter().line_segment(
        [
            mark.min + Vec2::new(1.0, 2.0),
            mark.min + Vec2::new(7.0, 8.0),
        ],
        Stroke::new(2.0, p.accent),
    );
    ui.painter().line_segment(
        [
            mark.min + Vec2::new(7.0, 8.0),
            mark.min + Vec2::new(1.0, 14.0),
        ],
        Stroke::new(2.0, p.accent),
    );
    ui.painter().line_segment(
        [
            mark.min + Vec2::new(11.0, 14.0),
            mark.min + Vec2::new(18.0, 14.0),
        ],
        Stroke::new(2.0, p.accent),
    );
    ui.painter().text(
        rect.min + Vec2::new(45.0, 23.0),
        Align2::LEFT_CENTER,
        "Pace",
        FontId::proportional(14.0),
        p.fg,
    );
    ui.scope_builder(
        UiBuilder::new()
            .id_salt("titlebar-sidebar")
            .max_rect(Rect::from_min_size(
                rect.min + Vec2::new(112.0, 9.0),
                Vec2::new(64.0, 28.0),
            ))
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| {
            if icons::button(ui, Icon::Sidebar, "Toggle sidebar").clicked() {
                actions.push(Action::ToggleSidebar);
            }
        },
    );
    if let Some(ws) = workspaces.iter().find(|w| Some(w.id) == active) {
        ui.painter()
            .with_clip_rect(Rect::from_min_max(
                rect.min + Vec2::new(185.0, 0.0),
                rect.max - Vec2::new(185.0, 0.0),
            ))
            .text(
                Pos2::new(rect.center().x, rect.center().y),
                Align2::CENTER_CENTER,
                ellipsize(&ws.name, ((rect.width() - 400.0) / 7.0).max(8.0) as usize),
                FontId::proportional(12.0),
                p.secondary,
            );
    }
    ui.scope_builder(
        UiBuilder::new()
            .id_salt("titlebar-controls")
            .max_rect(Rect::from_min_max(
                Pos2::new(rect.right() - 164.0, rect.top() + 9.0),
                rect.max - Vec2::new(9.0, 9.0),
            ))
            .layout(Layout::right_to_left(egui::Align::Center)),
        |ui| {
            if icons::button(ui, Icon::Close, "Close window").clicked() {
                actions.push(Action::WindowClose);
            }
            if icons::button(ui, Icon::Maximize, "Maximize").clicked() {
                let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                crate::platform::window::send(
                    &ctx,
                    crate::platform::window::WindowOperation::SetMaximized(!max),
                );
            }
            if icons::button(ui, Icon::Minus, "Minimize").clicked() {
                crate::platform::window::send(
                    &ctx,
                    crate::platform::window::WindowOperation::Minimize,
                );
            }
            ui.add_space(8.0);
            if icons::button(ui, Icon::Command, "Command palette").clicked() {
                actions.push(Action::Palette);
            }
        },
    );
}

pub fn sidebar_ui(
    workspaces: &[WorkspaceView],
    active: Option<WorkspaceId>,
    ui: &mut egui::Ui,
    rect: Rect,
    p: Palette,
    radius: u8,
    actions: &mut Vec<Action>,
) {
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: radius,
            se: 0,
        },
        p.sidebar,
    );
    ui.painter().line_segment(
        [rect.right_top(), rect.right_bottom()],
        Stroke::new(1.0, p.border),
    );
    ui.painter().text(
        rect.min + Vec2::new(18.0, 30.0),
        Align2::LEFT_CENTER,
        "Workspaces",
        FontId::proportional(11.0),
        p.muted,
    );
    ui.scope_builder(
        UiBuilder::new()
            .id_salt("workspace-create")
            .max_rect(Rect::from_min_size(
                Pos2::new(rect.right() - 42.0, rect.top() + 16.0),
                Vec2::splat(28.0),
            )),
        |ui| {
            if icons::button(ui, Icon::Plus, "New workspace").clicked() {
                actions.push(Action::New);
            }
        },
    );
    ui.scope_builder(
        UiBuilder::new()
            .id_salt("workspace-list")
            .max_rect(Rect::from_min_max(
                rect.min + Vec2::new(10.0, 58.0),
                rect.max - Vec2::new(10.0, 64.0),
            )),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("workspaces")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for ws in workspaces {
                        let (row, _) = ui.allocate_exact_size(
                            Vec2::new(rect.width() - 20.0, 50.0),
                            Sense::hover(),
                        );
                        let response = ui.interact(
                            row,
                            ui.id().with(("workspace", ws.id.get())),
                            Sense::click(),
                        );
                        if Some(ws.id) == active {
                            ui.painter().rect_filled(row, 10, p.hover);
                        } else if response.hovered() {
                            ui.painter().rect_filled(row, 10, p.raised);
                        }
                        let icon =
                            Rect::from_min_size(row.min + Vec2::new(12.0, 11.0), Vec2::splat(16.0));
                        icons::paint(
                            ui.painter(),
                            icon,
                            Icon::Terminal,
                            if Some(ws.id) == active {
                                p.accent
                            } else {
                                p.secondary
                            },
                        );
                        let max_name = (rect.width() - 84.0) / 7.0;
                        let name = ellipsize(&ws.name, max_name as usize);
                        ui.painter().text(
                            row.min + Vec2::new(38.0, 17.0),
                            Align2::LEFT_CENTER,
                            name,
                            FontId::proportional(12.0),
                            if Some(ws.id) == active {
                                p.fg
                            } else {
                                p.secondary
                            },
                        );
                        ui.painter().text(
                            row.min + Vec2::new(38.0, 34.0),
                            Align2::LEFT_CENTER,
                            path_label(&ws.cwd, ((rect.width() - 54.0) / 6.0) as usize),
                            FontId::proportional(10.0),
                            p.muted,
                        );
                        ui.painter().circle_filled(
                            Pos2::new(row.right() - 14.0, row.top() + 17.0),
                            2.5,
                            if ws.running {
                                if Some(ws.id) == active {
                                    p.green
                                } else {
                                    p.muted
                                }
                            } else {
                                p.ansi[1]
                            },
                        );
                        response
                            .clone()
                            .on_hover_text(ws.cwd.display().to_string())
                            .widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::SelectableLabel,
                                    true,
                                    Some(ws.id) == active,
                                    &ws.name,
                                )
                            });
                        if response.clicked() {
                            actions.push(Action::SelectWorkspace(ws.id));
                        }
                        response.context_menu(|ui| {
                            if ui.button("Rename").clicked() {
                                actions.push(Action::Rename(ws.id));
                                ui.close();
                            }
                            if ui.button("Close workspace").clicked() {
                                actions.push(Action::CloseWorkspace(ws.id));
                                ui.close();
                            }
                        });
                    }
                });
        },
    );
    let bottom = rect.bottom() - 52.0;
    ui.painter().line_segment(
        [
            Pos2::new(rect.left() + 16.0, bottom),
            Pos2::new(rect.right() - 16.0, bottom),
        ],
        Stroke::new(1.0, p.border),
    );
    ui.scope_builder(
        UiBuilder::new()
            .id_salt("sidebar-footer")
            .max_rect(Rect::from_min_size(
                Pos2::new(rect.left() + 12.0, bottom + 12.0),
                Vec2::new(rect.width() - 24.0, 28.0),
            ))
            .layout(Layout::left_to_right(egui::Align::Center)),
        |ui| {
            if icons::button(ui, Icon::Settings, "Preferences").clicked() {
                actions.push(Action::Settings);
            }
            if icons::button(ui, Icon::Keyboard, "Shortcuts and commands").clicked() {
                actions.push(Action::Palette);
            }
        },
    );
}
