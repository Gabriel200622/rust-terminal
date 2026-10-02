//! Preferences: grouped settings that apply as they change.
use super::Action;
use super::helpers::{
    ButtonKind, SheetPlacement, animate, button, caption, group, padded, place, section_label,
    segmented, sheet, sheet_footer, sheet_header, slider, stepper, text_field, toggle,
};
use super::theme_browser;
use crate::{
    config::{Config, Cursor},
    theme::{self, Palette},
};
use eframe::egui::{self, Align, Align2, Id, Layout, WidgetInfo, WidgetType, vec2};

pub fn show(
    ctx: &egui::Context,
    current: &Config,
    updates: &crate::runtime::updates::Updates,
    state: &mut theme_browser::State,
    actions: &mut Vec<Action>,
) {
    let mut config = current.clone();
    let p = Palette::for_config(&config);
    let screen = ctx.content_rect();
    let mut close = false;
    // The catalog needs room for its grid: the sheet widens as it appears.
    let themes = animate(ctx, Id::new("preferences-themes"), state.open, 0.16);
    let width = egui::lerp(500.0..=theme_browser::WIDTH, themes);
    let output = sheet(ctx, p, "Preferences", width, SheetPlacement::Center, |ui| {
        // Leave the window visible around the sheet when there is room; a
        // short window gives most of that margin back to the settings.
        let chrome = 52.0 + 60.0;
        let body_height = (screen.height() - chrome - 96.0)
            .max((screen.height() - chrome - 24.0).min(260.0))
            .clamp(120.0, 560.0);
        if state.open {
            close |= theme_browser::show(ui, p, &mut config, state, body_height);
            return;
        }
        close |= sheet_header(ui, p, "Preferences", Some("Close preferences"));
        egui::ScrollArea::vertical()
            .id_salt("preferences-body")
            .max_height(body_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                padded(ui, 20.0, |ui| {
                    section_label(ui, p, "Appearance");
                    group(ui, p, |ui, rows| {
                        if theme_browser::summary(ui, p, &config, rows) {
                            state.browse();
                        }
                        rows.row(ui, "Window zoom", |ui| {
                            let text = format!("{:.0}%", config.window_zoom * 100.0);
                            if stepper(
                                ui,
                                p,
                                "Window zoom",
                                &mut config.window_zoom,
                                Config::WINDOW_ZOOM_RANGE,
                                0.1,
                                &text,
                            ) {
                                config.window_zoom = (config.window_zoom * 100.0).round() / 100.0;
                            }
                        });
                    });
                    caption(
                        ui,
                        p,
                        if cfg!(target_os = "macos") {
                            "Window zoom: Command+Plus / Minus. Reset: Command+0. Saved for next launch."
                        } else {
                            "Window zoom: Ctrl+Plus / Minus. Reset: Ctrl+0. Saved for next launch."
                        },
                    );

                    ui.add_space(14.0);
                    section_label(ui, p, "Text");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Font size", |ui| {
                            let text = format!("{} pt", config.font_size.round() as i32);
                            stepper(
                                ui,
                                p,
                                "Font size",
                                &mut config.font_size,
                                9.0..=32.0,
                                1.0,
                                &text,
                            );
                        });
                        rows.row(ui, "Line spacing", |ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            let (_, value) = ui.allocate_space(vec2(34.0, 20.0));
                            ui.painter().text(
                                value.right_center(),
                                Align2::RIGHT_CENTER,
                                format!("{:.2}", config.line_height),
                                theme::medium(12.5),
                                p.secondary,
                            );
                            slider(
                                ui,
                                p,
                                "Line spacing",
                                &mut config.line_height,
                                1.0..=2.0,
                                0.05,
                                150.0,
                            );
                        });
                    });
                    caption(
                        ui,
                        p,
                        if cfg!(target_os = "macos") {
                            "Font size: Command+Shift+Plus / Minus. Reset: Command+Shift+0."
                        } else {
                            "Font size: Ctrl+Shift+Plus / Minus. Reset: Ctrl+Shift+0."
                        },
                    );

                    ui.add_space(14.0);
                    section_label(ui, p, "Cursor");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Style", |ui| {
                            segmented(
                                ui,
                                p,
                                "cursor-style",
                                &mut config.cursor,
                                &[
                                    (Cursor::Block, "Block"),
                                    (Cursor::Beam, "Beam"),
                                    (Cursor::Underline, "Underline"),
                                ],
                                228.0,
                            );
                        });
                        rows.row(ui, "Blink", |ui| {
                            toggle(ui, p, &mut config.cursor_blink, "Blink cursor");
                        });
                    });

                    ui.add_space(14.0);
                    section_label(ui, p, "Shell");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Program", |ui| {
                            let mut shell = config.shell.clone().unwrap_or_default();
                            let width = (ui.available_width() - 110.0).clamp(120.0, 250.0);
                            if text_field(
                                ui,
                                p,
                                Id::new("preferences-shell"),
                                &mut shell,
                                "System default",
                                "Shell program",
                                width,
                            )
                            .changed()
                            {
                                config.shell = (!shell.trim().is_empty()).then_some(shell);
                            }
                        });
                        rows.row(ui, "Scrollback", |ui| {
                            ui.add(
                                egui::DragValue::new(&mut config.scrollback)
                                    .range(0..=1_000_000)
                                    .speed(100)
                                    .suffix(" lines"),
                            )
                            .widget_info(|| {
                                WidgetInfo::labeled(WidgetType::DragValue, true, "Scrollback lines")
                            });
                        });
                    });
                    caption(
                        ui,
                        p,
                        "Shell and scrollback changes apply to new terminals.",
                    );

                    ui.add_space(14.0);
                    section_label(ui, p, "Sessions");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Restore workspaces on launch", |ui| {
                            toggle(
                                ui,
                                p,
                                &mut config.restore_workspaces,
                                "Restore workspaces on launch",
                            );
                        });
                        rows.row(ui, "Confirm before closing terminals", |ui| {
                            toggle(
                                ui,
                                p,
                                &mut config.confirm_close,
                                "Confirm before closing terminals",
                            );
                        });
                    });
                    caption(
                        ui,
                        p,
                        "Restoring reopens folders and layouts with fresh shells.",
                    );
                    ui.add_space(14.0);
                    section_label(ui, p, "Notifications");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Desktop banners", |ui| {
                            toggle(
                                ui,
                                p,
                                &mut config.desktop_notifications,
                                "Desktop banners",
                            );
                        });
                    });
                    caption(
                        ui,
                        p,
                        "Show a system banner when a terminal asks for attention. Pane rings, unread counts and history stay on.",
                    );
                    ui.add_space(14.0);
                    section_label(ui, p, "Updates");
                    group(ui, p, |ui, rows| {
                        rows.row(ui, "Check automatically", |ui| {
                            toggle(ui, p, &mut config.check_updates, "Check for updates automatically");
                        });
                        rows.row(ui, "Release channel", |ui| {
                            use crate::runtime::updates::ReleaseChannel;
                            segmented(ui, p, "release-channel", &mut config.release_channel, &[(ReleaseChannel::Stable, "Stable"), (ReleaseChannel::Beta, "Beta")], 150.0);
                        });
                    });
                    caption(ui, p, "Beta includes previews and newer stable releases. Updates are installed only when you choose.");
                    crate::ui::updates::preferences_status(ui, p, updates, actions);
                    ui.add_space(18.0);
                });
            });

        let bar = sheet_footer(ui, p);
        place(
            ui,
            bar,
            Layout::left_to_right(Align::Center),
            "preferences-reset",
            |ui| {
                if button(ui, p, "Reset to defaults", ButtonKind::Quiet).clicked() {
                    config = Config {
                        custom_themes: config.custom_themes.clone(),
                        ..Config::default()
                    };
                }
            },
        );
        place(
            ui,
            bar,
            Layout::right_to_left(Align::Center),
            "preferences-done",
            |ui| {
                close |= button(ui, p, "Done", ButtonKind::Primary).clicked();
            },
        );
    });
    if (close || output.backdrop_clicked) && state.may_close() {
        actions.push(Action::CloseOverlay);
    }
    if &config != current {
        actions.push(Action::Preferences(config));
    }
}
