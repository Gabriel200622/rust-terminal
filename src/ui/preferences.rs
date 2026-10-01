use super::Action;
use crate::{
    config::{Config, Cursor, Theme},
    theme::Palette,
};
use eframe::egui::{self, Align2, Layout, Stroke, Vec2};
pub fn show(ctx: &egui::Context, current: &Config, open: &mut bool, actions: &mut Vec<Action>) {
    let mut config = current.clone();
    let p = Palette::new(config.theme);
    let before = toml::to_string(&config).unwrap_or_default();
    let mut visible = *open;
    egui::Window::new("Preferences")
        .open(&mut visible)
        .collapsible(false)
        .resizable(false)
        .default_width(450.0)
        .default_height(640.0)
        .max_height((ctx.content_rect().height() - 60.0).max(180.0))
        .vscroll(true)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::new().show(ui, |ui| {
                ui.spacing_mut().interact_size.y = 28.0;
                ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
                ui.add_space(2.0);
                ui.label(egui::RichText::new("Appearance").size(11.0).color(p.muted));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for (theme, name) in [
                        (Theme::Graphite, "Graphite"),
                        (Theme::Dusk, "Dusk"),
                        (Theme::Light, "Light"),
                    ] {
                        ui.selectable_value(&mut config.theme, theme, name);
                    }
                });
                ui.add_space(10.0);
                egui::Grid::new("settings-grid")
                    .num_columns(2)
                    .spacing([24.0, 14.0])
                    .show(ui, |ui| {
                        ui.label("Font size");
                        ui.add(
                            egui::Slider::new(&mut config.font_size, 9.0..=32.0)
                                .step_by(1.0)
                                .suffix(" px"),
                        );
                        ui.end_row();
                        ui.label("Line spacing");
                        ui.add(egui::Slider::new(&mut config.line_height, 1.0..=2.0).step_by(0.05));
                        ui.end_row();
                        ui.label("Cursor");
                        ui.horizontal(|ui| {
                            for (cursor, name) in [
                                (Cursor::Block, "Block"),
                                (Cursor::Beam, "Beam"),
                                (Cursor::Underline, "Line"),
                            ] {
                                ui.selectable_value(&mut config.cursor, cursor, name);
                            }
                        });
                        ui.end_row();
                        ui.label("Blink cursor");
                        ui.scope(|ui| {
                            ui.visuals_mut().widgets.inactive.bg_stroke = Stroke::new(1.0, p.muted);
                            ui.checkbox(&mut config.cursor_blink, "");
                        });
                        ui.end_row();
                        ui.label("Scrollback");
                        ui.add(
                            egui::DragValue::new(&mut config.scrollback)
                                .range(0..=1_000_000)
                                .speed(100),
                        );
                        ui.end_row();
                    });
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Sessions").size(11.0).color(p.muted));
                ui.checkbox(
                    &mut config.restore_workspaces,
                    "Restore workspace directories on launch",
                );
                ui.checkbox(
                    &mut config.confirm_close,
                    "Confirm before closing terminals",
                );
                ui.add_space(8.0);
                let mut shell = config.shell.clone().unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label("Shell");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut shell)
                                .hint_text("System default")
                                .desired_width(280.0),
                        )
                        .changed()
                    {
                        config.shell = if shell.trim().is_empty() {
                            None
                        } else {
                            Some(shell)
                        };
                    }
                });
                ui.label(
                    egui::RichText::new("Shell and scrollback changes apply to new terminals.")
                        .size(10.0)
                        .color(p.muted),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui.button("Reset defaults").clicked() {
                        config = Config::default();
                    }
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Done").clicked() {
                            *open = false;
                        }
                    });
                });
                ui.add_space(4.0);
            });
        });
    *open = *open && visible;
    if toml::to_string(&config).unwrap_or_default() != before {
        actions.push(Action::Preferences(config));
    }
}
