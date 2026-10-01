use super::Action;
use crate::{
    icons::{self, Icon},
    theme::Palette,
};
use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Vec2};
use pace_model::{Axis, PaneId};
pub fn show(
    ctx: &egui::Context,
    query: &mut String,
    open: &mut bool,
    pane: Option<PaneId>,
    p: Palette,
    actions: &mut Vec<Action>,
) {
    let mut visible = *open;
    let modkey = if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl+Shift+"
    };
    egui::Window::new("Commands")
        .open(&mut visible)
        .collapsible(false)
        .resizable(false)
        .default_width(470.0)
        .default_height(0.0)
        .anchor(Align2::CENTER_TOP, [0.0, 110.0])
        .show(ctx, |ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut (*query))
                    .hint_text("Find a command…")
                    .desired_width(f32::INFINITY),
            );
            if (*query).is_empty() {
                r.request_focus();
            }
            ui.add_space(8.0);
            let commands = [
                ("New workspace", "T", Icon::Plus, Action::New),
                (
                    "Split right",
                    "D",
                    Icon::SplitVertical,
                    Action::Split(pane.unwrap_or(PaneId::new(0)), Axis::Vertical),
                ),
                (
                    "Split below",
                    "E",
                    Icon::SplitHorizontal,
                    Action::Split(pane.unwrap_or(PaneId::new(0)), Axis::Horizontal),
                ),
                ("Find in terminal", "F", Icon::Search, Action::Find),
                ("Toggle sidebar", "B", Icon::Sidebar, Action::ToggleSidebar),
                ("Preferences", ",", Icon::Settings, Action::Settings),
                (
                    "Clear scrollback",
                    "",
                    Icon::Terminal,
                    Action::Clear(pane.unwrap_or(PaneId::new(0))),
                ),
                (
                    "Restart terminal",
                    "",
                    Icon::Terminal,
                    Action::Restart(pane.unwrap_or(PaneId::new(0))),
                ),
                ("Focus terminal", "Enter", Icon::Maximize, Action::Zoom),
            ];
            let mut first = true;
            for (name, key, icon, action) in commands {
                if !name.to_lowercase().contains(&(*query).to_lowercase()) {
                    continue;
                }
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());

                if response.hovered() {
                    ui.painter().rect_filled(rect, 7, p.hover);
                }
                icons::paint(
                    ui.painter(),
                    Rect::from_center_size(
                        Pos2::new(rect.left() + 18.0, rect.center().y),
                        Vec2::splat(16.0),
                    ),
                    icon,
                    p.secondary,
                );
                ui.painter().text(
                    Pos2::new(rect.left() + 40.0, rect.center().y),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::proportional(12.0),
                    p.fg,
                );
                if !key.is_empty() {
                    ui.painter().text(
                        Pos2::new(rect.right() - 12.0, rect.center().y),
                        Align2::RIGHT_CENTER,
                        format!("{modkey}{key}"),
                        FontId::proportional(10.0),
                        p.muted,
                    );
                }
                if response.clicked() || (first && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    actions.push(action);
                    *open = false;
                }
                first = false;
            }
        });
    *open = *open && visible;
}
