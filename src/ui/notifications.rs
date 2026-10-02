//! Notification popover. Every row captures its originating pane and generation.
use super::{Action, helpers};
use crate::{
    icons::{self, Icon},
    notifications::Notifications,
    theme::{self, Palette},
};
use eframe::egui::{self, Align, Id, Layout, Order, Pos2, RichText, Stroke, vec2};
use neptune_model::Model;

pub struct NotificationView<'a> {
    pub history: &'a Notifications,
    pub model: &'a Model,
    pub unavailable: bool,
    pub first_frame: &'a mut bool,
}

pub fn show(
    ctx: &egui::Context,
    p: Palette,
    mut view: NotificationView<'_>,
    actions: &mut Vec<Action>,
) -> egui::Response {
    let first_frame = *view.first_frame;
    let bounds = ctx.content_rect();
    let width = 380.0_f32.min(bounds.width() - 24.0).max(120.0);
    let response = egui::Area::new(Id::new("notification-popover"))
        .order(Order::Foreground)
        .movable(false)
        .fixed_pos(Pos2::new(
            bounds.right() - width - 12.0,
            bounds.top() + 46.0,
        ))
        .show(ctx, |ui| {
            // Areas remember their previous measured size. Reopening a once-empty
            // popover must let the scroll viewport grow with new notifications.
            ui.set_max_height((bounds.height() - 60.0).max(100.0));
            egui::Frame::popup(ui.style())
                .fill(p.elevated)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(10)
                .inner_margin(12)
                .show(ui, |ui| panel(ui, p, &mut view, width, actions));
        })
        .response;
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Window, true, "Notification history")
    });
    if !first_frame && response.clicked_elsewhere() {
        actions.push(Action::CloseOverlay);
    }
    response
}

fn panel(
    ui: &mut egui::Ui,
    p: Palette,
    view: &mut NotificationView<'_>,
    width: f32,
    actions: &mut Vec<Action>,
) {
    ui.set_width(width - 24.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("Notifications")
                .font(theme::semibold(15.0))
                .color(p.fg),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let close = icons::button(ui, Icon::Close, "Close notifications");
            if *view.first_frame && !ui.is_sizing_pass() {
                close.request_focus();
                *view.first_frame = false;
            }
            if close.clicked() {
                actions.push(Action::CloseOverlay);
            }
        });
    });
    ui.horizontal(|ui| {
        ui.add_enabled_ui(view.history.unread(None) > 0, |ui| {
            if helpers::button(ui, p, "Mark all read", helpers::ButtonKind::Secondary).clicked() {
                actions.push(Action::ReadNotifications);
            }
        });
        ui.add_enabled_ui(view.history.entries().next().is_some(), |ui| {
            if helpers::button(ui, p, "Clear all", helpers::ButtonKind::Secondary).clicked() {
                actions.push(Action::ClearNotifications);
            }
        });
    });
    ui.add_space(8.0);
    egui::ScrollArea::vertical()
        .id_salt("notification-list")
        .max_height((ui.ctx().content_rect().height() - 170.0).max(60.0))
        .show(ui, |ui| {
            if view.unavailable {
                ui.label(
                    RichText::new(
                        "Desktop delivery is unavailable. Check your system notification settings.",
                    )
                    .color(p.secondary),
                );
                ui.add_space(8.0);
            }
            if view.history.entries().next().is_none() {
                ui.add_space(16.0);
                ui.label(
                    RichText::new("You’re all caught up")
                        .font(theme::medium(13.0))
                        .color(p.fg),
                );
                ui.label(
                    RichText::new("Alerts from your terminals appear here.").color(p.secondary),
                );
                ui.add_space(16.0);
            }
            for entry in view.history.entries().rev() {
                let workspace = view
                    .model
                    .workspace_for_pane(entry.pane)
                    .and_then(|id| view.model.workspace(id))
                    .map_or("Terminal", |w| w.name());
                ui.push_id(entry.sequence, |ui| {
                    notification_row(ui, p, entry, workspace, width, actions)
                });
            }
        });
}

fn notification_row(
    ui: &mut egui::Ui,
    p: Palette,
    entry: &crate::notifications::Entry,
    workspace: &str,
    width: f32,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width((width - 72.0).max(64.0));
            ui.label(
                RichText::new(format!("{workspace} · Pane {}", entry.pane))
                    .font(theme::regular(11.0))
                    .color(p.secondary),
            );
            let title = if entry.unread {
                format!("• {}", entry.notification.title)
            } else {
                entry.notification.title.clone()
            };
            let title = RichText::new(title)
                .font(theme::medium(13.0))
                .color(if entry.unread { p.accent } else { p.fg });
            if ui
                .add(egui::Button::new(title).wrap().frame(false))
                .clicked()
            {
                actions.push(Action::OpenNotification(entry.pane, entry.generation));
            }
            if !entry.notification.body.is_empty() {
                ui.label(RichText::new(&entry.notification.body).color(p.secondary));
            }
        });
        if icons::button(ui, Icon::Close, "Dismiss notification").clicked() {
            actions.push(Action::DismissNotification(entry.sequence));
        }
    });
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);
}

/// Counts use the same compact pill in workspace and folder rows.
pub fn badge(ui: &egui::Ui, center: egui::Pos2, count: usize, p: Palette) {
    let label = if count > 99 {
        "99+".into()
    } else {
        count.to_string()
    };
    let text = ui
        .painter()
        .layout_no_wrap(label, theme::medium(10.0), p.on_accent);
    let rect = egui::Rect::from_center_size(center, vec2((text.size().x + 10.0).max(18.0), 18.0));
    ui.painter().rect_filled(rect, 9, p.accent);
    helpers::galley_at(
        ui.painter(),
        Pos2::new(rect.center().x - text.size().x * 0.5, rect.center().y),
        text,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use neptune_model::PaneId;

    #[test]
    fn notifications_popover_grows_after_new_entries_and_stays_inside_a_narrow_window() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::platform::fonts::bundled_definitions());
        let config = crate::config::Config::default();
        theme::apply(&ctx, &config);
        let p = Palette::for_config(&config);
        let model = Model::default();
        let mut history = Notifications::default();
        let render = |history: &Notifications, size: egui::Vec2| {
            let mut rect = egui::Rect::NOTHING;
            let mut first_frame = true;
            for _ in 0..3 {
                let mut frame = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |_| {
                        rect = show(
                            &ctx,
                            p,
                            NotificationView {
                                history,
                                model: &model,
                                unavailable: false,
                                first_frame: &mut first_frame,
                            },
                            &mut Vec::new(),
                        )
                        .rect;
                    },
                );
                frame.textures_delta.clear();
            }
            assert!(!first_frame, "visible popover accepts keyboard focus");
            rect
        };
        let empty = render(&history, vec2(1180.0, 760.0));
        for _ in 0..4 {
            history.push(
                PaneId::new(1),
                1,
                terminal_core::Notification {
                    title: "Ready for review".into(),
                    body: "A process is waiting for your input.".into(),
                    ..Default::default()
                },
            );
        }
        let populated = render(&history, vec2(1180.0, 760.0));
        assert!(
            populated.height() > empty.height() + 100.0,
            "{empty:?} -> {populated:?}"
        );
        let narrow = render(&history, vec2(640.0, 400.0));
        assert!(
            narrow.bottom() <= 400.0 && narrow.right() <= 640.0,
            "{narrow:?}"
        );
    }
}
